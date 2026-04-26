//! Tauri commands for automations: list / create / delete / pause-resume /
//! run-now. Persistence lives in [`crate::db`]; the runtime scheduler is
//! threaded through Tauri state.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use senda_core::{Automation, Guards, Trigger};
use senda_scheduler::Scheduler;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::agent_runtime::{resolve_agent, spawn_for_automation};
use crate::db::{AutomationRow, AutomationRunRow, Db, PendingRun};
use crate::scheduler_host::{automation_from_row, save_automation_to_db};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateAutomationArgs {
    pub name: String,
    pub agent_id: String,
    pub trigger: Trigger,
    pub guards: Guards,
    #[serde(default)]
    pub prompt_template: Option<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[tauri::command]
pub async fn list_automations(db: tauri::State<'_, Db>) -> Result<Vec<AutomationRow>, String> {
    db.list_automations().map_err(|e| format!("db: {e}"))
}

#[tauri::command]
pub async fn create_automation(
    db: tauri::State<'_, Db>,
    scheduler: tauri::State<'_, Arc<Scheduler>>,
    app: tauri::AppHandle,
    args: CreateAutomationArgs,
) -> Result<i64, String> {
    let mut automation = Automation {
        id: 0,
        name: args.name,
        agent_id: args.agent_id,
        trigger: args.trigger,
        guards: args.guards,
        prompt_template: args.prompt_template,
        enabled: args.enabled,
        last_run_at: None,
        last_run_status: None,
        next_run_at: None,
    };
    let id = save_automation_to_db(&db, &automation)?;
    automation.id = id;
    scheduler
        .add_automation(automation)
        .await
        .map_err(|e| format!("scheduler: {e}"))?;
    let _ = app.emit("automations:changed", ());
    Ok(id)
}

#[tauri::command]
pub async fn delete_automation(
    db: tauri::State<'_, Db>,
    scheduler: tauri::State<'_, Arc<Scheduler>>,
    app: tauri::AppHandle,
    id: i64,
) -> Result<(), String> {
    let _ = scheduler.remove_automation(id).await;
    db.delete_automation(id).map_err(|e| format!("db: {e}"))?;
    let _ = app.emit("automations:changed", ());
    Ok(())
}

#[tauri::command]
pub async fn set_automation_enabled(
    db: tauri::State<'_, Db>,
    scheduler: tauri::State<'_, Arc<Scheduler>>,
    app: tauri::AppHandle,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    db.set_automation_enabled(id, enabled)
        .map_err(|e| format!("db: {e}"))?;
    if !enabled {
        let _ = scheduler.remove_automation(id).await;
    } else if let Some(row) = db
        .list_automations()
        .map_err(|e| format!("db: {e}"))?
        .into_iter()
        .find(|r| r.id == id)
    {
        if let Some(automation) = automation_from_row(&row) {
            scheduler
                .add_automation(automation)
                .await
                .map_err(|e| format!("scheduler: {e}"))?;
        }
    }
    let _ = app.emit("automations:changed", ());
    Ok(())
}

#[tauri::command]
pub async fn run_automation_now(
    scheduler: tauri::State<'_, Arc<Scheduler>>,
    id: i64,
    dry_run: Option<bool>,
) -> Result<(), String> {
    scheduler
        .run_now(id, dry_run.unwrap_or(false))
        .await
        .map_err(|e| format!("scheduler: {e}"))
}

#[tauri::command]
pub async fn list_automation_runs(
    db: tauri::State<'_, Db>,
    automation_id: i64,
    limit: Option<i64>,
) -> Result<Vec<AutomationRunRow>, String> {
    db.list_automation_runs(automation_id, limit.unwrap_or(50))
        .map_err(|e| format!("db: {e}"))
}

#[tauri::command]
pub async fn list_recent_automation_runs(
    db: tauri::State<'_, Db>,
    limit: Option<i64>,
) -> Result<Vec<AutomationRunRow>, String> {
    db.list_recent_automation_runs(limit.unwrap_or(100))
        .map_err(|e| format!("db: {e}"))
}

#[tauri::command]
pub async fn list_pending_approvals(db: tauri::State<'_, Db>) -> Result<Vec<PendingRun>, String> {
    db.list_pending_runs().map_err(|e| format!("db: {e}"))
}

#[tauri::command]
pub async fn count_pending_approvals(db: tauri::State<'_, Db>) -> Result<u32, String> {
    db.count_pending_runs().map_err(|e| format!("db: {e}"))
}

#[tauri::command]
pub async fn reject_pending_run(
    db: tauri::State<'_, Db>,
    app: AppHandle,
    run_id: i64,
) -> Result<(), String> {
    db.mark_pending_run_status(run_id, "cancelled")
        .map_err(|e| format!("db: {e}"))?;
    let _ = app.emit("approvals:changed", ());
    Ok(())
}

/// Approve a pending run and dispatch the agent against the queued prompt.
/// Bypasses idempotency / rate-limit (the original fire() already cleared
/// those guards) and writes the outcome straight into the same row.
#[tauri::command]
pub async fn approve_pending_run(
    db: tauri::State<'_, Db>,
    app: AppHandle,
    run_id: i64,
) -> Result<(), String> {
    let pending = db
        .take_pending_run(run_id)
        .map_err(|e| format!("db: {e}"))?
        .ok_or_else(|| format!("run {run_id} not pending"))?;

    db.mark_pending_run_status(run_id, "running")
        .map_err(|e| format!("db: {e}"))?;
    let _ = app.emit("approvals:changed", ());

    let resolved = resolve_agent(db.inner(), &pending.agent_id);
    let outcome = match resolved {
        Some((cli, name)) => {
            let r = spawn_for_automation(
                cli,
                &name,
                &pending.prompt,
                false,
                Duration::from_secs(15 * 60),
            )
            .await;
            (r.success, Some(r.output), r.error)
        }
        None => (
            false,
            None,
            Some(format!("agent `{}` not found", pending.agent_id)),
        ),
    };

    let ended_at = unix_now();
    let status = if outcome.0 { "success" } else { "failed" };
    db.record_automation_run_end(
        run_id,
        ended_at,
        status,
        outcome.1.as_deref(),
        outcome.2.as_deref(),
    )
    .map_err(|e| format!("db: {e}"))?;
    let _ = app.emit("automation:fired", &pending.automation_id);
    Ok(())
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Fire a synthetic POST against the local webhook server. Useful for testing
/// a freshly-created webhook automation without leaving the app.
#[tauri::command]
pub async fn webhook_self_test(path: String, body: Option<String>) -> Result<u16, String> {
    let url = format!("http://127.0.0.1:9876/hook/{path}");
    let body = body.unwrap_or_else(|| "{\"source\":\"senda-self-test\"}".to_string());
    let resp = reqwest::Client::new()
        .post(&url)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("self-test: {e}"))?;
    Ok(resp.status().as_u16())
}
