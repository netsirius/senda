//! Tauri commands for automations: list / create / delete / pause-resume /
//! run-now. Persistence lives in [`crate::db`]; the runtime scheduler is
//! threaded through Tauri state.

use std::sync::Arc;

use senda_core::{Automation, Guards, Trigger};
use senda_scheduler::Scheduler;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::db::{AutomationRow, AutomationRunRow, Db};
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
