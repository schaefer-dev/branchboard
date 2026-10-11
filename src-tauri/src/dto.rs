//! Data transfer objects returned to the frontend.

use git_core::GitVersion;
use serde::Serialize;
use ts_rs::TS;
use workspace::{RepoId, WorkspaceSnapshot};

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GitInfo {
    pub path: String,
    pub version: GitVersion,
}

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AddRepoResult {
    pub workspace: WorkspaceSnapshot,
    pub repo_id: RepoId,
    /// The repository was already in the list and has only been selected.
    pub already_added: bool,
}
