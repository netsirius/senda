//! Glue between the in-process [`senda_scheduler::Scheduler`] and the rest of
//! the Tauri app. Provides the two trait impls the scheduler needs
//! ([`Store`], [`AgentRunner`]) and bootstraps automations from SQLite.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use senda_core::Automation;
use senda_scheduler::{AgentRunner, RunContext, RunOutcome, Scheduler, Store};
use tauri::{AppHandle, Emitter};

use crate::agent_runtime::{resolve_agent, spawn_for_automation};
use crate::db::{Db, NewAutomationRow};

/// Worst-case wall clock for a single automation run. Anything longer is
/// almost always a stuck CLI; the scheduler kills it and records the timeout.
const RUN_TIMEOUT: Duration = Duration::from_secs(15 * 60);

pub struct SqliteStore {
    db: Db,
    app: AppHandle,
}

impl SqliteStore {
    pub fn new(db: Db, app: AppHandle) -> Self {
        Self { db, app }
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
    async fn record_pending_run(&self, automation_id: i64, started_at: i64, prompt: &str) -> i64 {
        let id = self
            .db
            .record_pending_run(automation_id, started_at, prompt)
            .unwrap_or(0);
        let _ = self.app.emit("approvals:changed", ());
        id
    }
}

/// Production runner: resolves the agent from the catalog, spawns the
/// matching CLI without a PTY, captures stdout+stderr, and emits a
/// `automation:fired` event so the UI can refresh the runs list.
pub struct CliBackedRunner {
    pub app: AppHandle,
    pub db: Db,
}

#[async_trait]
impl AgentRunner for CliBackedRunner {
    async fn run(&self, ctx: RunContext) -> RunOutcome {
        tracing::info!(
            automation = ctx.automation_id,
            agent = %ctx.agent_id,
            event = %ctx.event_id,
            "automation fired"
        );

        let resolved = resolve_agent(&self.db, &ctx.agent_id);
        let Some((cli, agent_name)) = resolved else {
            return RunOutcome {
                success: false,
                output: None,
                error: Some(format!(
                    "agent `{}` not found — was it deleted?",
                    ctx.agent_id
                )),
            };
        };

        let result =
            spawn_for_automation(cli, &agent_name, &ctx.prompt, ctx.dry_run, RUN_TIMEOUT).await;
        let _ = self.app.emit("automation:fired", &ctx.automation_id);

        if let Some(err) = &result.error {
            return RunOutcome {
                success: false,
                output: if result.output.is_empty() {
                    None
                } else {
                    Some(result.output)
                },
                error: Some(err.clone()),
            };
        }

        RunOutcome {
            success: result.success,
            output: Some(result.output),
            error: result.exit_code.and_then(|code| {
                if code == 0 {
                    None
                } else {
                    Some(format!("exit code {code}"))
                }
            }),
        }
    }
}

pub fn spawn_scheduler(app: AppHandle, db: Db) -> Arc<Scheduler> {
    let runner: Arc<dyn AgentRunner> = Arc::new(CliBackedRunner {
        app: app.clone(),
        db: db.clone(),
    });
    let store: Arc<dyn Store> = Arc::new(SqliteStore::new(db.clone(), app.clone()));

    tauri::async_runtime::block_on(async move {
        let scheduler = Scheduler::new(runner, store).await.expect("scheduler init");
        scheduler.start().await.expect("scheduler start");

        if let Ok(rows) = db.list_automations() {
            for row in rows {
                if let Some(automation) = automation_from_row(&row) {
                    if let Err(e) = scheduler.add_automation(automation).await {
                        tracing::warn!(?e, name = %row.name, "failed to add automation");
                    }
                }
            }
        }

        let addr = "127.0.0.1:9876".parse().expect("static addr");
        if let Err(e) = scheduler.start_webhook_server(addr).await {
            tracing::warn!(?e, "webhook server failed to start");
        }

        // Phase B: spawn the MCP event-trigger watcher loop.
        crate::mcp_watcher::spawn_event_watchers(app.clone(), db.clone(), Arc::clone(&scheduler));

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
        prompt_template: automation.prompt_template.as_deref(),
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
        prompt_template: row.prompt_template.clone(),
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
