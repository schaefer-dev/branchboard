//! Tauri shell of Branchboard.
//!
//! This crate only wires things together: it exposes commands to the
//! frontend and delegates all actual work to the core crates.

mod commands;
mod dto;
mod error;
mod events;
mod state;

use std::sync::Arc;

use tauri::{Manager, WindowEvent};
use tracing_subscriber::EnvFilter;
use workspace::Workspace;

use crate::events::TauriSummarySink;
use crate::state::AppState;

/// Starts the application and blocks until it exits.
///
/// # Panics
///
/// Panics if the Tauri runtime fails to start.
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // `BRANCHBOARD_WORKSPACE_FILE` points development and test runs at a
            // separate repository list.
            let file = match std::env::var_os("BRANCHBOARD_WORKSPACE_FILE") {
                Some(file) => file.into(),
                None => app.path().app_config_dir()?.join("workspace.json"),
            };
            tracing::info!(file = %file.display(), "opening workspace");
            let sink = Arc::new(TauriSummarySink(app.handle().clone()));
            app.manage(AppState::new(Workspace::open(file), sink));

            // Discover git in the background so the window shows immediately.
            // The frontend requests the first summary refresh once it listens
            // for the results.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = handle.state::<AppState>().git().await {
                    tracing::warn!(?error, "git is not available");
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(true) = event {
                let state = window.state::<AppState>();
                if state.take_focus_refresh() {
                    commands::spawn_summary_refresh(window.app_handle(), None);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_git_info,
            commands::get_workspace,
            commands::add_repo,
            commands::relocate_repo,
            commands::remove_repo,
            commands::rename_repo,
            commands::set_repo_favorite,
            commands::set_repo_group,
            commands::set_active_repo,
            commands::create_group,
            commands::rename_group,
            commands::delete_group,
            commands::set_group_collapsed,
            commands::refresh_summaries,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Branchboard");
}

fn init_tracing() {
    let default_filter = if cfg!(debug_assertions) {
        "info,git_core=debug,workspace=debug,branchboard_lib=debug"
    } else {
        "warn"
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
