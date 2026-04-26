//! Tauri shell entrypoint.
//!
//! Phase 0 wired only a smoke-test command. Phase 1 adds catalog reading,
//! agent execution and a SQLite-backed history. Tauri state holds the DB
//! handle and the in-flight executions registry.

mod commands;
mod db;

use commands::agents::read_catalog;
use commands::execution::{cancel_execution, list_executions, run_agent, Executions};
use commands::greeting::hello_world;

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
        .manage(db)
        .manage(executions)
        .invoke_handler(tauri::generate_handler![
            hello_world,
            read_catalog,
            run_agent,
            cancel_execution,
            list_executions,
        ])
        .setup(|_app| {
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "senda backend ready");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running senda");
}
