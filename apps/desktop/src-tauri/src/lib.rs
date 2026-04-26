//! Tauri shell entrypoint. Phase 0 only wires a smoke-test command so the
//! frontend can prove the IPC bridge is alive. Phase 1 adds the real catalog
//! and execution commands under `commands::*`.

mod commands;

use commands::greeting::hello_world;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,senda=debug")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![hello_world])
        .setup(|_app| {
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "senda backend ready",);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running senda");
}
