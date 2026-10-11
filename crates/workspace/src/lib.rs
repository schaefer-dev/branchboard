//! The user's set of repositories: registry, persistence and background
//! summaries. Independent of Tauri; events leave through [`SummarySink`].

mod error;
mod registry;
mod snapshot;
mod store;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use git_core::Git;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

pub use error::WorkspaceError;
pub use registry::{Added, Group, GroupId, Registry, RepoEntry, RepoId};
pub use snapshot::{RepoView, SummaryState, SummaryUpdate, WorkspaceSnapshot};

/// How many `git status` processes run at once when refreshing summaries.
const MAX_PARALLEL_SUMMARIES: usize = 4;

/// Receives summary updates as they complete.
pub trait SummarySink: Send + Sync + 'static {
    fn publish(&self, update: SummaryUpdate);
}

pub struct Workspace {
    store: store::Store,
    registry: Mutex<Registry>,
}

impl Workspace {
    /// Opens the workspace persisted at `path` (created on first save).
    pub fn open(path: PathBuf) -> Self {
        let store = store::Store::new(path);
        let registry = store.load();
        Self {
            store,
            registry: Mutex::new(registry),
        }
    }

    pub async fn snapshot(&self) -> WorkspaceSnapshot {
        WorkspaceSnapshot::of(&*self.registry.lock().await)
    }

    /// Applies a change and persists it. The change is applied to a copy, so
    /// a failed save leaves both memory and disk untouched.
    pub async fn update<T>(
        &self,
        change: impl FnOnce(&mut Registry) -> Result<T, WorkspaceError>,
    ) -> Result<(T, WorkspaceSnapshot), WorkspaceError> {
        let mut registry = self.registry.lock().await;
        let mut next = registry.clone();
        let value = change(&mut next)?;
        if next != *registry {
            self.store.save(&next).await.map_err(WorkspaceError::Save)?;
            *registry = next;
        }
        Ok((value, WorkspaceSnapshot::of(&registry)))
    }

    /// Adds the repository containing `path` and makes it the active one.
    pub async fn add_repo(
        &self,
        git: &Git,
        path: &Path,
    ) -> Result<(Added, WorkspaceSnapshot), WorkspaceError> {
        let work_tree = normalize(&git.resolve_work_tree(path).await?);
        self.update(|registry| {
            let added = registry.add_repo(work_tree);
            registry.set_active_repo(Some(added.id()))?;
            Ok(added)
        })
        .await
    }

    /// Points a (missing) repository at its new location.
    pub async fn relocate_repo(
        &self,
        git: &Git,
        id: &RepoId,
        path: &Path,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let work_tree = normalize(&git.resolve_work_tree(path).await?);
        let ((), snapshot) = self
            .update(|registry| registry.relocate_repo(id, work_tree))
            .await?;
        Ok(snapshot)
    }

    /// Refreshes the summaries of all repositories, or only of `only`, and
    /// publishes each result as soon as it is ready.
    pub async fn refresh_summaries(
        &self,
        git: &Git,
        only: Option<&RepoId>,
        sink: &Arc<dyn SummarySink>,
    ) {
        let targets: Vec<(RepoId, PathBuf)> = self
            .registry
            .lock()
            .await
            .repos
            .iter()
            .filter(|repo| only.is_none_or(|id| &repo.id == id))
            .map(|repo| (repo.id.clone(), repo.path.clone()))
            .collect();

        let limit = Arc::new(Semaphore::new(MAX_PARALLEL_SUMMARIES));
        let mut tasks = JoinSet::new();
        for (repo_id, path) in targets {
            let (git, sink, limit) = (git.clone(), Arc::clone(sink), Arc::clone(&limit));
            tasks.spawn(async move {
                let Ok(_permit) = limit.acquire_owned().await else {
                    return;
                };
                let state = summarize(&git, &path).await;
                sink.publish(SummaryUpdate { repo_id, state });
            });
        }
        while tasks.join_next().await.is_some() {}
    }
}

async fn summarize(git: &Git, path: &Path) -> SummaryState {
    if snapshot::is_missing(path) {
        return SummaryState::Missing;
    }
    match git.summary(path).await {
        Ok(summary) => SummaryState::Ready { summary },
        Err(error) => {
            tracing::warn!(?error, path = %path.display(), "summary failed");
            SummaryState::Failed {
                message: error.to_string(),
            }
        }
    }
}

/// Canonical form of a repository path, so the same folder is never
/// registered twice. `dunce` avoids `\\?\` prefixes on Windows.
fn normalize(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_owned())
}
