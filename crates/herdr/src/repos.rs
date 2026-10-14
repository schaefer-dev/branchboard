//! Which herdr workspaces and agents belong to which repository.

use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use crate::session::Pane;
use crate::{AgentState, Session};

/// One agent working in a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentSummary {
    pub pane_id: String,
    /// e.g. "claude" or "codex".
    pub agent: String,
    pub state: AgentState,
    pub title: Option<String>,
}

/// An agent in any herdr workspace, as listed in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentEntry {
    pub pane_id: String,
    /// e.g. "claude" or "codex".
    pub agent: String,
    pub state: AgentState,
    pub title: Option<String>,
    /// The label of the herdr workspace (space) the agent runs in.
    pub workspace: String,
    /// The label of its tab, if it has one.
    pub tab: Option<String>,
}

/// The agents working in one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoAgents {
    /// The most urgent state of all agents.
    pub state: AgentState,
    /// Most urgent first.
    pub agents: Vec<AgentSummary>,
}

/// Maps directories to the repository containing them. With nested
/// repositories the innermost one wins.
#[derive(Debug, Clone)]
pub struct RepoMatcher<K> {
    repos: Vec<Repo<K>>,
}

#[derive(Debug, Clone)]
struct Repo<K> {
    key: K,
    path: PathBuf,
    /// Comparable form of `path`.
    root: PathBuf,
}

impl<K: Clone + Eq + Hash> RepoMatcher<K> {
    pub fn new(repos: impl IntoIterator<Item = (K, PathBuf)>) -> Self {
        let mut repos: Vec<Repo<K>> = repos
            .into_iter()
            .map(|(key, path)| Repo {
                key,
                root: comparable(&path.to_string_lossy()),
                path,
            })
            .collect();
        // Deepest first, so the first match is the innermost repository.
        repos.sort_by_key(|repo| std::cmp::Reverse(repo.root.components().count()));
        Self { repos }
    }

    /// The repository containing `directory`, if any.
    pub fn locate(&self, directory: &str) -> Option<&K> {
        let directory = comparable(directory);
        self.repos
            .iter()
            .find(|repo| directory.starts_with(&repo.root))
            .map(|repo| &repo.key)
    }

    pub fn path(&self, key: &K) -> Option<&Path> {
        self.repos
            .iter()
            .find(|repo| &repo.key == key)
            .map(|repo| repo.path.as_path())
    }
}

impl Session {
    /// The agents of every repository that has at least one.
    pub fn repo_agents<K: Clone + Eq + Hash>(
        &self,
        repos: &RepoMatcher<K>,
    ) -> HashMap<K, RepoAgents> {
        let mut found: HashMap<K, Vec<AgentSummary>> = HashMap::new();
        for agent in &self.agents {
            let Some(key) = self.agent_repo(agent, repos) else {
                continue;
            };
            found.entry(key.clone()).or_default().push(AgentSummary {
                pane_id: agent.pane_id.clone(),
                agent: agent.kind().to_owned(),
                state: agent.agent_status,
                title: agent.title().map(str::to_owned),
            });
        }
        found
            .into_iter()
            .map(|(key, mut agents)| {
                agents.sort_by(|a, b| {
                    b.state
                        .urgency()
                        .cmp(&a.state.urgency())
                        .then_with(|| a.pane_id.cmp(&b.pane_id))
                });
                let state = agents[0].state;
                (key, RepoAgents { state, agents })
            })
            .collect()
    }

