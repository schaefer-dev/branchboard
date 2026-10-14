//! The parts of herdr's `session.snapshot` that Branchboard uses.

#![allow(
    clippy::struct_field_names,
    reason = "fields mirror the names in herdr's JSON"
)]

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What an agent is doing, as herdr detects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AgentState {
    /// Needs input, an approval or a decision.
    Blocked,
    /// Finished, and nobody has looked at it yet.
    Done,
    Working,
    /// Finished or waiting, and seen.
    Idle,
    /// herdr cannot tell (also any state a newer herdr adds).
    Unknown,
}

impl AgentState {
    /// How much this state asks for attention; the highest one represents a
    /// repository with several agents.
    pub fn urgency(self) -> u8 {
        match self {
            Self::Blocked => 4,
            Self::Done => 3,
            Self::Working => 2,
            Self::Idle => 1,
            Self::Unknown => 0,
        }
    }
}

/// A point-in-time view of the herdr session.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub protocol: u32,
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    #[serde(default)]
    pub panes: Vec<Pane>,
    #[serde(default)]
    pub agents: Vec<Agent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Workspace {
    pub workspace_id: String,
    #[serde(default)]
    pub number: u32,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub focused: bool,
    /// Set for workspaces herdr created for a git worktree.
    #[serde(default)]
    pub worktree: Option<Worktree>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Worktree {
    #[serde(default)]
    pub repo_root: Option<String>,
    #[serde(default)]
    pub checkout_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Pane {
    pub pane_id: String,
    pub workspace_id: String,
    #[serde(default)]
    pub cwd: Option<String>,
    /// Working directory of the process in the foreground, e.g. an agent
    /// started in another directory.
    #[serde(default)]
    pub foreground_cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Agent {
    pub pane_id: String,
    pub workspace_id: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub display_agent: Option<String>,
    #[serde(default = "unknown", deserialize_with = "agent_state")]
    pub agent_status: AgentState,
    /// A name the user gave the agent.
    #[serde(default)]
    pub name: Option<String>,
    /// The terminal title without escape codes; agents such as Claude Code
    /// put the topic of the conversation there.
    #[serde(default)]
    pub terminal_title_stripped: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub foreground_cwd: Option<String>,
}

fn unknown() -> AgentState {
    AgentState::Unknown
}

/// Reads herdr's `agent_status`; states a newer herdr adds become `Unknown`.
fn agent_state<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<AgentState, D::Error> {
    let status = Option::<String>::deserialize(deserializer)?;
    Ok(match status.as_deref() {
        Some("blocked") => AgentState::Blocked,
        Some("done") => AgentState::Done,
        Some("working") => AgentState::Working,
        Some("idle") => AgentState::Idle,
        _ => AgentState::Unknown,
    })
}

/// The `result` of a `session.snapshot` request.
#[derive(Debug, Deserialize)]
pub(crate) struct SnapshotResult {
    pub snapshot: Session,
}

impl Session {
    pub(crate) fn pane_ids(&self) -> BTreeSet<&str> {
        self.panes
            .iter()
            .map(|pane| pane.pane_id.as_str())
            .collect()
    }

    pub(crate) fn pane(&self, id: &str) -> Option<&Pane> {
        self.panes.iter().find(|pane| pane.pane_id == id)
    }
}

impl Pane {
    /// The directory the pane is working in: the foreground process's, else
    /// the shell's.
    pub fn directory(&self) -> Option<&str> {
        self.foreground_cwd.as_deref().or(self.cwd.as_deref())
    }
}

impl Agent {
    /// A short name for the agent, e.g. "claude".
    pub fn kind(&self) -> &str {
        self.display_agent
            .as_deref()
            .or(self.agent.as_deref())
            .unwrap_or("agent")
    }

    /// What the agent is about: its name, else its terminal title.
    pub fn title(&self) -> Option<&str> {
        self.name
            .as_deref()
            .or(self.terminal_title_stripped.as_deref())
            .map(str::trim)
            .filter(|title| !title.is_empty())
    }
}
