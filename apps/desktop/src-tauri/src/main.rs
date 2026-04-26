// Hide the Windows console window in release builds.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

fn main() {
    senda_desktop_lib::run();
}
