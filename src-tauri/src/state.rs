use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use git_core::{DiscoveryOptions, Git};
use herdr::{EmbeddedClient, HerdrLink, RepoMatcher};
use pty::PtyCommand;
use std::path::PathBuf;
use tokio::sync::OnceCell;

use workspace::{Editor, RepoId, Workspace, WorkspaceError, WorkspaceSnapshot};

use crate::dto::{AgentRow, AgentsSnapshot, RepoAgents};
use crate::error::{AppError, AppErrorKind};
use crate::terminal::Terminal;

/// After starting the embedded client, how often and how long to wait for
/// the herdr server it may have started.
const HERDR_START_ATTEMPTS: u32 = 20;
const HERDR_START_INTERVAL: Duration = Duration::from_millis(250);

/// Window focus refreshes all summaries, but at most this often.
const FOCUS_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

/// Application-wide state managed by Tauri. Cheap to clone, so background
/// tasks can own a handle.
#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub app_version: String,
    pub workspace: Workspace,
    pub herdr: HerdrLink,
    /// The agents last sent to the frontend. Held while computing and
    /// sending the next ones, so updates cannot overtake each other.
    agents: tokio::sync::Mutex<Option<AgentsSnapshot>>,
    git: OnceCell<Result<Git, AppError>>,
    last_focus_refresh: Mutex<Option<Instant>>,
    /// Background fetches only happen while the window has focus.
    focused: AtomicBool,
    editor_override: Option<PathBuf>,
    pub terminal: Terminal,
    /// Where the embedded client's herdr config is written.
    terminal_config: PathBuf,
    /// Program to run instead of herdr in the terminal (tests).
    terminal_program: Option<PathBuf>,
}

/// Paths that differ between the real app and tests.
pub struct Paths {
    pub editor: Option<PathBuf>,
    pub terminal_config: PathBuf,
    pub terminal_program: Option<PathBuf>,
}

impl Deref for AppState {
    type Target = Inner;

    fn deref(&self) -> &Inner {
        &self.0
    }
}

impl AppState {
    pub fn new(app_version: String, workspace: Workspace, herdr: HerdrLink, paths: Paths) -> Self {
        Self(Arc::new(Inner {
            app_version,
            workspace,
            herdr,
            agents: tokio::sync::Mutex::new(None),
            git: OnceCell::new(),
            last_focus_refresh: Mutex::new(None),
            focused: AtomicBool::new(true),
            editor_override: paths.editor,
            terminal: Terminal::default(),
            terminal_config: paths.terminal_config,
            terminal_program: paths.terminal_program,
        }))
    }

    pub fn set_focused(&self, focused: bool) {
        self.focused.store(focused, Ordering::Relaxed);
    }

    /// Fetches the repository on screen in the background if background
    /// fetching is on, the window has focus and the last fetch is older
    /// than the configured interval.
    pub fn spawn_background_fetch(&self) {
        if !self.focused.load(Ordering::Relaxed) {
            return;
        }
        let state = self.clone();
        tauri::async_runtime::spawn(async move {
            let minutes = state.workspace.settings().await.fetch_interval_minutes;
            if minutes == 0 {
                return;
            }
            if let Ok(git) = state.git().await {
                let max_age = Duration::from_secs(u64::from(minutes) * 60);
                state.workspace.fetch_if_stale(git, max_age).await;
            }
        });
    }

    /// Connects the herdr link once the embedded client is up: when the
    /// client had to start the herdr server, its socket appears a moment
    /// later. Gives up after a few seconds; window focus retries anyway.
    pub fn spawn_herdr_connect_after_start(&self) {
        let herdr = self.herdr.clone();
        tauri::async_runtime::spawn(async move {
            for _ in 0..HERDR_START_ATTEMPTS {
                herdr.connect().await;
                if herdr.session().is_some() {
                    return;
                }
                tokio::time::sleep(HERDR_START_INTERVAL).await;
            }
        });
    }

    /// Refreshes repository summaries (all, or only `only`) in the
    /// background. Results arrive as `repo-summary` events.
    pub fn spawn_summary_refresh(&self, only: Option<RepoId>) {
        let state = self.clone();
        tauri::async_runtime::spawn(async move {
            match state.git().await {
                Ok(git) => state.workspace.refresh_summaries(git, only.as_ref()).await,
                Err(error) => tracing::debug!(?error, "skipping summary refresh without git"),
            }
        });
    }
}

