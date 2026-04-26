//! Single SQLite connection at `~/.senda/data.db`. Schema is defined inline
//! and applied on first open — `IF NOT EXISTS` keeps it idempotent.
//!
//! Phase 1 only needs the `executions` table; later phases add automations,
//! processed_events, agent_targets, etc.

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{params, Connection};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("could not resolve home directory")]
    NoHome,
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct Db(pub Arc<Mutex<Connection>>);

impl Db {
    pub fn open() -> Result<Self, DbError> {
        let home = dirs::home_dir().ok_or(DbError::NoHome)?;
        let dir = home.join(".senda");
        std::fs::create_dir_all(&dir)?;
        let path: PathBuf = dir.join("data.db");
        let conn = Connection::open(&path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Db(Arc::new(Mutex::new(conn)));
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), DbError> {
        let conn = self.0.lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS executions (
                id            TEXT PRIMARY KEY,
                agent_id      TEXT NOT NULL,
                agent_source  TEXT NOT NULL,
                cli           TEXT NOT NULL,
                started_at    INTEGER NOT NULL,
                ended_at      INTEGER,
                exit_code     INTEGER,
                prompt_hash   TEXT NOT NULL,
                cwd           TEXT,
                dry_run       INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_executions_started
                ON executions(started_at DESC);
            CREATE INDEX IF NOT EXISTS idx_executions_agent
                ON executions(agent_id);

            CREATE TABLE IF NOT EXISTS connected_repos (
                id                    INTEGER PRIMARY KEY AUTOINCREMENT,
                provider              TEXT NOT NULL,
                org                   TEXT NOT NULL,
                project               TEXT,
                repo                  TEXT NOT NULL,
                url                   TEXT NOT NULL,
                local_path            TEXT NOT NULL,
                branch                TEXT NOT NULL DEFAULT 'main',
                auth_kind             TEXT NOT NULL,
                auth_keyring_id       TEXT,
                auto_sync             INTEGER NOT NULL DEFAULT 1,
                sync_interval_seconds INTEGER NOT NULL DEFAULT 600,
                last_synced_at        INTEGER,
                last_sync_error       TEXT,
                created_at            INTEGER NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_repos_url
                ON connected_repos(url);

            CREATE TABLE IF NOT EXISTS automations (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL UNIQUE,
                agent_id        TEXT NOT NULL,
                trigger_kind    TEXT NOT NULL,
                trigger_config  TEXT NOT NULL,
                guards          TEXT NOT NULL,
                enabled         INTEGER NOT NULL DEFAULT 1,
                created_at      INTEGER NOT NULL,
                last_run_at     INTEGER,
                last_run_status TEXT,
                next_run_at     INTEGER
            );

            CREATE TABLE IF NOT EXISTS automation_runs (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                automation_id   INTEGER,
                started_at      INTEGER NOT NULL,
                ended_at        INTEGER,
                status          TEXT NOT NULL,
                trigger_event_id TEXT,
                output_text     TEXT,
                error_text      TEXT,
                dry_run         INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (automation_id) REFERENCES automations(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS processed_events (
                automation_id INTEGER NOT NULL,
                event_id      TEXT NOT NULL,
                processed_at  INTEGER NOT NULL,
                PRIMARY KEY (automation_id, event_id),
                FOREIGN KEY (automation_id) REFERENCES automations(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS published_agents (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                agent_name  TEXT NOT NULL,
                repo_id     INTEGER NOT NULL,
                pr_url      TEXT NOT NULL,
                pr_number   INTEGER NOT NULL,
                pr_state    TEXT NOT NULL,
                draft_path  TEXT NOT NULL,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                FOREIGN KEY (repo_id) REFERENCES connected_repos(id) ON DELETE CASCADE
            );
            "#,
        )?;
        Ok(())
    }

    // ── connected_repos helpers ──────────────────────────────────────────────

    pub fn insert_repo(&self, row: &NewRepoRow<'_>) -> Result<i64, DbError> {
        let conn = self.0.lock();
        conn.execute(
            "INSERT INTO connected_repos (provider, org, project, repo, url, local_path, branch, auth_kind, auth_keyring_id, auto_sync, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                row.provider,
                row.org,
                row.project,
                row.repo,
                row.url,
                row.local_path,
                row.branch,
                row.auth_kind,
                row.auth_keyring_id,
                row.auto_sync as i64,
                row.created_at,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_repos(&self) -> Result<Vec<RepoRow>, DbError> {
        let conn = self.0.lock();
        let mut stmt = conn.prepare(
            "SELECT id, provider, org, project, repo, url, local_path, branch, auth_kind, auth_keyring_id, auto_sync, sync_interval_seconds, last_synced_at, last_sync_error \
             FROM connected_repos ORDER BY created_at",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(RepoRow {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    org: row.get(2)?,
                    project: row.get(3)?,
                    repo: row.get(4)?,
                    url: row.get(5)?,
                    local_path: row.get(6)?,
                    branch: row.get(7)?,
                    auth_kind: row.get(8)?,
                    auth_keyring_id: row.get(9)?,
                    auto_sync: row.get::<_, i64>(10)? != 0,
                    sync_interval_seconds: row.get::<_, i64>(11)? as u32,
                    last_synced_at: row.get(12)?,
                    last_sync_error: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_repo(&self, id: i64) -> Result<(), DbError> {
        let conn = self.0.lock();
        conn.execute("DELETE FROM connected_repos WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn record_sync(&self, id: i64, synced_at: i64, error: Option<&str>) -> Result<(), DbError> {
        let conn = self.0.lock();
        conn.execute(
            "UPDATE connected_repos SET last_synced_at = ?2, last_sync_error = ?3 WHERE id = ?1",
            params![id, synced_at, error],
        )?;
        Ok(())
    }

    pub fn record_start(&self, row: &ExecutionStart<'_>) -> Result<(), DbError> {
        let conn = self.0.lock();
        conn.execute(
            "INSERT INTO executions (id, agent_id, agent_source, cli, started_at, prompt_hash, cwd, dry_run) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                row.id,
                row.agent_id,
                row.agent_source,
                row.cli,
                row.started_at,
                row.prompt_hash,
                row.cwd,
                row.dry_run as i64
            ],
        )?;
        Ok(())
    }

    pub fn record_end(
        &self,
        id: &str,
        ended_at: i64,
        exit_code: Option<i32>,
    ) -> Result<(), DbError> {
        let conn = self.0.lock();
        conn.execute(
            "UPDATE executions SET ended_at = ?2, exit_code = ?3 WHERE id = ?1",
            params![id, ended_at, exit_code],
        )?;
        Ok(())
    }

    pub fn list_executions(&self, limit: i64) -> Result<Vec<ExecutionRow>, DbError> {
        let conn = self.0.lock();
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, agent_source, cli, started_at, ended_at, exit_code, prompt_hash, cwd, dry_run \
             FROM executions ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit], |row| {
                Ok(ExecutionRow {
                    id: row.get(0)?,
                    agent_id: row.get(1)?,
                    agent_source: row.get(2)?,
                    cli: row.get(3)?,
                    started_at: row.get(4)?,
                    ended_at: row.get(5)?,
                    exit_code: row.get(6)?,
                    prompt_hash: row.get(7)?,
                    cwd: row.get(8)?,
                    dry_run: row.get::<_, i64>(9)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[derive(Debug, Clone)]
pub struct NewRepoRow<'a> {
    pub provider: &'a str,
    pub org: &'a str,
    pub project: Option<&'a str>,
    pub repo: &'a str,
    pub url: &'a str,
    pub local_path: &'a str,
    pub branch: &'a str,
    pub auth_kind: &'a str,
    pub auth_keyring_id: Option<&'a str>,
    pub auto_sync: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoRow {
    pub id: i64,
    pub provider: String,
    pub org: String,
    pub project: Option<String>,
    pub repo: String,
    pub url: String,
    pub local_path: String,
    pub branch: String,
    pub auth_kind: String,
    pub auth_keyring_id: Option<String>,
    pub auto_sync: bool,
    pub sync_interval_seconds: u32,
    pub last_synced_at: Option<i64>,
    pub last_sync_error: Option<String>,
}

/// Bundle of fields needed to record an execution start; using a struct
/// keeps the call site readable when SQLite needs eight columns in one go.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionStart<'a> {
    pub id: &'a str,
    pub agent_id: &'a str,
    pub agent_source: &'a str,
    pub cli: &'a str,
    pub started_at: i64,
    pub prompt_hash: &'a str,
    pub cwd: Option<&'a str>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRow {
    pub id: String,
    pub agent_id: String,
    pub agent_source: String,
    pub cli: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub exit_code: Option<i32>,
    pub prompt_hash: String,
    pub cwd: Option<String>,
    pub dry_run: bool,
}
