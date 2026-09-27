//! X3 Atomic Star OS — the operator console backend.
//!
//! `main.rs` is the desktop entry point and does nothing but call [`run`]. Every
//! read lives in a module that a test can reach:
//!
//! * [`chain`] — JSON-RPC reads against the node the operator is running;
//! * [`swarm`] — the task queue served by `services/x3-swarm-api`;
//! * [`probe`] — reachability of the local lane services;
//! * [`commands`] — the command bodies, each taking the client it needs;
//! * [`monitor`] — the background tick that pushes status to the panels;
//! * [`error`] / [`models`] — the IPC contract with the webview.
//!
//! The rule the whole crate is built around: if the console cannot prove an
//! answer, it returns the typed reason it could not, and never a value that
//! looks live.

pub mod chain;
pub mod commands;
pub mod error;
pub mod models;
pub mod monitor;
pub mod probe;
pub mod state;
pub mod swarm;

use state::ConsoleState;

/// Build and run the console.
pub fn run() {
    let state = ConsoleState::from_env();
    let monitor_state = state.clone();

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_notification::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_node_status,
            commands::get_validator_health,
            commands::get_system_metrics,
            commands::swarm_get_tasks,
            commands::swarm_get_health,
            commands::swarm_approve_task,
            commands::swarm_reject_task,
            commands::inferstructor_check_services,
        ])
        .setup(move |app| {
            monitor::spawn(app.handle().clone(), monitor_state);
            Ok(())
        })
        .run(tauri::generate_context!());

    // The console is the whole process: there is nothing left to do if the
    // event loop cannot start, but it is still reported rather than swallowed.
    if let Err(error) = result {
        eprintln!("x3-os: the operator console could not start: {error}");
        std::process::exit(1);
    }
}
