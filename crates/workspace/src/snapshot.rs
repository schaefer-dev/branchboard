//! Read models sent to the frontend.

use std::path::Path;

use git_core::{CommitList, RepoSummary, StatusSnapshot};
use serde::Serialize;
use ts_rs::TS;

use crate::registry::{Group, GroupId, Registry, RepoId, Settings};

/// Everything the frontend needs to render the repository list.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkspaceSnapshot {
    pub repos: Vec<RepoView>,
    pub groups: Vec<Group>,
    pub active_repo_id: Option<RepoId>,
    pub settings: Settings,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepoView {
    pub id: RepoId,
    /// Display name: the custom name or the folder name.
    pub name: String,
    pub custom_name: Option<String>,
    pub path: String,
    pub group_id: Option<GroupId>,
    pub favorite: bool,
    /// The ref the user chose to compare branches with; `null` means
    /// automatic (`origin/development`, else `origin/main`). The branch list
    /// reports the ref actually used.
    pub mainline: Option<String>,
    /// The folder was moved or deleted, or is no longer a repository.
    pub missing: bool,
}

impl WorkspaceSnapshot {
    pub(crate) fn of(registry: &Registry) -> Self {
        Self {
            repos: registry
                .repos
                .iter()
                .map(|repo| RepoView {
                    id: repo.id.clone(),
                    name: repo.display_name(),
                    custom_name: repo.custom_name.clone(),
                    path: repo.path.display().to_string(),
                    group_id: repo.group_id.clone(),
                    favorite: repo.favorite,
                    mainline: repo.mainline().map(str::to_owned),
                    missing: is_missing(&repo.path),
                })
                .collect(),
            groups: registry.groups.clone(),
            active_repo_id: registry.active_repo_id.clone(),
            settings: registry.settings.clone(),
        }
    }
}

/// Cheap check whether a working tree still exists at `path`. `.git` may be
/// a directory or, for linked worktrees and submodules, a file.
pub(crate) fn is_missing(path: &Path) -> bool {
    !path.join(".git").exists()
}

/// Background update for one repository's sidebar summary.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SummaryUpdate {
    pub repo_id: RepoId,
    pub state: SummaryState,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export)]
pub enum SummaryState {
    Ready { summary: RepoSummary },
    Missing,
    Failed { message: String },
}

/// New status of the watched repository.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StatusUpdate {
    pub repo_id: RepoId,
    pub status: StatusSnapshot,
}

/// Progress of a fetch of one repository.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FetchUpdate {
    pub repo_id: RepoId,
    pub state: FetchState,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export)]
pub enum FetchState {
    Running,
    Done,
    Failed {
        message: String,
        detail: Option<String>,
    },
}

/// How a branch differs from the mainline, commit by commit.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BranchCommits {
    pub branch: String,
    pub mainline: String,
    /// Commits on the branch but not on the mainline. If the mainline does
    /// not exist, simply the latest commits of the branch.
    pub ahead: CommitList,
    /// Commits on the mainline but not on the branch; `None` if the mainline
    /// does not exist.
    pub behind: Option<CommitList>,
}
