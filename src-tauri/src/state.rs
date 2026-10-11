use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use git_core::{DiscoveryOptions, Git};
use tokio::sync::OnceCell;
use workspace::{RepoId, SummarySink, Workspace};

use crate::error::AppError;

/// Window focus refreshes all summaries, but at most this often.
const FOCUS_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

/// Application-wide state managed by Tauri.
pub struct AppState {
    git: OnceCell<Result<Git, AppError>>,
    pub workspace: Workspace,
    summaries: Arc<dyn SummarySink>,
    last_focus_refresh: Mutex<Option<Instant>>,
}

impl AppState {
    pub fn new(workspace: Workspace, summaries: Arc<dyn SummarySink>) -> Self {
        Self {
            git: OnceCell::new(),
            workspace,
            summaries,
            last_focus_refresh: Mutex::new(None),
        }
    }

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

    /// Refreshes repository summaries (all, or only `only`). Results arrive
    /// as `repo-summary` events.
    pub async fn refresh_summaries(&self, only: Option<&RepoId>) {
        match self.git().await {
            Ok(git) => {
                self.workspace
                    .refresh_summaries(git, only, &self.summaries)
                    .await;
            }
            Err(error) => tracing::debug!(?error, "skipping summary refresh without git"),
        }
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
