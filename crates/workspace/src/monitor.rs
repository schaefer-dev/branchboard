//! Keeps the status of the repository on screen up to date.
//!
//! Only one repository is watched at a time: the one the user is looking at.
//! File system events are debounced and coalesced into at most one pending
//! refresh, and results are only published when the status actually changed.
//! The last status of every repository is cached so switching back shows
//! something immediately.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::Duration;

use git_core::{Git, GitError, RepoSummary, StatusSnapshot};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::snapshot::{StatusUpdate, SummaryState, SummaryUpdate};
use crate::{EventSink, RepoId};

/// Quiet period after a file system event before the status is read.
const DEBOUNCE: Duration = Duration::from_millis(150);
/// Minimum time between two watcher-triggered refreshes.
const MIN_INTERVAL: Duration = Duration::from_millis(300);

pub(crate) struct Monitor {
    sink: Arc<dyn EventSink>,
    cache: Mutex<HashMap<RepoId, StatusSnapshot>>,
    watched: Mutex<Option<Watched>>,
    /// Held while the index or working tree is being changed, so refreshes
    /// never observe a half-finished operation.
    pub(crate) writes: tokio::sync::Mutex<()>,
}

struct Watched {
    repo_id: RepoId,
    wake: Arc<Notify>,
    task: JoinHandle<()>,
    _watcher: Option<RecommendedWatcher>,
}

impl Drop for Watched {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Monitor {
    pub(crate) fn new(sink: Arc<dyn EventSink>) -> Arc<Self> {
        Arc::new(Self {
            sink,
            cache: Mutex::new(HashMap::new()),
            watched: Mutex::new(None),
            writes: tokio::sync::Mutex::new(()),
        })
    }

    pub(crate) fn cached(&self, repo_id: &RepoId) -> Option<StatusSnapshot> {
        lock(&self.cache).get(repo_id).cloned()
    }

    /// Starts watching `repo_id` unless it is watched already. Any previously
    /// watched repository stops being watched.
    pub(crate) fn watch(self: &Arc<Self>, git: &Git, repo_id: &RepoId, path: &Path) {
        let mut watched = lock(&self.watched);
        if watched.as_ref().is_some_and(|w| &w.repo_id == repo_id) {
            return;
        }
        let wake = Arc::new(Notify::new());
        let fs_watcher = start_watcher(path, Arc::clone(&wake));
        let task = tokio::spawn(refresh_loop(
            Arc::downgrade(self),
            git.clone(),
            repo_id.clone(),
            path.to_owned(),
            Arc::clone(&wake),
        ));
        tracing::debug!(path = %path.display(), "watching repository");
        *watched = Some(Watched {
            repo_id: repo_id.clone(),
            wake,
            task,
            _watcher: fs_watcher,
        });
    }

    /// The repository currently watched, i.e. the one on screen.
    pub(crate) fn watched(&self) -> Option<RepoId> {
        lock(&self.watched).as_ref().map(|w| w.repo_id.clone())
    }

    /// Requests a refresh of the watched repository.
    pub(crate) fn poke(&self) {
        if let Some(watched) = lock(&self.watched).as_ref() {
            watched.wake.notify_one();
        }
    }

    /// Stops watching and forgets the cached status of `repo_id`.
    pub(crate) fn forget(&self, repo_id: &RepoId) {
        lock(&self.cache).remove(repo_id);
        let mut watched = lock(&self.watched);
        if watched.as_ref().is_some_and(|w| &w.repo_id == repo_id) {
            *watched = None;
        }
    }

    /// Reads the status, caches it and publishes it if it changed.
    pub(crate) async fn refresh(
        &self,
        git: &Git,
        repo_id: &RepoId,
        path: &Path,
    ) -> Result<StatusSnapshot, GitError> {
        let status = git.status(path).await?;
        self.publish(repo_id, &status);
        Ok(status)
    }

    fn publish(&self, repo_id: &RepoId, status: &StatusSnapshot) {
        let previous = lock(&self.cache).insert(repo_id.clone(), status.clone());
        if previous.as_ref() == Some(status) {
            return;
        }
        self.sink.status(StatusUpdate {
            repo_id: repo_id.clone(),
            status: status.clone(),
        });
        self.sink.summary(SummaryUpdate {
            repo_id: repo_id.clone(),
            state: SummaryState::Ready {
                summary: RepoSummary {
                    head: status.head.clone(),
                    dirty: status.is_dirty(),
                },
            },
        });
    }
}

async fn refresh_loop(
    monitor: Weak<Monitor>,
    git: Git,
    repo_id: RepoId,
    path: PathBuf,
    wake: Arc<Notify>,
) {
    loop {
        wake.notified().await;
        tokio::time::sleep(DEBOUNCE).await;
        let Some(monitor) = monitor.upgrade() else {
            return;
        };
        {
            let _writes = monitor.writes.lock().await;
            if let Err(error) = monitor.refresh(&git, &repo_id, &path).await {
                tracing::warn!(?error, path = %path.display(), "status refresh failed");
            }
        }
        drop(monitor);
        tokio::time::sleep(MIN_INTERVAL).await;
    }
}

fn start_watcher(root: &Path, wake: Arc<Notify>) -> Option<RecommendedWatcher> {
    let git_dir = root.join(".git");
    let handler = move |result: notify::Result<notify::Event>| match result {
        Ok(event) if is_relevant(&event, &git_dir) => wake.notify_one(),
        Ok(_) => {}
        Err(error) => tracing::debug!(%error, "watcher error"),
    };
    let mut watcher = match notify::recommended_watcher(handler) {
        Ok(watcher) => watcher,
        Err(error) => {
            tracing::warn!(%error, "file watcher unavailable");
            return None;
        }
    };
    if let Err(error) = watcher.watch(root, RecursiveMode::Recursive) {
        tracing::warn!(%error, path = %root.display(), "cannot watch repository");
        return None;
    }
    Some(watcher)
}

/// Working tree changes are always relevant. Inside `.git` only changes to
/// the index, `HEAD` and refs matter, not objects, logs or lock files.
fn is_relevant(event: &notify::Event, git_dir: &Path) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event.paths.iter().any(|path| {
        let Ok(inside) = path.strip_prefix(git_dir) else {
            return true;
        };
        if path.extension().is_some_and(|ext| ext == "lock") {
            return false;
        }
        match inside.components().next() {
            Some(Component::Normal(first)) => {
                ["index", "HEAD", "packed-refs", "refs", "MERGE_HEAD"]
                    .iter()
                    .any(|name| first == *name)
            }
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(path: &str) -> notify::Event {
        notify::Event::new(EventKind::Any).add_path(PathBuf::from(path))
    }

    #[test]
    fn filters_git_internals() {
        let git_dir = Path::new("/repo/.git");
        for relevant in [
            "/repo/src/main.rs",
            "/repo/.git/index",
            "/repo/.git/HEAD",
            "/repo/.git/refs/heads/main",
        ] {
            assert!(is_relevant(&event(relevant), git_dir), "{relevant}");
        }
        for irrelevant in [
            "/repo/.git/index.lock",
            "/repo/.git/objects/ab/cdef",
            "/repo/.git/logs/HEAD",
            "/repo/.git/FETCH_HEAD",
        ] {
            assert!(!is_relevant(&event(irrelevant), git_dir), "{irrelevant}");
        }
    }
}
