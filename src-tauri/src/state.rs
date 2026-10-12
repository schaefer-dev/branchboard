use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use git_core::{DiscoveryOptions, Git};
use tokio::sync::OnceCell;
use workspace::{RepoId, Workspace};

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
}

impl Deref for AppState {
    type Target = Inner;

    fn deref(&self) -> &Inner {
        &self.0
    }
}

impl AppState {
    pub fn new(app_version: String, workspace: Workspace) -> Self {
        Self(Arc::new(Inner {
            app_version,
            workspace,
            git: OnceCell::new(),
            last_focus_refresh: Mutex::new(None),
        }))
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
