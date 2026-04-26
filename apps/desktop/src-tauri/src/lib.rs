//! Tauri shell entrypoint.
//!
//! Phase 0 wired only a smoke-test command. Phase 1 added catalog reading,
//! agent execution and SQLite history. Phase 2 plugged in connected repos.
//! Phase 3 brings the local scheduler online (cron / webhook / manual) plus
//! an automations CRUD surface.

mod agent_runtime;
mod agent_watcher;
mod commands;
mod db;
mod mcp_client;
mod mcp_watcher;
mod scheduler_host;
mod secrets;
mod sync;

use commands::agents::read_catalog;
use commands::automations::{
    approve_pending_run, count_pending_approvals, create_automation, delete_automation,
    list_automation_runs, list_automations, list_pending_approvals, list_recent_automation_runs,
    reject_pending_run, run_automation_now, set_automation_enabled, webhook_self_test,
};
use commands::discovery::{
    add_mcp, create_skill, delete_mcp, delete_skill, introspect_mcp_tools, list_builtin_tools,
    list_installed_mcps, list_skills,
};
use commands::editor::{delete_agent, list_drafts, read_agent_source, save_agent};
use commands::execution::{
    cancel_execution, list_executions, list_executions_for_agent, run_agent, Executions,
};
use commands::generate::generate_agent;
use commands::oauth::{github_device_authorize, github_device_poll};
use commands::os_scheduler::{os_scheduler_install, os_scheduler_status, os_scheduler_uninstall};
use commands::publish::publish_agent;
use commands::repos::{add_repo, disconnect_repo, list_repos, sync_repo};
use commands::system::reveal_in_finder;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,senda=debug")),
        )
        .init();

    let db = db::Db::open().expect("open senda data.db");
    let executions = Executions::default();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(db.clone())
        .manage(executions)
        .invoke_handler(tauri::generate_handler![
            read_catalog,
            run_agent,
            cancel_execution,
            list_executions,
            add_repo,
            list_repos,
            disconnect_repo,
            sync_repo,
            github_device_authorize,
            github_device_poll,
            list_automations,
            create_automation,
            delete_automation,
            set_automation_enabled,
            run_automation_now,
            list_automation_runs,
            list_recent_automation_runs,
            list_pending_approvals,
            count_pending_approvals,
            approve_pending_run,
            reject_pending_run,
            webhook_self_test,
            list_executions_for_agent,
            reveal_in_finder,
            save_agent,
            delete_agent,
            list_drafts,
            read_agent_source,
            publish_agent,
            list_installed_mcps,
            list_builtin_tools,
            list_skills,
            generate_agent,
            add_mcp,
            delete_mcp,
            delete_skill,
            create_skill,
            introspect_mcp_tools,
            os_scheduler_status,
            os_scheduler_install,
            os_scheduler_uninstall,
        ])
        .setup(move |app| {
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "senda backend ready");
            let handle = app.handle().clone();
            sync::spawn_background_sync(handle.clone(), db.clone());
            agent_watcher::spawn_agent_watcher(handle.clone());
            // Scheduler boots inside setup so the runner has a real AppHandle.
            let scheduler = scheduler_host::spawn_scheduler(handle, db.clone());
            app.manage(scheduler);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running senda");
}

/// Headless one-shot tick fired by the OS scheduler (LaunchAgent / systemd).
///
/// We don't open a Tauri window; we just inspect the automations table and
/// spawn the agent CLI for any cron entry whose next firing time is past.
/// This keeps cron usable when the user has Senda closed.
pub fn run_headless_tick() {
    use chrono::Utc;
    use senda_core::Trigger;
    use std::time::Duration;
    use tokio::runtime::Builder;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,senda=info")),
        )
        .init();

    let Ok(db) = db::Db::open() else {
        eprintln!("senda: could not open data.db; skipping headless tick");
        return;
    };
    let Ok(rows) = db.list_automations() else {
        eprintln!("senda: could not read automations; skipping headless tick");
        return;
    };

    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    runtime.block_on(async move {
        let now = Utc::now();
        for row in rows {
            if !row.enabled || row.trigger_kind != "schedule" {
                continue;
            }
            let Some(automation) = scheduler_host::automation_from_row(&row) else {
                continue;
            };
            let Trigger::Schedule { cron: expr, .. } = &automation.trigger else {
                continue;
            };
            let Ok(schedule) = cron::Schedule::try_from(expr.as_str()) else {
                continue;
            };
            // Take the most recent past firing for this cron and check whether
            // it's after `last_run_at`. That handles a tick happening late.
            let last_run = automation.last_run_at.unwrap_or(0);
            let last_run_dt =
                chrono::DateTime::<Utc>::from_timestamp(last_run, 0).unwrap_or_default();
            let due = schedule
                .after(&last_run_dt)
                .next()
                .map(|t| t <= now)
                .unwrap_or(false);
            if !due {
                continue;
            }
            let event_id = format!("os-tick-{}", now.timestamp());
            tracing::info!(
                automation = automation.id,
                "headless tick firing automation"
            );
            let prompt = automation
                .prompt_template
                .clone()
                .unwrap_or_else(|| "scheduled trigger".into());
            let agent_id = automation.agent_id.clone();
            let aut_id = automation.id;
            // Run the CLI synchronously per automation; we don't need
            // concurrency in a 60s tick.
            let resolved = agent_runtime::resolve_agent(&db, &agent_id);
            let started_at = now.timestamp();
            let run_id = db
                .record_automation_run_start(aut_id, started_at)
                .unwrap_or(0);
            let outcome = match resolved {
                Some((cli, name)) => {
                    let r = agent_runtime::spawn_for_automation(
                        cli,
                        &name,
                        &prompt,
                        false,
                        Duration::from_secs(15 * 60),
                    )
                    .await;
                    senda_scheduler::RunOutcome {
                        success: r.success,
                        output: Some(r.output),
                        error: r.error,
                    }
                }
                None => senda_scheduler::RunOutcome {
                    success: false,
                    output: None,
                    error: Some(format!("agent `{agent_id}` not found")),
                },
            };
            let ended_at = Utc::now().timestamp();
            let status = if outcome.success { "success" } else { "failed" };
            let _ = db.record_automation_run_end(
                run_id,
                ended_at,
                status,
                outcome.output.as_deref(),
                outcome.error.as_deref(),
            );
            let _ = db.mark_processed(aut_id, &event_id, ended_at);
        }
    });
}