    /// Every agent in the session with the repository it works in, if any:
    /// most urgent first, then in workspace order.
    pub fn agent_list<K: Clone + Eq + Hash>(
        &self,
        repos: &RepoMatcher<K>,
    ) -> Vec<(Option<K>, AgentEntry)> {
        let mut listed: Vec<(u32, Option<K>, AgentEntry)> = self
            .agents
            .iter()
            .map(|agent| {
                let workspace = self.workspace(&agent.workspace_id);
                let entry = AgentEntry {
                    pane_id: agent.pane_id.clone(),
                    agent: agent.kind().to_owned(),
                    state: agent.agent_status,
                    title: agent.title().map(str::to_owned),
                    workspace: workspace.map_or_else(
                        || agent.workspace_id.clone(),
                        |workspace| workspace.label.clone(),
                    ),
                    tab: agent
                        .tab_id
                        .as_deref()
                        .and_then(|id| self.tab(id))
                        .and_then(|tab| tab.label.clone()),
                };
                let number = workspace.map_or(u32::MAX, |workspace| workspace.number);
                (number, self.agent_repo(agent, repos).cloned(), entry)
            })
            .collect();
        listed.sort_by(|(a_number, _, a), (b_number, _, b)| {
            b.state
                .urgency()
                .cmp(&a.state.urgency())
                .then(a_number.cmp(b_number))
                .then_with(|| a.pane_id.cmp(&b.pane_id))
        });
        listed
            .into_iter()
            .map(|(_, key, entry)| (key, entry))
            .collect()
    }

    fn agent_repo<'r, K: Clone + Eq + Hash>(
        &self,
        agent: &crate::session::Agent,
        repos: &'r RepoMatcher<K>,
    ) -> Option<&'r K> {
        agent
            .foreground_cwd
            .as_deref()
            .or(agent.cwd.as_deref())
            .or_else(|| self.pane(&agent.pane_id).and_then(Pane::directory))
            .and_then(|directory| repos.locate(directory))
    }

    /// The workspace to show for a repository: the one with the most panes
    /// working in it (a worktree workspace of the repository counts as one
    /// such pane). Ties go to the lower workspace number.
    pub fn workspace_for<K: Clone + Eq + Hash>(
        &self,
        repos: &RepoMatcher<K>,
        key: &K,
    ) -> Option<&str> {
        let in_repo = |directory: Option<&str>| {
            directory.and_then(|directory| repos.locate(directory)) == Some(key)
        };
        self.workspaces
            .iter()
            .map(|workspace| {
                let panes = self
                    .panes
                    .iter()
                    .filter(|pane| {
                        pane.workspace_id == workspace.workspace_id && in_repo(pane.directory())
                    })
                    .count();
                let worktree = workspace.worktree.as_ref().is_some_and(|worktree| {
                    in_repo(worktree.checkout_path.as_deref())
                        || in_repo(worktree.repo_root.as_deref())
                });
                (workspace, panes + usize::from(worktree))
            })
            .filter(|(_, score)| *score > 0)
            .max_by(|(a, a_score), (b, b_score)| {
                a_score.cmp(b_score).then_with(|| b.number.cmp(&a.number))
            })
            .map(|(workspace, _)| workspace.workspace_id.as_str())
    }
}

/// A path in a form that compares like the file system does.
fn comparable(path: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(windows_comparable(path))
    } else {
        PathBuf::from(path)
    }
}