impl Inner {
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

    /// The editor to open files in: the override, or VS Code found on the
    /// same search path as git.
    pub async fn editor(&self) -> Result<Editor, AppError> {
        if let Some(program) = &self.editor_override {
            return Ok(Editor::custom(program.clone()));
        }
        let git = self.git().await?;
        Editor::detect(git.runner().search_path())
            .ok_or_else(|| WorkspaceError::EditorNotFound.into())
    }

    /// The agents of every repository, from the last herdr session.
    pub async fn agents(&self) -> AgentsSnapshot {
        let mut last = self.agents.lock().await;
        let agents = self.current_agents().await;
        last.replace(agents.clone());
        agents
    }

    /// Calls `send` with the agents of every repository if they changed
    /// since they were last sent or returned.
    pub async fn publish_agents(&self, send: impl FnOnce(&AgentsSnapshot)) {
        let mut last = self.agents.lock().await;
        let agents = self.current_agents().await;
        if last.as_ref() != Some(&agents) {
            send(&agents);
            last.replace(agents);
        }
    }

    async fn current_agents(&self) -> AgentsSnapshot {
        let Some(session) = self.herdr.session() else {
            return AgentsSnapshot {
                available: false,
                repos: Vec::new(),
                agents: Vec::new(),
                focused_repo_id: None,
            };
        };
        let snapshot = self.workspace.snapshot().await;
        let matcher = repo_matcher(&snapshot);
        let agents = session
            .agent_list(&matcher)
            .into_iter()
            .map(|(repo_id, agent)| AgentRow { agent, repo_id })
            .collect();
        let mut found = session.repo_agents(&matcher);
        let repos = snapshot
            .repos
            .into_iter()
            .filter_map(|repo| {
                let agents = found.remove(&repo.id)?;
                Some(RepoAgents {
                    repo_id: repo.id,
                    state: agents.state,
                    agents: agents.agents,
                })
            })
            .collect();
        AgentsSnapshot {
            available: true,
            repos,
            agents,
            focused_repo_id: session.focused_repo(&matcher),
        }
    }

    /// The herdr binary for the embedded terminal: found on the same search
    /// path as git, else where herdr's installers put it.
    pub async fn herdr_binary(&self) -> Option<PathBuf> {
        if let Some(program) = &self.terminal_program {
            return Some(program.clone());
        }
        if let Ok(git) = self.git().await
            && let Some(binary) = herdr::find_binary(git.runner().search_path())
        {
            return Some(binary);
        }
        herdr::find_binary(&std::env::var_os("PATH").unwrap_or_default())
    }

    /// The command that starts the embedded herdr client, attached to the
    /// session the link talks to. Writes the client's config first.
    pub async fn terminal_command(&self) -> Result<PtyCommand, AppError> {
        let unavailable = || AppError::from(herdr::HerdrError::NotRunning);
        let endpoint = self.herdr.endpoint().ok_or_else(unavailable)?.clone();
        let binary = self.herdr_binary().await.ok_or_else(|| {
            AppError::new(
                AppErrorKind::HerdrUnavailable,
                "herdr was not found. Install herdr and restart Branchboard.",
                None,
            )
        })?;
        let user = herdr::user_config_path().and_then(|path| std::fs::read_to_string(path).ok());
        let config = herdr::embedded_config(user.as_deref());
        if let Some(dir) = self.terminal_config.parent() {
            std::fs::create_dir_all(dir).map_err(|error| AppError::io(&error))?;
        }
        std::fs::write(&self.terminal_config, config).map_err(|error| AppError::io(&error))?;
        let client = EmbeddedClient {
            binary,
            endpoint,
            config: self.terminal_config.clone(),
        };
        Ok(client.command(std::env::var_os("LANG").as_deref()))
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

/// Locates directories in the repositories of `snapshot`.
pub fn repo_matcher(snapshot: &WorkspaceSnapshot) -> RepoMatcher<RepoId> {
    RepoMatcher::new(
        snapshot
            .repos
            .iter()
            .map(|repo| (repo.id.clone(), PathBuf::from(&repo.path))),
    )
}
