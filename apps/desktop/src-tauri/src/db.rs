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
                prompt_template TEXT,
                enabled         INTEGER NOT NULL DEFAULT 1,
                created_at      INTEGER NOT NULL,
                last_run_at     INTEGER,
                last_run_status TEXT,
                next_run_at     INTEGER
            );
            -- Idempotent migration for installs that pre-date the column.
            -- ALTER TABLE ... ADD COLUMN errors with "duplicate column"; we
            -- swallow it via the IF clause that SQLite's PRAGMA doesn't have.
            -- A no-op SELECT against the column tells us whether to skip.

            CREATE TABLE IF NOT EXISTS automation_runs (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                automation_id   INTEGER,
                started_at      INTEGER NOT NULL,
                ended_at        INTEGER,
                status          TEXT NOT NULL,
                trigger_event_id TEXT,
                pending_prompt  TEXT,
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
        // Add prompt_template to pre-existing automations tables. Ignore the
        // "duplicate column" error so re-runs are no-ops.
        let _ = conn.execute(
            "ALTER TABLE automations ADD COLUMN prompt_template TEXT",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE automation_runs ADD COLUMN pending_prompt TEXT",
            [],
        );
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

    // ── automations helpers ─────────────────────────────────────────────────

    pub fn insert_automation(&self, row: &NewAutomationRow<'_>) -> Result<i64, DbError> {
        let conn = self.0.lock();
        conn.execute(
            "INSERT INTO automations (name, agent_id, trigger_kind, trigger_config, guards, prompt_template, enabled, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![row.name, row.agent_id, row.trigger_kind, row.trigger_config, row.guards, row.prompt_template, row.enabled as i64, row.created_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_automations(&self) -> Result<Vec<AutomationRow>, DbError> {
        let conn = self.0.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, agent_id, trigger_kind, trigger_config, guards, prompt_template, enabled, created_at, last_run_at, last_run_status \
             FROM automations ORDER BY created_at",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(AutomationRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    agent_id: row.get(2)?,
                    trigger_kind: row.get(3)?,
                    trigger_config: row.get(4)?,
                    guards: row.get(5)?,
                    prompt_template: row.get(6)?,
                    enabled: row.get::<_, i64>(7)? != 0,
                    created_at: row.get(8)?,
                    last_run_at: row.get(9)?,
                    last_run_status: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_automation(&self, id: i64) -> Result<(), DbError> {
        let conn = self.0.lock();
        conn.execute("DELETE FROM automations WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn set_automation_enabled(&self, id: i64, enabled: bool) -> Result<(), DbError> {
        let conn = self.0.lock();
        conn.execute(
            "UPDATE automations SET enabled = ?2 WHERE id = ?1",
            params![id, enabled as i64],
        )?;
        Ok(())
    }

    pub fn already_processed(&self, automation_id: i64, event_id: &str) -> Result<bool, DbError> {
        let conn = self.0.lock();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM processed_events WHERE automation_id = ?1 AND event_id = ?2",
            params![automation_id, event_id],
            |row| row.get(0),
        )?;
        Ok(n > 0)
    }

    pub fn mark_processed(
        &self,
        automation_id: i64,
        event_id: &str,
        processed_at: i64,
    ) -> Result<(), DbError> {
        let conn = self.0.lock();
        conn.execute(
            "INSERT OR IGNORE INTO processed_events (automation_id, event_id, processed_at) VALUES (?1, ?2, ?3)",
            params![automation_id, event_id, processed_at],
        )?;
        Ok(())
    }

    pub fn record_automation_run_start(
        &self,
        automation_id: i64,
        started_at: i64,
    ) -> Result<i64, DbError> {
        let conn = self.0.lock();
        conn.execute(
            "INSERT INTO automation_runs (automation_id, started_at, status) VALUES (?1, ?2, 'running')",
            params![automation_id, started_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn record_pending_run(
        &self,
        automation_id: i64,
        started_at: i64,
        prompt: &str,
    ) -> Result<i64, DbError> {
        let conn = self.0.lock();
        conn.execute(
            "INSERT INTO automation_runs (automation_id, started_at, status, pending_prompt) \
             VALUES (?1, ?2, 'awaiting_approval', ?3)",
            params![automation_id, started_at, prompt],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_pending_runs(&self) -> Result<Vec<PendingRun>, DbError> {
        let conn = self.0.lock();
        let mut stmt = conn.prepare(
            "SELECT r.id, r.automation_id, a.name, a.agent_id, r.started_at, r.pending_prompt \
             FROM automation_runs r \
             JOIN automations a ON a.id = r.automation_id \
             WHERE r.status = 'awaiting_approval' \
             ORDER BY r.started_at DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(PendingRun {
                    id: row.get(0)?,
                    automation_id: row.get(1)?,
                    automation_name: row.get(2)?,
                    agent_id: row.get(3)?,
                    queued_at: row.get(4)?,
                    prompt: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn take_pending_run(&self, run_id: i64) -> Result<Option<PendingRun>, DbError> {
        let conn = self.0.lock();
        let row = conn
            .query_row(
                "SELECT r.id, r.automation_id, a.name, a.agent_id, r.started_at, r.pending_prompt \
                 FROM automation_runs r \
                 JOIN automations a ON a.id = r.automation_id \
                 WHERE r.id = ?1 AND r.status = 'awaiting_approval'",
                params![run_id],
                |row| {
                    Ok(PendingRun {
                        id: row.get(0)?,
                        automation_id: row.get(1)?,
                        automation_name: row.get(2)?,
                        agent_id: row.get(3)?,
                        queued_at: row.get(4)?,
                        prompt: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    })
                },
            )
            .ok();
        Ok(row)
    }

    pub fn mark_pending_run_status(&self, run_id: i64, status: &str) -> Result<(), DbError> {
        let conn = self.0.lock();
        conn.execute(
            "UPDATE automation_runs SET status = ?2 WHERE id = ?1",
            params![run_id, status],
        )?;
        Ok(())
    }

    pub fn count_pending_runs(&self) -> Result<u32, DbError> {
        let conn = self.0.lock();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM automation_runs WHERE status = 'awaiting_approval'",
            [],
            |row| row.get(0),
        )?;
        Ok(n as u32)
    }

    pub fn record_automation_run_end(
        &self,
        run_id: i64,
        ended_at: i64,
        status: &str,
        output: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.0.lock();
        conn.execute(
            "UPDATE automation_runs SET ended_at = ?2, status = ?3, output_text = ?4, error_text = ?5 WHERE id = ?1",
            params![run_id, ended_at, status, output, error],
        )?;
        Ok(())
    }

    pub fn runs_last_hour(&self, automation_id: i64, now: i64) -> Result<u32, DbError> {
        let conn = self.0.lock();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM automation_runs WHERE automation_id = ?1 AND started_at >= ?2",
            params![automation_id, now - 3600],
            |row| row.get(0),
        )?;
        Ok(n as u32)
    }

    pub fn list_recent_automation_runs(
        &self,
        limit: i64,
    ) -> Result<Vec<AutomationRunRow>, DbError> {
        let conn = self.0.lock();
        let mut stmt = conn.prepare(
            "SELECT id, automation_id, started_at, ended_at, status, output_text, error_text, dry_run \
             FROM automation_runs ORDER BY started_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(AutomationRunRow {
                    id: row.get(0)?,
                    automation_id: row.get(1)?,
                    started_at: row.get(2)?,
                    ended_at: row.get(3)?,
                    status: row.get(4)?,
                    output_text: row.get(5)?,
                    error_text: row.get(6)?,
                    dry_run: row.get::<_, i64>(7)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_executions_for_agent(
        &self,
        agent_id: &str,
        limit: i64,
    ) -> Result<Vec<ExecutionRow>, DbError> {
        let conn = self.0.lock();
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, agent_source, cli, started_at, ended_at, exit_code, prompt_hash, cwd, dry_run \
             FROM executions WHERE agent_id = ?1 ORDER BY started_at DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![agent_id, limit], |row| {
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

    pub fn list_automation_runs(
        &self,
        automation_id: i64,
        limit: i64,
    ) -> Result<Vec<AutomationRunRow>, DbError> {
        let conn = self.0.lock();
        let mut stmt = conn.prepare(
            "SELECT id, automation_id, started_at, ended_at, status, output_text, error_text, dry_run \
             FROM automation_runs WHERE automation_id = ?1 ORDER BY started_at DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![automation_id, limit], |row| {
                Ok(AutomationRunRow {
                    id: row.get(0)?,
                    automation_id: row.get(1)?,
                    started_at: row.get(2)?,
                    ended_at: row.get(3)?,
                    status: row.get(4)?,
                    output_text: row.get(5)?,
                    error_text: row.get(6)?,
                    dry_run: row.get::<_, i64>(7)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
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
pub struct NewAutomationRow<'a> {
    pub name: &'a str,
    pub agent_id: &'a str,
    pub trigger_kind: &'a str,
    pub trigger_config: &'a str,
    pub guards: &'a str,
    pub prompt_template: Option<&'a str>,
    pub enabled: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRow {
    pub id: i64,
    pub name: String,
    pub agent_id: String,
    pub trigger_kind: String,
    pub trigger_config: String,
    pub guards: String,
    pub prompt_template: Option<String>,
    pub enabled: bool,
    pub created_at: i64,
    pub last_run_at: Option<i64>,
    pub last_run_status: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingRun {
    pub id: i64,
    pub automation_id: i64,
    pub automation_name: String,
    pub agent_id: String,
    pub queued_at: i64,
    pub prompt: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRunRow {
    pub id: i64,
    pub automation_id: Option<i64>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub status: String,
    pub output_text: Option<String>,
    pub error_text: Option<String>,
    pub dry_run: bool,
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
