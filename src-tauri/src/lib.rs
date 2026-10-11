//! Tauri shell of Branchboard.
//!
//! This crate only wires things together: it exposes commands to the
//! frontend and delegates all actual work to the core crates.

mod commands;
mod dto;
mod error;
mod state;

use tauri::Manager;
use tracing_subscriber::EnvFilter;

use crate::state::AppState;

/// Starts the application and blocks until it exits.
///
/// # Panics
///
/// Panics if the Tauri runtime fails to start.
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .manage(AppState::default())
        .setup(|app| {
            // Discover git in the background so the window shows immediately;
            // the first command that needs git awaits the result.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = handle.state::<AppState>().git().await {
                    tracing::warn!(?error, "git is not available");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_git_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Branchboard");
}

fn init_tracing() {
    let default_filter = if cfg!(debug_assertions) {
        "info,git_core=debug,branchboard_lib=debug"
    } else {
        "warn"
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
