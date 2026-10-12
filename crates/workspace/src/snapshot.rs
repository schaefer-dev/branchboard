//! Read models sent to the frontend.

use std::path::Path;

use git_core::{RepoSummary, StatusSnapshot};
use serde::Serialize;
use ts_rs::TS;

use crate::registry::{Group, GroupId, Registry, RepoId};

/// Everything the frontend needs to render the repository list.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WorkspaceSnapshot {
    pub repos: Vec<RepoView>,
    pub groups: Vec<Group>,
    pub active_repo_id: Option<RepoId>,
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
                    missing: is_missing(&repo.path),
                })
                .collect(),
            groups: registry.groups.clone(),
            active_repo_id: registry.active_repo_id.clone(),
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
