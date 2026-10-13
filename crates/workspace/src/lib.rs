//! The user's set of repositories: registry, persistence, background
//! summaries and the live status of the repository on screen. Independent of
//! Tauri; events leave through [`EventSink`].

mod editor;
mod error;
mod monitor;
mod registry;
mod snapshot;
mod store;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use git_core::{
    BranchList, ChangeKind, DiffRequest, FileDiff, Git, GitError, Head, StatusSnapshot,
};
use highlight::Highlighter;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

pub use editor::Editor;
pub use error::WorkspaceError;
pub use registry::{
    Added, DEFAULT_MAINLINE, Group, GroupId, Registry, RepoEntry, RepoId, Settings,
};
pub use snapshot::{
    BranchCommits, FetchState, FetchUpdate, RepoView, StatusUpdate, SummaryState, SummaryUpdate,
    WorkspaceSnapshot,
};

use crate::monitor::Monitor;

/// How many `git status` processes run at once when refreshing summaries.
const MAX_PARALLEL_SUMMARIES: usize = 4;

/// Receives background updates as they happen.
pub trait EventSink: Send + Sync + 'static {
    fn summary(&self, update: SummaryUpdate);
    fn status(&self, update: StatusUpdate);
    fn fetch(&self, update: FetchUpdate);
}

pub struct Workspace {
    store: store::Store,
    registry: Mutex<Registry>,
    sink: Arc<dyn EventSink>,
    monitor: Arc<Monitor>,
    /// Repositories with a fetch in flight.
    fetching: std::sync::Mutex<HashSet<RepoId>>,
    highlighter: Arc<OnceLock<Highlighter>>,
}

impl Workspace {
    /// Opens the workspace persisted at `path` (created on first save).
    pub fn open(path: PathBuf, sink: Arc<dyn EventSink>) -> Self {
        let store = store::Store::new(path);
        let registry = store.load();
        Self {
            store,
            registry: Mutex::new(registry),
            monitor: Monitor::new(Arc::clone(&sink)),
            sink,
            fetching: std::sync::Mutex::new(HashSet::new()),
            highlighter: Arc::new(OnceLock::new()),
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
            for removed in registry
                .repos
                .iter()
                .filter(|repo| next.repo(&repo.id).is_err())
            {
                self.monitor.forget(&removed.id);
            }
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
        self.monitor.forget(id);
        let ((), snapshot) = self
            .update(|registry| registry.relocate_repo(id, work_tree))
            .await?;
        Ok(snapshot)
    }

    /// Refreshes the summaries of all repositories, or only of `only`, and
    /// publishes each result as soon as it is ready.
    pub async fn refresh_summaries(&self, git: &Git, only: Option<&RepoId>) {
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
            let (git, sink, limit) = (git.clone(), Arc::clone(&self.sink), Arc::clone(&limit));
            tasks.spawn(async move {
                let Ok(_permit) = limit.acquire_owned().await else {
                    return;
                };
                let state = summarize(&git, &path).await;
                sink.summary(SummaryUpdate { repo_id, state });
            });
        }
        while tasks.join_next().await.is_some() {}
    }

    /// The status of `id`, which also becomes the watched repository. A
    /// cached status is returned immediately and refreshed in the background.
    pub async fn status(&self, git: &Git, id: &RepoId) -> Result<StatusSnapshot, WorkspaceError> {
        let path = self.repo_path(id).await?;
        self.monitor.watch(git, id, &path);
        if let Some(cached) = self.monitor.cached(id) {
            self.monitor.poke();
            return Ok(cached);
        }
        Ok(self.monitor.refresh(git, id, &path).await?)
    }

    /// Re-reads the status of the watched repository, e.g. on window focus.
    pub fn refresh_status(&self) {
        self.monitor.poke();
    }

    pub async fn diff(
        &self,
        git: &Git,
        id: &RepoId,
        request: &DiffRequest,
    ) -> Result<FileDiff, WorkspaceError> {
        let path = self.repo_path(id).await?;
        let mut diff = git.diff(&path, request).await?;
        if diff.binary || diff.too_large || !Highlighter::supports(&request.path) {
            return Ok(diff);
        }
        let sources = git.diff_sources(&path, request).await?;
        let highlighter = Arc::clone(&self.highlighter);
        let file = request.path.clone();
        // Highlighting is CPU-bound; keep it off the async workers.
        diff = tokio::task::spawn_blocking(move || {
            highlighter.get_or_init(Highlighter::new).highlight_diff(
                &file,
                &mut diff,
                sources.old.as_deref(),
                sources.new.as_deref(),
            );
            diff
        })
        .await
        .map_err(|error| WorkspaceError::Highlight(error.to_string()))?;
        Ok(diff)
    }

