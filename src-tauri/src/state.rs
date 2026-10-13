use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use git_core::{DiscoveryOptions, Git};
use std::path::PathBuf;
use tokio::sync::OnceCell;

use workspace::{Editor, RepoId, Workspace, WorkspaceError};

use crate::error::AppError;

/// Window focus refreshes all summaries, but at most this often.
const FOCUS_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

/// Application-wide state managed by Tauri. Cheap to clone, so background
/// tasks can own a handle.
#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub app_version: String,
    pub workspace: Workspace,
    git: OnceCell<Result<Git, AppError>>,
    last_focus_refresh: Mutex<Option<Instant>>,
    /// Background fetches only happen while the window has focus.
    focused: AtomicBool,
    editor_override: Option<PathBuf>,
}

impl Deref for AppState {
    type Target = Inner;

    fn deref(&self) -> &Inner {
        &self.0
    }
}

impl AppState {
    pub fn new(
        app_version: String,
        workspace: Workspace,
        editor_override: Option<PathBuf>,
    ) -> Self {
        Self(Arc::new(Inner {
            app_version,
            workspace,
            git: OnceCell::new(),
            last_focus_refresh: Mutex::new(None),
            focused: AtomicBool::new(true),
            editor_override,
        }))
    }

    pub fn set_focused(&self, focused: bool) {
        self.focused.store(focused, Ordering::Relaxed);
    }

    /// Fetches the repository on screen in the background if background
    /// fetching is on, the window has focus and the last fetch is older
    /// than the configured interval.
    pub fn spawn_background_fetch(&self) {
        if !self.focused.load(Ordering::Relaxed) {
            return;
        }
        let state = self.clone();
        tauri::async_runtime::spawn(async move {
            let minutes = state.workspace.settings().await.fetch_interval_minutes;
            if minutes == 0 {
                return;
            }
            if let Ok(git) = state.git().await {
                let max_age = Duration::from_secs(u64::from(minutes) * 60);
                state.workspace.fetch_if_stale(git, max_age).await;
            }
        });
    }

    /// Refreshes repository summaries (all, or only `only`) in the
    /// background. Results arrive as `repo-summary` events.
    pub fn spawn_summary_refresh(&self, only: Option<RepoId>) {
        let state = self.clone();
        tauri::async_runtime::spawn(async move {
            match state.git().await {
                Ok(git) => state.workspace.refresh_summaries(git, only.as_ref()).await,
                Err(error) => tracing::debug!(?error, "skipping summary refresh without git"),
            }
        });
    }
}

impl Inner {
    /// The git installation, discovered once on first access.
    pub async fn git(&self) -> Result<&Git, AppError> {
        self.git
            .get_or_init(|| async {
                Git::discover(DiscoveryOptions::default())
                    .await
                    .map_err(AppError::from)
            })
            .await
            .as_ref()
            .map_err(Clone::clone)
    }

    /// The editor to open files in: the override, or VS Code found on the
    /// same search path as git.
    pub async fn editor(&self) -> Result<Editor, AppError> {
        if let Some(program) = &self.editor_override {
            return Ok(Editor::custom(program.clone()));
        }
        let git = self.git().await?;
        Editor::detect(git.runner().search_path())
            .ok_or_else(|| WorkspaceError::EditorNotFound.into())
    }

    /// Whether a focus-triggered refresh is due; records it if so.
    pub fn take_focus_refresh(&self) -> bool {
        let mut last = self
            .last_focus_refresh
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let due = last.is_none_or(|at| at.elapsed() >= FOCUS_REFRESH_INTERVAL);
        if due {
            *last = Some(Instant::now());
        }
        due
    }
}
