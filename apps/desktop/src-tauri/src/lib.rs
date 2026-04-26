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
    create_automation, delete_automation, list_automation_runs, list_automations,
    run_automation_now, set_automation_enabled,
};
use commands::discovery::{list_builtin_tools, list_installed_mcps, list_skills};
use commands::editor::{delete_agent, list_drafts, read_agent_source, save_agent};
use commands::execution::{cancel_execution, list_executions, run_agent, Executions};
use commands::generate::generate_agent;
use commands::greeting::hello_world;
use commands::oauth::{github_device_authorize, github_device_poll};
use commands::publish::publish_agent;
use commands::repos::{add_repo, disconnect_repo, list_repos, sync_repo};
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
            hello_world,
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
            save_agent,
            delete_agent,
            list_drafts,
            read_agent_source,
            publish_agent,
            list_installed_mcps,
            list_builtin_tools,
            list_skills,
            generate_agent,
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