    pub async fn stage(
        &self,
        git: &Git,
        id: &RepoId,
        paths: &[String],
    ) -> Result<StatusSnapshot, WorkspaceError> {
        let path = self.repo_path(id).await?;
        let _writes = self.monitor.writes.lock().await;
        git.stage(&path, paths).await?;
        Ok(self.monitor.refresh(git, id, &path).await?)
    }

    /// Unstages the given paths. Renames are unstaged together with their
    /// original path.
    pub async fn unstage(
        &self,
        git: &Git,
        id: &RepoId,
        paths: &[String],
    ) -> Result<StatusSnapshot, WorkspaceError> {
        let path = self.repo_path(id).await?;
        let _writes = self.monitor.writes.lock().await;
        let status = git.status(&path).await?;
        let mut targets = Vec::with_capacity(paths.len());
        for change in paths.iter().filter_map(|p| status.find_staged(p)) {
            targets.push(change.path.clone());
            targets.extend(change.orig_path.clone());
        }
        let unborn = matches!(status.head, Head::Unborn { .. });
        git.unstage(&path, &targets, unborn).await?;
        Ok(self.monitor.refresh(git, id, &path).await?)
    }

    /// Discards unstaged changes: tracked files are restored from the index,
    /// untracked files are moved to the trash. Conflicted files are skipped.
    pub async fn discard(
        &self,
        git: &Git,
        id: &RepoId,
        paths: &[String],
    ) -> Result<StatusSnapshot, WorkspaceError> {
        let root = self.repo_path(id).await?;
        let _writes = self.monitor.writes.lock().await;
        let status = git.status(&root).await?;
        let (mut restore, mut untracked) = (Vec::new(), Vec::new());
        for change in paths.iter().filter_map(|p| status.find_unstaged(p)) {
            match change.kind {
                ChangeKind::Untracked => untracked.push(join_relative(&root, &change.path)),
                ChangeKind::Conflicted => {}
                _ => restore.push(change.path.clone()),
            }
        }
        git.restore_worktree(&root, &restore).await?;
        move_to_trash(untracked).await?;
        Ok(self.monitor.refresh(git, id, &root).await?)
    }

    pub async fn branches(&self, git: &Git, id: &RepoId) -> Result<BranchList, WorkspaceError> {
        let (path, mainline) = self.repo_path_and_mainline(id).await?;
        Ok(git.branches(&path, &mainline).await?)
    }

    /// Opens the repository, or one of its files at `line`, in the editor.
    pub async fn open_in_editor(
        &self,
        editor: &Editor,
        id: &RepoId,
        file: Option<&str>,
        line: Option<u32>,
    ) -> Result<(), WorkspaceError> {
        let root = self.repo_path(id).await?;
        let file = file.map(|relative| join_relative(&root, relative));
        editor.open(&root, file.as_deref().map(|path| (path, line)))
    }

    /// Commits `branch` is ahead of and behind the mainline.
    pub async fn branch_commits(
        &self,
        git: &Git,
        id: &RepoId,
        branch: &str,
    ) -> Result<BranchCommits, WorkspaceError> {
        const LIMIT: usize = 100;
        let (path, mainline) = self.repo_path_and_mainline(id).await?;
        let list = git.branches(&path, &mainline).await?;
        let (ahead, behind) = if list.mainline_exists {
            let ahead = git.log(&path, Some(&mainline), branch, LIMIT).await?;
            let behind = git.log(&path, Some(branch), &mainline, LIMIT).await?;
            (ahead, Some(behind))
        } else {
            (git.log(&path, None, branch, LIMIT).await?, None)
        };
        Ok(BranchCommits {
            branch: branch.to_owned(),
            mainline,
            ahead,
            behind,
        })
    }

