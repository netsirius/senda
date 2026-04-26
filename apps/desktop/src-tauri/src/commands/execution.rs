//! Spawn a CLI agent under a PTY and stream its output back to the frontend
//! as Tauri events. Each invocation gets a fresh `executionId`; the frontend
//! listens for `execution:<id>:output` and `execution:<id>:done`.
//!
//! Cancellation: `cancel_execution(executionId)` kills the child via the
//! handle stashed in [`Executions`].

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex as PlMutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Emitter;

use crate::db::Db;
use senda_core::AgentCli;

/// Per-execution handle so the cancel command can stop a running PTY.
struct Running {
    /// Holds the spawned child so we can `kill()` on cancel. Wrapped in `Arc<Mutex<_>>`
    /// because both the writer task and the cancel command may interact with it.
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
}

#[derive(Default)]
pub struct Executions {
    inner: PlMutex<HashMap<String, Running>>,
}

impl Executions {
    fn insert(&self, id: String, running: Running) {
        self.inner.lock().insert(id, running);
    }
    fn take(&self, id: &str) -> Option<Running> {
        self.inner.lock().remove(id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RunAgentArgs {
    pub agent_id: String,
    pub agent_source: String,
    pub cli: AgentCli,
    pub agent_name: String,
    pub prompt: String,
    pub cwd: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RunAgentResult {
    pub execution_id: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct OutputEvent {
    chunk: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
struct DoneEvent {
    exit_code: Option<i32>,
    error: Option<String>,
}

#[tauri::command]
pub async fn run_agent(
    app: tauri::AppHandle,
    state: tauri::State<'_, Executions>,
    db: tauri::State<'_, Db>,
    args: RunAgentArgs,
) -> Result<RunAgentResult, String> {
    let execution_id = uuid::Uuid::new_v4().to_string();
    let prompt_hash = sha256_hex(&args.prompt);
    let started_at = unix_now();

    let cmd = build_command(&args);

    db.record_start(&crate::db::ExecutionStart {
        id: &execution_id,
        agent_id: &args.agent_id,
        agent_source: &args.agent_source,
        cli: args.cli.as_id(),
        started_at,
        prompt_hash: &prompt_hash,
        cwd: args.cwd.as_deref(),
        dry_run: args.dry_run,
    })
    .map_err(|e| format!("db: {e}"))?;

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("openpty: {e}"))?;

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn: {e}"))?;
    drop(pair.slave); // free the slave so the child gets EOF on master close

    let child_arc: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>> =
        Arc::new(Mutex::new(child));
    state.insert(
        execution_id.clone(),
        Running {
            child: Arc::clone(&child_arc),
        },
    );

    let exec_id_for_reader = execution_id.clone();
    let app_for_reader = app.clone();
    let master = pair.master;
    let db_for_reader = db.inner().clone();

    std::thread::spawn(move || {
        run_pty_pump(
            exec_id_for_reader,
            app_for_reader,
            master,
            child_arc,
            db_for_reader,
        );
    });

    Ok(RunAgentResult { execution_id })
}

#[tauri::command]
pub async fn cancel_execution(
    state: tauri::State<'_, Executions>,
    execution_id: String,
) -> Result<(), String> {
    if let Some(running) = state.take(&execution_id) {
        let mut child = running.child.lock().unwrap();
        let _ = child.kill();
    }
    Ok(())
}

#[tauri::command]
pub async fn list_executions(
    db: tauri::State<'_, Db>,
    limit: Option<i64>,
) -> Result<Vec<crate::db::ExecutionRow>, String> {
    db.list_executions(limit.unwrap_or(50))
        .map_err(|e| format!("db: {e}"))
}

// ── internals ────────────────────────────────────────────────────────────────

fn build_command(args: &RunAgentArgs) -> CommandBuilder {
    let (program, extra_args): (&str, Vec<String>) = match args.cli {
        AgentCli::Copilot => (
            "copilot",
            vec![
                format!("--agent={}", args.agent_name),
                format!("--prompt={}", args.prompt),
            ],
        ),
        AgentCli::ClaudeCode => (
            "claude",
            vec![
                "--agent".into(),
                args.agent_name.clone(),
                args.prompt.clone(),
            ],
        ),
        AgentCli::Gemini => (
            "gemini",
            vec![
                "--agent".into(),
                args.agent_name.clone(),
                "--prompt".into(),
                args.prompt.clone(),
            ],
        ),
    };

    let mut cmd = CommandBuilder::new(program);
    for a in extra_args {
        cmd.arg(a);
    }
    if let Some(cwd) = &args.cwd {
        cmd.cwd(cwd);
    }
    if args.dry_run {
        cmd.env("SENDA_DRY_RUN", "1");
    }
    cmd.env("TERM", "xterm-256color");
    cmd
}

fn run_pty_pump(
    execution_id: String,
    app: tauri::AppHandle,
    master: Box<dyn MasterPty + Send>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>,
    db: Db,
) {
    use std::io::Read;

    let mut reader = match master.try_clone_reader() {
        Ok(r) => r,
        Err(e) => {
            emit_done(&app, &execution_id, None, Some(format!("reader: {e}")));
            return;
        }
    };

    let mut buf = [0u8; 8 * 1024];
    let output_event = format!("execution:{execution_id}:output");
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = app.emit(&output_event, OutputEvent { chunk });
            }
            Err(e) => {
                tracing::warn!(?e, "pty read error");
                break;
            }
        }
    }

    let exit_code = match child.lock().unwrap().wait() {
        Ok(status) => status.exit_code() as i32,
        Err(e) => {
            emit_done(&app, &execution_id, None, Some(format!("wait: {e}")));
            return;
        }
    };

    let ended_at = unix_now();
    if let Err(e) = db.record_end(&execution_id, ended_at, Some(exit_code)) {
        tracing::warn!(?e, "failed to record execution end");
    }

    emit_done(&app, &execution_id, Some(exit_code), None);
}

fn emit_done(
    app: &tauri::AppHandle,
    execution_id: &str,
    exit_code: Option<i32>,
    error: Option<String>,
) {
    let event = format!("execution:{execution_id}:done");
    let _ = app.emit(&event, DoneEvent { exit_code, error });
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_is_deterministic() {
        assert_eq!(sha256_hex("hello"), sha256_hex("hello"));
        assert_ne!(sha256_hex("hello"), sha256_hex("world"));
        assert_eq!(sha256_hex("hello").len(), 64);
    }
}
