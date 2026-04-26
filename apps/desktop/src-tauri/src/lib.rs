//! Tauri shell entrypoint.
//!
//! Phase 0 wired only a smoke-test command. Phase 1 added catalog reading,
//! agent execution and SQLite history. Phase 2 plugs in connected repos
//! (clone, list, sync, disconnect) and the GitHub Device Flow.

mod commands;
mod db;
mod secrets;
mod sync;

use commands::agents::read_catalog;
use commands::execution::{cancel_execution, list_executions, run_agent, Executions};
use commands::greeting::hello_world;
use commands::oauth::{github_device_authorize, github_device_poll};
use commands::repos::{add_repo, disconnect_repo, list_repos, sync_repo};

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
        ])
        .setup(move |app| {
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "senda backend ready");
            sync::spawn_background_sync(app.handle().clone(), db.clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running senda");
}
