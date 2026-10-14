//! Data transfer objects returned to the frontend.

use git_core::GitVersion;
use herdr::{AgentEntry, AgentState, AgentSummary};
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

/// The coding agents herdr runs in each repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentsSnapshot {
    /// A herdr session is running and reachable.
    pub available: bool,
    /// Only repositories with at least one agent, in list order.
    pub repos: Vec<RepoAgents>,
    /// Every agent in every herdr workspace, most urgent first.
    pub agents: Vec<AgentRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentRow {
    #[serde(flatten)]
    pub agent: AgentEntry,
    /// The repository the agent works in, if it is in the list.
    pub repo_id: Option<RepoId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepoAgents {
    pub repo_id: RepoId,
    /// The most urgent state of all agents.
    pub state: AgentState,
    /// Most urgent first.
    pub agents: Vec<AgentSummary>,
}
