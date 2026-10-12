//! Tauri shell of Branchboard.
//!
//! This crate only wires things together: it exposes commands to the
//! frontend and delegates all actual work to the core crates.

mod commands;
mod dto;
mod error;
mod events;
mod state;

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{Manager, Runtime, WindowEvent};
use tracing_subscriber::EnvFilter;
use workspace::Workspace;

use crate::events::TauriEvents;
use crate::state::AppState;

/// Settings that differ between the real app and tests.
#[derive(Debug, Clone, Default)]
pub struct AppOptions {
    /// Repository list to use instead of the one in the app config directory.
    pub workspace_file: Option<PathBuf>,
}

/// Starts the application and blocks until it exits.
///
/// # Panics
///
/// Panics if the Tauri runtime fails to start.
pub fn run() {
    init_tracing();
    // `BRANCHBOARD_WORKSPACE_FILE` points development and test runs at a
    // separate repository list.
    let options = AppOptions {
        workspace_file: std::env::var_os("BRANCHBOARD_WORKSPACE_FILE").map(PathBuf::from),
    };
    build(tauri::Builder::default(), options)
        .run(tauri::generate_context!())
        .expect("error while running Branchboard");
}

/// Configures plugins, state, commands and event handlers. Generic over the
/// runtime so tests can drive the app through Tauri's mock runtime.
pub fn build<R: Runtime>(builder: tauri::Builder<R>, options: AppOptions) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let file = match options.workspace_file.clone() {
                Some(file) => file,
                None => app.path().app_config_dir()?.join("workspace.json"),
            };
            tracing::info!(file = %file.display(), "opening workspace");
            let events = Arc::new(TauriEvents(app.handle().clone()));
            let version = app.package_info().version.to_string();
            let state = AppState::new(version, Workspace::open(file, events));
            app.manage(state.clone());

            // Discover git in the background so the window shows immediately.
            // The frontend requests the first summary refresh once it listens
            // for the results.
            tauri::async_runtime::spawn(async move {
                if let Err(error) = state.git().await {
                    tracing::warn!(?error, "git is not available");
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Focused(true) = event {
                let state = window.state::<AppState>();
                state.workspace.refresh_status();
                if state.take_focus_refresh() {
                    state.spawn_summary_refresh(None);
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
            commands::get_status,
            commands::get_diff,
            commands::stage_paths,
            commands::unstage_paths,
            commands::discard_paths,
        ])
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
