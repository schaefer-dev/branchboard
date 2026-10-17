//! Tauri shell of Branchboard.
//!
//! This crate only wires things together: it exposes commands to the
//! frontend and delegates all actual work to the core crates.

mod alerts;
mod commands;
mod dto;
mod error;
mod events;
mod state;
mod terminal;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use herdr::{Endpoint, HerdrLink};
use tauri::{Manager, RunEvent, Runtime, WindowEvent};
use tracing_subscriber::EnvFilter;
use workspace::Workspace;

use crate::alerts::SystemAlerts;
pub use crate::alerts::{Alerts, Notice, Sound};
use crate::events::TauriEvents;
use crate::state::{Alerting, AppState, Paths};

/// How often to check whether the repository on screen is due for a fetch.
const BACKGROUND_FETCH_CHECK: Duration = Duration::from_secs(60);

/// Settings that differ between the real app and tests.
#[derive(Debug, Clone, Default)]
pub struct AppOptions {
    /// Repository list to use instead of the one in the app config directory.
    pub workspace_file: Option<PathBuf>,
    /// Editor command to use instead of the detected VS Code launcher.
    pub editor: Option<PathBuf>,
    /// Socket of the herdr session to use instead of the one herdr's own
    /// environment variables and config directory point at.
    pub herdr_socket: Option<PathBuf>,
    /// Program to run in the embedded terminal instead of herdr.
    pub terminal_program: Option<PathBuf>,
    /// Receives alerts about agents instead of the system's notifications
    /// and speakers.
    pub alerts: Option<Arc<dyn Alerts>>,
    /// How long an agent's new state must hold before it is alerted about,
    /// instead of [`herdr::ALERT_DELAY`].
    pub alert_delay: Option<Duration>,
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
        editor: std::env::var_os("BRANCHBOARD_EDITOR").map(PathBuf::from),
        herdr_socket: None,
        // The Windows smoke test runs a shell in place of herdr.
        terminal_program: std::env::var_os("BRANCHBOARD_TERMINAL_PROGRAM").map(PathBuf::from),
        alerts: None,
        alert_delay: None,
    };
    build(tauri::Builder::default(), options)
        .build(tauri::generate_context!())
        .expect("error while starting Branchboard")
        .run(|app, event| {
            // End the embedded herdr client with the app; the herdr server
            // and its agents keep running.
            if let RunEvent::Exit = event
                && let Some(state) = app.try_state::<AppState>()
            {
                state.terminal.close();
            }
        });
}

/// Configures plugins, state, commands and event handlers. Generic over the
/// runtime so tests can drive the app through Tauri's mock runtime.
pub fn build<R: Runtime>(builder: tauri::Builder<R>, options: AppOptions) -> tauri::Builder<R> {
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            let file = match options.workspace_file.clone() {
                Some(file) => file,
                None => app.path().app_config_dir()?.join("workspace.json"),
            };
            tracing::info!(file = %file.display(), "opening workspace");
            let events = Arc::new(TauriEvents(app.handle().clone()));
            let version = app.package_info().version.to_string();
            let herdr_endpoint = match options.herdr_socket.clone() {
                Some(socket) => Some(Endpoint::new(socket)),
                None => Endpoint::from_env(),
            };
            let terminal_config =
                terminal::config_file(file.parent().unwrap_or_else(|| std::path::Path::new(".")));
            let alerts = match options.alerts.clone() {
                Some(alerts) => alerts,
                None => Arc::new(SystemAlerts::new(app.handle().clone())),
            };
            let state = AppState::new(
                version,
                Workspace::open(file, events.clone()),
                HerdrLink::new(herdr_endpoint, events),
                Paths {
                    editor: options.editor.clone(),
                    terminal_config,
                    terminal_program: options.terminal_program.clone(),
                },
                Alerting {
                    alerts,
                    delay: options.alert_delay.unwrap_or(herdr::ALERT_DELAY),
                },
            );
            app.manage(state.clone());

            // Discover git in the background so the window shows immediately.
            // The frontend requests the first summary refresh once it listens
            // for the results.
            let discovery = state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = discovery.git().await {
                    tracing::warn!(?error, "git is not available");
                }
            });

            // Checks once a minute whether a background fetch is due; the
            // check itself is a file timestamp read.
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(BACKGROUND_FETCH_CHECK);
                loop {
                    ticker.tick().await;
                    state.spawn_background_fetch();
                }
            });
            Ok(())
        })
        .on_window_event(on_window_event)
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
            commands::commit,
            commands::get_head_commit,
            commands::undo_last_commit,
            commands::open_in_editor,
            commands::get_branches,
            commands::get_branch_commits,
            commands::get_review_files,
            commands::get_review_diff,
            commands::switch_branch,
            commands::fetch_repo,
            commands::set_mainline,
            commands::set_fetch_interval,
            commands::set_terminal_font_size,
            commands::set_sound_muted,
            commands::set_mute_when_watching,
            commands::get_agents,
            commands::open_in_herdr,
            commands::focus_agent,
            commands::paste_to_agent,
            commands::focus_repo_in_herdr,
            commands::set_terminal_visible,
            commands::terminal_info,
            commands::terminal_open,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_close,
        ])
}

/// Window focus refreshes what may have changed while the app was in the
/// background.
fn on_window_event<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
    if let WindowEvent::Focused(focused) = event {
        let state = window.state::<AppState>();
        state.set_focused(*focused);
        if *focused {
            // herdr may have been started (or restarted) meanwhile.
            let herdr = state.herdr.clone();
            tauri::async_runtime::spawn(async move { herdr.connect().await });
            state.workspace.refresh_status();
            state.spawn_background_fetch();
            if state.take_focus_refresh() {
                state.spawn_summary_refresh(None);
            }
        }
    }
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
