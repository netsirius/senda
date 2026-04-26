//! Glue between the in-process [`senda_scheduler::Scheduler`] and the rest of
//! the Tauri app. We provide the two trait impls the scheduler needs
//! ([`Store`], [`AgentRunner`]) and helper functions to load automations from
//! SQLite at startup.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use senda_core::Automation;
use senda_scheduler::{AgentRunner, RunContext, RunOutcome, Scheduler, Store};
use tauri::{AppHandle, Emitter};

use crate::db::{Db, NewAutomationRow};

pub struct SqliteStore {
    db: Db,
}

impl SqliteStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Store for SqliteStore {
    async fn already_processed(&self, automation_id: i64, event_id: &str) -> bool {
        self.db
            .already_processed(automation_id, event_id)
            .unwrap_or(false)
    }
    async fn mark_processed(&self, automation_id: i64, event_id: &str) {
        let _ = self.db.mark_processed(automation_id, event_id, unix_now());
    }
    async fn record_run_start(&self, automation_id: i64, started_at: i64) -> i64 {
        self.db
            .record_automation_run_start(automation_id, started_at)
            .unwrap_or(0)
    }
    async fn record_run_end(&self, run_id: i64, ended_at: i64, outcome: &RunOutcome) {
        let status = if outcome.success { "success" } else { "failed" };
        let _ = self.db.record_automation_run_end(
            run_id,
            ended_at,
            status,
            outcome.output.as_deref(),
            outcome.error.as_deref(),
        );
    }
    async fn runs_last_hour(&self, automation_id: i64) -> u32 {
        self.db
            .runs_last_hour(automation_id, unix_now())
            .unwrap_or(0)
    }
}

/// Phase 3 runner — emits a `automation:fired` event so the UI can show the
/// run in real time. It does NOT spawn a CLI process yet; that integration
/// stitches together with the existing PTY runner in a follow-up. For now
/// the scheduler loop, guards and audit trail are all wired and verifiable.
pub struct EventEmittingRunner {
    pub app: AppHandle,
}

#[async_trait]
impl AgentRunner for EventEmittingRunner {
    async fn run(&self, ctx: RunContext) -> RunOutcome {
        tracing::info!(
            automation = ctx.automation_id,
            agent = %ctx.agent_id,
            event = %ctx.event_id,
            "automation fired"
        );
        let _ = self.app.emit("automation:fired", &ctx.automation_id);
        RunOutcome {
            success: true,
            output: Some(format!("[stub] would run {} with prompt", ctx.agent_id)),
            error: None,
        }
    }
}

pub fn spawn_scheduler(app: AppHandle, db: Db) -> Arc<Scheduler> {
    let runner: Arc<dyn AgentRunner> = Arc::new(EventEmittingRunner { app: app.clone() });
    let store: Arc<dyn Store> = Arc::new(SqliteStore::new(db.clone()));

    tauri::async_runtime::block_on(async move {
        let scheduler = Scheduler::new(runner, store).await.expect("scheduler init");
        scheduler.start().await.expect("scheduler start");

        // Reload automations from SQLite.
        if let Ok(rows) = db.list_automations() {
            for row in rows {
                if let Some(automation) = automation_from_row(&row) {
                    if let Err(e) = scheduler.add_automation(automation).await {
                        tracing::warn!(?e, name = %row.name, "failed to add automation");
                    }
                }
            }
        }

        // Webhook server on localhost:9876 — falls back gracefully if the port
        // is taken (the user can override via env once Phase 3 polish lands).
        let addr = "127.0.0.1:9876".parse().expect("static addr");
        if let Err(e) = scheduler.start_webhook_server(addr).await {
            tracing::warn!(?e, "webhook server failed to start");
        }

        scheduler
    })
}

pub fn save_automation_to_db(db: &Db, automation: &Automation) -> Result<i64, String> {
    let trigger_kind = senda_scheduler::trigger_kind(&automation.trigger).to_string();
    let trigger_config =
        serde_json::to_string(&automation.trigger).map_err(|e| format!("encode trigger: {e}"))?;
    let guards =
        serde_json::to_string(&automation.guards).map_err(|e| format!("encode guards: {e}"))?;
    db.insert_automation(&NewAutomationRow {
        name: &automation.name,
        agent_id: &automation.agent_id,
        trigger_kind: &trigger_kind,
        trigger_config: &trigger_config,
        guards: &guards,
        enabled: automation.enabled,
        created_at: unix_now(),
    })
    .map_err(|e| format!("db: {e}"))
}

pub fn automation_from_row(row: &crate::db::AutomationRow) -> Option<Automation> {
    let trigger = serde_json::from_str(&row.trigger_config).ok()?;
    let guards = serde_json::from_str(&row.guards).ok()?;
    Some(Automation {
        id: row.id,
        name: row.name.clone(),
        agent_id: row.agent_id.clone(),
        trigger,
        guards,
        enabled: row.enabled,
        last_run_at: row.last_run_at,
        last_run_status: row.last_run_status.clone(),
        next_run_at: None,
    })
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
