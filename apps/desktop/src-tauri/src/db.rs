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
            "#,
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
