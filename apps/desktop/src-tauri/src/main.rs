// Hide the Windows console window in release builds.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

fn main() {
    // `--headless-tick` is invoked by the OS scheduler (LaunchAgent / systemd
    // timer) every minute to fire any due cron automations without opening
    // the GUI. Keep the binary single-process — Tauri's webview doesn't load
    // here.
    if std::env::args().any(|arg| arg == "--headless-tick") {
        senda_desktop_lib::run_headless_tick();
        return;
    }
    senda_desktop_lib::run();
}