/// Windows paths compare case-insensitively and may arrive as verbatim
/// (`\\?\C:\…`) or MSYS (`/c/…`, from Git Bash) paths.
fn windows_comparable(path: &str) -> String {
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    let bytes = path.as_bytes();
    let path = if bytes.len() >= 2
        && bytes[0] == b'/'
        && bytes[1].is_ascii_alphabetic()
        && (bytes.len() == 2 || bytes[2] == b'/')
    {
        format!("{}:{}", char::from(bytes[1]), &path[2..])
    } else {
        path.to_owned()
    };
    path.replace('/', "\\").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        serde_json::from_str::<crate::session::SnapshotResult>(include_str!(
            "../tests/fixtures/session.json"
        ))
        .unwrap()
        .snapshot
    }

    fn matcher(repos: &[(&'static str, &str)]) -> RepoMatcher<&'static str> {
        RepoMatcher::new(repos.iter().map(|(key, path)| (*key, PathBuf::from(path))))
    }

    #[test]
    fn innermost_repository_wins() {
        let repos = matcher(&[("outer", "/work/app"), ("inner", "/work/app/vendor/lib")]);
        assert_eq!(repos.locate("/work/app/src"), Some(&"outer"));
        assert_eq!(repos.locate("/work/app/vendor/lib/src"), Some(&"inner"));
        assert_eq!(repos.locate("/work/app"), Some(&"outer"));
        assert_eq!(repos.locate("/work/application"), None);
        assert_eq!(repos.locate("/elsewhere"), None);
    }

    #[test]
    fn windows_paths_compare_like_windows() {
        assert_eq!(windows_comparable(r"C:\Dev\App"), r"c:\dev\app");
        assert_eq!(windows_comparable(r"\\?\C:\Dev\App"), r"c:\dev\app");
        assert_eq!(windows_comparable("/c/Dev/App"), r"c:\dev\app");
        assert_eq!(windows_comparable("/c"), "c:");
        assert_eq!(windows_comparable("/cache/x"), r"\cache\x");
    }

    #[test]
    fn agents_roll_up_per_repository() {
        let repos = matcher(&[
            ("api", "/home/dev/api"),
            ("web", "/home/dev/web"),
            ("docs", "/home/dev/docs"),
        ]);
        let agents = session().repo_agents(&repos);

        let api = &agents["api"];
        assert_eq!(api.state, AgentState::Blocked);
        let states: Vec<_> = api.agents.iter().map(|agent| agent.state).collect();
        assert_eq!(states, [AgentState::Blocked, AgentState::Working]);
        assert_eq!(api.agents[0].agent, "codex");
        assert_eq!(api.agents[1].title.as_deref(), Some("Refactor the router"));

        // The agent's own working directory counts, not the workspace it is in.
        let web = &agents["web"];
        assert_eq!(web.state, AgentState::Done);
        assert_eq!(web.agents[0].agent, "claude");

        assert!(!agents.contains_key("docs"));
        // Agents outside every repository are ignored.
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn lists_every_agent_most_urgent_first() {
        let repos = matcher(&[("api", "/home/dev/api"), ("web", "/home/dev/web")]);
        let listed = session().agent_list(&repos);
        let summary: Vec<_> = listed
            .iter()
            .map(|(repo, entry)| (entry.pane_id.as_str(), entry.state, *repo))
            .collect();
        assert_eq!(
            summary,
            [
                ("w1:p2", AgentState::Blocked, Some("api")),
                ("w1:p4", AgentState::Done, Some("web")),
                ("w1:p3", AgentState::Working, Some("api")),
                // Spaces that are no repository are listed too.
                ("w5:p2", AgentState::Idle, None),
                ("w5:p1", AgentState::Unknown, None),
            ]
        );
        let (_, first) = &listed[0];
        assert_eq!(first.workspace, "api");
        assert_eq!(first.tab.as_deref(), Some("agents"));
        assert_eq!(first.agent, "codex");
        let (_, unlabelled) = &listed[3];
        assert_eq!(unlabelled.workspace, "scratch");
        assert_eq!(unlabelled.tab, None);
    }

    #[test]
    fn unknown_states_from_newer_herdr_versions_are_tolerated() {
        let agents = session().repo_agents(&matcher(&[("scratch", "/tmp/scratch")]));
        assert_eq!(agents["scratch"].state, AgentState::Unknown);
    }

    #[test]
    fn workspace_with_most_panes_in_the_repository() {
        let session = session();
        let repos = matcher(&[
            ("api", "/home/dev/api"),
            ("web", "/home/dev/web"),
            ("docs", "/home/dev/docs"),
            ("site", "/home/dev/site"),
        ]);
        assert_eq!(session.workspace_for(&repos, &"api"), Some("w1"));
        // w2 has one pane in web, w3 (a worktree workspace) has one too; the
        // lower number wins the tie.
        assert_eq!(session.workspace_for(&repos, &"web"), Some("w2"));
        assert_eq!(session.workspace_for(&repos, &"docs"), None);
        // Found through herdr's worktree info alone.
        assert_eq!(session.workspace_for(&repos, &"site"), Some("w4"));
    }
}