    /// Switches to `branch` and returns the updated branch list. The new
    /// status is published like any other status change.
    pub async fn switch_branch(
        &self,
        git: &Git,
        id: &RepoId,
        branch: &str,
    ) -> Result<BranchList, WorkspaceError> {
        let (path, mainline) = self.repo_path_and_mainline(id).await?;
        {
            let _writes = self.monitor.writes.lock().await;
            git.switch(&path, branch).await?;
            self.monitor.refresh(git, id, &path).await?;
        }
        Ok(git.branches(&path, &mainline).await?)
    }

    /// Fetches all remotes of `id` and returns the updated branch list.
    /// Progress is also published as [`FetchUpdate`]s. A fetch that is
    /// already running is not started twice.
    pub async fn fetch(&self, git: &Git, id: &RepoId) -> Result<BranchList, WorkspaceError> {
        let (path, mainline) = self.repo_path_and_mainline(id).await?;
        let started = self
            .fetching
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.clone());
        if started {
            self.publish_fetch(id, FetchState::Running);
            let result = git.fetch(&path).await;
            self.fetching
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(id);
            match result {
                Ok(()) => self.publish_fetch(id, FetchState::Done),
                Err(error) => {
                    let (message, detail) = describe_fetch_error(&error);
                    self.publish_fetch(id, FetchState::Failed { message, detail });
                    return Err(error.into());
                }
            }
        }
        Ok(git.branches(&path, &mainline).await?)
    }

    /// Fetches the watched repository if its last fetch (by any tool) is
    /// older than `max_age`. Failures are published, not returned.
    pub async fn fetch_if_stale(&self, git: &Git, max_age: Duration) {
        let Some(id) = self.monitor.watched() else {
            return;
        };
        let Ok(path) = self.repo_path(&id).await else {
            return;
        };
        let last = git.last_fetched(&path).await.ok().flatten();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let max_age = i64::try_from(max_age.as_secs()).unwrap_or(i64::MAX);
        if last.is_some_and(|at| now.saturating_sub(at) < max_age) {
            return;
        }
        tracing::debug!(path = %path.display(), "background fetch");
        if let Err(error) = self.fetch(git, &id).await {
            tracing::info!(?error, "background fetch failed");
        }
    }

    pub async fn settings(&self) -> Settings {
        self.registry.lock().await.settings.clone()
    }

    fn publish_fetch(&self, id: &RepoId, state: FetchState) {
        self.sink.fetch(FetchUpdate {
            repo_id: id.clone(),
            state,
        });
    }

    async fn repo_path(&self, id: &RepoId) -> Result<PathBuf, WorkspaceError> {
        Ok(self.registry.lock().await.repo(id)?.path.clone())
    }

    async fn repo_path_and_mainline(
        &self,
        id: &RepoId,
    ) -> Result<(PathBuf, String), WorkspaceError> {
        let registry = self.registry.lock().await;
        let repo = registry.repo(id)?;
        Ok((repo.path.clone(), repo.mainline().to_owned()))
    }
}

/// User-facing message and git's own explanation of a failed fetch.
fn describe_fetch_error(error: &GitError) -> (String, Option<String>) {
    match error {
        GitError::Failed { stderr, .. } => ("Fetching failed.".into(), Some(stderr.clone())),
        GitError::Timeout { .. } => ("Fetching took too long and was stopped.".into(), None),
        other => ("Fetching failed.".into(), Some(other.to_string())),
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

/// Joins a repository-relative path with `/` separators onto `root`.
fn join_relative(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

async fn move_to_trash(paths: Vec<PathBuf>) -> Result<(), WorkspaceError> {
    if paths.is_empty() {
        return Ok(());
    }
    tokio::task::spawn_blocking(move || {
        let context = trash_context();
        context.delete_all(&paths)
    })
    .await
    .map_err(|error| WorkspaceError::Trash(error.to_string()))?
    .map_err(|error| WorkspaceError::Trash(error.to_string()))
}

#[cfg(target_os = "macos")]
fn trash_context() -> trash::TrashContext {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    // The default (Finder via AppleScript) needs automation permission.
    let mut context = trash::TrashContext::default();
    context.set_delete_method(DeleteMethod::NsFileManager);
    context
}

#[cfg(not(target_os = "macos"))]
fn trash_context() -> trash::TrashContext {
    trash::TrashContext::default()
}

/// Canonical form of a repository path, so the same folder is never
/// registered twice. `dunce` avoids `\\?\` prefixes on Windows.
fn normalize(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_owned())
}
