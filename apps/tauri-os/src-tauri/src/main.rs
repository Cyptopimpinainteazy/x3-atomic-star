//! Desktop entry point. All of the console lives in the library so that it can
//! be tested without a webview (`tests/` links against `tauri_os_backend`).

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    tauri_os_backend::run();
}
