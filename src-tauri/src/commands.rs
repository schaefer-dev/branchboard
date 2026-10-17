//! Commands invoked from the frontend. Keep these thin: translate between
//! IPC types and the core crates, nothing else. Every command needs a typed
//! wrapper in `ui/src/lib/api/commands.ts`.

#![allow(
    clippy::needless_pass_by_value,
    reason = "Tauri injects command arguments by value"
)]

use std::path::PathBuf;

use git_core::{
    BranchList, DiffRequest, FileDiff, HeadCommit, ReviewDiffRequest, ReviewFiles, ReviewTarget,
    StatusSnapshot,
};
use tauri::State;
use tauri::ipc::{Channel, InvokeResponseBody};
use workspace::{
    Added, BranchCommits, CommitResult, GroupId, RepoId, UndoCommitResult, WorkspaceSnapshot,
};

use crate::dto::{AddRepoResult, AgentsSnapshot, AppInfo, GitInfo, TerminalInfo};
use crate::error::AppError;
use crate::state::{AppState, repo_matcher};

type CommandResult<T> = Result<T, AppError>;

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: state.app_version.clone(),
    }
}

#[tauri::command]
pub async fn get_git_info(state: State<'_, AppState>) -> CommandResult<GitInfo> {
    let git = state.git().await?;
    Ok(GitInfo {
        path: git.program().display().to_string(),
        version: git.version(),
    })
}

#[tauri::command]
pub async fn get_workspace(state: State<'_, AppState>) -> CommandResult<WorkspaceSnapshot> {
    Ok(state.workspace.snapshot().await)
}

#[tauri::command]
pub async fn add_repo(state: State<'_, AppState>, path: PathBuf) -> CommandResult<AddRepoResult> {
    let git = state.git().await?;
    let (added, workspace) = state.workspace.add_repo(git, &path).await?;
    let (repo_id, already_added) = match added {
        Added::New(id) => (id, false),
        Added::Existing(id) => (id, true),
    };
    state.spawn_summary_refresh(Some(repo_id.clone()));
    state.herdr.republish();
    Ok(AddRepoResult {
        workspace,
        repo_id,
        already_added,
    })
}

#[tauri::command]
pub async fn relocate_repo(
    state: State<'_, AppState>,
    repo_id: RepoId,
    path: PathBuf,
) -> CommandResult<WorkspaceSnapshot> {
    let git = state.git().await?;
    let workspace = state.workspace.relocate_repo(git, &repo_id, &path).await?;
    state.spawn_summary_refresh(Some(repo_id));
    state.herdr.republish();
    Ok(workspace)
}

#[tauri::command]
pub async fn remove_repo(
    state: State<'_, AppState>,
    repo_id: RepoId,
) -> CommandResult<WorkspaceSnapshot> {
    let workspace = update(&state, |registry| registry.remove_repo(&repo_id)).await?;
    state.herdr.republish();
    Ok(workspace)
}

#[tauri::command]
pub async fn rename_repo(
    state: State<'_, AppState>,
    repo_id: RepoId,
    name: Option<String>,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| {
        registry.rename_repo(&repo_id, name.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn set_repo_favorite(
    state: State<'_, AppState>,
    repo_id: RepoId,
    favorite: bool,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| registry.set_favorite(&repo_id, favorite)).await
}

#[tauri::command]
pub async fn set_repo_group(
    state: State<'_, AppState>,
    repo_id: RepoId,
    group_id: Option<GroupId>,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| {
        registry.set_repo_group(&repo_id, group_id)
    })
    .await
}

#[tauri::command]
pub async fn set_active_repo(
    state: State<'_, AppState>,
    repo_id: Option<RepoId>,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| {
        registry.set_active_repo(repo_id.as_ref())
    })
    .await
}

/// Creates a group and, if given, moves a repository into it.
#[tauri::command]
pub async fn create_group(
    state: State<'_, AppState>,
    name: String,
    repo_id: Option<RepoId>,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| {
        let group_id = registry.create_group(&name)?;
        match &repo_id {
            Some(repo_id) => registry.set_repo_group(repo_id, Some(group_id)),
            None => Ok(()),
        }
    })
    .await
}

#[tauri::command]
pub async fn rename_group(
    state: State<'_, AppState>,
    group_id: GroupId,
    name: String,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| registry.rename_group(&group_id, &name)).await
}

#[tauri::command]
pub async fn delete_group(
    state: State<'_, AppState>,
    group_id: GroupId,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| registry.delete_group(&group_id)).await
}

#[tauri::command]
pub async fn set_group_collapsed(
    state: State<'_, AppState>,
    group_id: GroupId,
    collapsed: bool,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| {
        registry.set_group_collapsed(&group_id, collapsed)
    })
    .await
}

/// Refreshes all repository summaries; results arrive as events.
#[tauri::command]
pub fn refresh_summaries(state: State<'_, AppState>) {
    state.spawn_summary_refresh(None);
}

/// Status of a repository, which becomes the watched one.
#[tauri::command]
pub async fn get_status(
    state: State<'_, AppState>,
    repo_id: RepoId,
) -> CommandResult<StatusSnapshot> {
    let git = state.git().await?;
    Ok(state.workspace.status(git, &repo_id).await?)
}

#[tauri::command]
pub async fn get_diff(
    state: State<'_, AppState>,
    repo_id: RepoId,
    request: DiffRequest,
) -> CommandResult<FileDiff> {
    let git = state.git().await?;
    Ok(state.workspace.diff(git, &repo_id, &request).await?)
}

#[tauri::command]
pub async fn stage_paths(
    state: State<'_, AppState>,
    repo_id: RepoId,
    paths: Vec<String>,
) -> CommandResult<StatusSnapshot> {
    let git = state.git().await?;
    Ok(state.workspace.stage(git, &repo_id, &paths).await?)
}

#[tauri::command]
pub async fn unstage_paths(
    state: State<'_, AppState>,
    repo_id: RepoId,
    paths: Vec<String>,
) -> CommandResult<StatusSnapshot> {
    let git = state.git().await?;
    Ok(state.workspace.unstage(git, &repo_id, &paths).await?)
}

/// Discards unstaged changes; untracked files go to the trash.
#[tauri::command]
pub async fn discard_paths(
    state: State<'_, AppState>,
    repo_id: RepoId,
    paths: Vec<String>,
) -> CommandResult<StatusSnapshot> {
    let git = state.git().await?;
    Ok(state.workspace.discard(git, &repo_id, &paths).await?)
}

/// Commits the staged changes; `amend` replaces the last commit instead.
#[tauri::command]
pub async fn commit(
    state: State<'_, AppState>,
    repo_id: RepoId,
    message: String,
    amend: bool,
) -> CommandResult<CommitResult> {
    let git = state.git().await?;
    state
        .workspace
        .commit(git, &repo_id, &message, amend)
        .await
        .map_err(|error| AppError::from(error).when_git_failed("Commit failed."))
}

/// The commit `HEAD` points to; `null` before the first commit.
#[tauri::command]
pub async fn get_head_commit(
    state: State<'_, AppState>,
    repo_id: RepoId,
) -> CommandResult<Option<HeadCommit>> {
    let git = state.git().await?;
    Ok(state.workspace.head_commit(git, &repo_id).await?)
}

/// Takes back the last commit if it is not on a remote; its changes stay
/// staged.
#[tauri::command]
pub async fn undo_last_commit(
    state: State<'_, AppState>,
    repo_id: RepoId,
) -> CommandResult<UndoCommitResult> {
    let git = state.git().await?;
    state
        .workspace
        .undo_last_commit(git, &repo_id)
        .await
        .map_err(|error| AppError::from(error).when_git_failed("Could not undo the commit."))
}

#[tauri::command]
pub async fn get_branches(
    state: State<'_, AppState>,
    repo_id: RepoId,
) -> CommandResult<BranchList> {
    let git = state.git().await?;
    Ok(state.workspace.branches(git, &repo_id).await?)
}

/// Opens the repository, or one of its files at `line`, in the editor.
#[tauri::command]
pub async fn open_in_editor(
    state: State<'_, AppState>,
    repo_id: RepoId,
    path: Option<String>,
    line: Option<u32>,
) -> CommandResult<()> {
    let editor = state.editor().await?;
    Ok(state
        .workspace
        .open_in_editor(&editor, &repo_id, path.as_deref(), line)
        .await?)
}

#[tauri::command]
pub async fn get_branch_commits(
    state: State<'_, AppState>,
    repo_id: RepoId,
    branch: String,
) -> CommandResult<BranchCommits> {
    let git = state.git().await?;
    Ok(state
        .workspace
        .branch_commits(git, &repo_id, &branch)
        .await?)
}

/// The files a branch (against the mainline) or a single commit changes.
#[tauri::command]
pub async fn get_review_files(
    state: State<'_, AppState>,
    repo_id: RepoId,
    target: ReviewTarget,
) -> CommandResult<ReviewFiles> {
    let git = state.git().await?;
    Ok(state.workspace.review_files(git, &repo_id, &target).await?)
}

#[tauri::command]
pub async fn get_review_diff(
    state: State<'_, AppState>,
    repo_id: RepoId,
    request: ReviewDiffRequest,
) -> CommandResult<FileDiff> {
    let git = state.git().await?;
    Ok(state.workspace.review_diff(git, &repo_id, &request).await?)
}

#[tauri::command]
pub async fn switch_branch(
    state: State<'_, AppState>,
    repo_id: RepoId,
    branch: String,
) -> CommandResult<BranchList> {
    let git = state.git().await?;
    state
        .workspace
        .switch_branch(git, &repo_id, &branch)
        .await
        .map_err(|error| {
            AppError::from(error).when_git_failed(format!("Could not switch to {branch}."))
        })
}

#[tauri::command]
pub async fn fetch_repo(state: State<'_, AppState>, repo_id: RepoId) -> CommandResult<BranchList> {
    let git = state.git().await?;
    state
        .workspace
        .fetch(git, &repo_id)
        .await
        .map_err(|error| AppError::from(error).when_git_failed("Fetching failed."))
}

/// Sets the ref branches are compared with; `null` restores the default.
#[tauri::command]
pub async fn set_mainline(
    state: State<'_, AppState>,
    repo_id: RepoId,
    mainline: Option<String>,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| {
        registry.set_mainline(&repo_id, mainline.as_deref())
    })
    .await
}

/// Minutes between background fetches; 0 turns them off.
#[tauri::command]
pub async fn set_fetch_interval(
    state: State<'_, AppState>,
    minutes: u32,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| {
        registry.set_fetch_interval(minutes);
        Ok(())
    })
    .await
}

/// Font size of the embedded terminal; kept within a sensible range.
#[tauri::command]
pub async fn set_terminal_font_size(
    state: State<'_, AppState>,
    size: u16,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| {
        registry.set_terminal_font_size(size);
        Ok(())
    })
    .await
}

/// No sounds when agents need the user; notifications still show.
#[tauri::command]
pub async fn set_sound_muted(
    state: State<'_, AppState>,
    muted: bool,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| {
        registry.set_sound_muted(muted);
        Ok(())
    })
    .await
}

/// No sound for an agent the user is watching in the terminal.
#[tauri::command]
pub async fn set_mute_when_watching(
    state: State<'_, AppState>,
    mute: bool,
) -> CommandResult<WorkspaceSnapshot> {
    update(&state, |registry| {
        registry.set_mute_when_watching(mute);
        Ok(())
    })
    .await
}

async fn update(
    state: &AppState,
    change: impl FnOnce(&mut workspace::Registry) -> Result<(), workspace::WorkspaceError>,
) -> CommandResult<WorkspaceSnapshot> {
    let ((), snapshot) = state.workspace.update(change).await?;
    Ok(snapshot)
}

/// The coding agents per repository. Connects to herdr first if needed;
/// later changes arrive as `repo-agents` events.
#[tauri::command]
pub async fn get_agents(state: State<'_, AppState>) -> CommandResult<AgentsSnapshot> {
    state.herdr.connect().await;
    Ok(state.agents().await)
}

/// Shows the repository's herdr workspace, or creates one in it.
#[tauri::command]
pub async fn open_in_herdr(state: State<'_, AppState>, repo_id: RepoId) -> CommandResult<()> {
    let workspace = state.workspace.snapshot().await;
    let repo = workspace
        .repos
        .iter()
        .find(|repo| repo.id == repo_id)
        .ok_or(workspace::WorkspaceError::RepoNotFound)?;
    state
        .herdr
        .open(&repo_matcher(&workspace), &repo_id, &repo.name)
        .await?;
    Ok(())
}

/// Shows the repository's herdr workspace if it has one; never creates one.
/// Returns whether there was one.
#[tauri::command]
pub async fn focus_repo_in_herdr(
    state: State<'_, AppState>,
    repo_id: RepoId,
) -> CommandResult<bool> {
    let workspace = state.workspace.snapshot().await;
    if !workspace.repos.iter().any(|repo| repo.id == repo_id) {
        return Err(workspace::WorkspaceError::RepoNotFound.into());
    }
    let focused = state
        .herdr
        .focus_repo(&repo_matcher(&workspace), &repo_id)
        .await?;
    Ok(focused.is_some())
}

/// Shows an agent in herdr: its workspace, tab and pane. A finished agent
/// counts as seen afterwards.
#[tauri::command]
pub async fn focus_agent(state: State<'_, AppState>, pane_id: String) -> CommandResult<()> {
    Ok(state.herdr.focus_agent(&pane_id).await?)
}

/// Puts `text` into the agent's input as one paste, without submitting it.
#[tauri::command]
pub async fn paste_to_agent(
    state: State<'_, AppState>,
    pane_id: String,
    text: String,
) -> CommandResult<()> {
    Ok(state.herdr.paste(&pane_id, &text).await?)
}

/// Whether the right column shows the terminal; an agent in it may be
/// watched, which can silence its alerts.
#[tauri::command]
pub fn set_terminal_visible(state: State<'_, AppState>, visible: bool) {
    state.set_terminal_visible(visible);
}

/// Whether the embedded terminal can run herdr.
#[tauri::command]
pub async fn terminal_info(state: State<'_, AppState>) -> CommandResult<TerminalInfo> {
    Ok(TerminalInfo {
        available: state.herdr_binary().await.is_some(),
    })
}

/// Starts the embedded herdr client, replacing one that still runs. Output
/// arrives on `output` as raw bytes, followed by `{ "exit": code }`.
#[tauri::command]
pub async fn terminal_open(
    state: State<'_, AppState>,
    cols: u16,
    rows: u16,
    output: Channel<InvokeResponseBody>,
) -> CommandResult<()> {
    let command = state.terminal_command().await?;
    state
        .terminal
        .open(&command, pty::Size { cols, rows }, output)?;
    state.spawn_herdr_connect_after_start();
    Ok(())
}

/// Input from the user, as typed or pasted.
#[tauri::command]
pub fn terminal_write(state: State<'_, AppState>, data: String) -> CommandResult<()> {
    state.terminal.write(data.as_bytes())
}

#[tauri::command]
pub fn terminal_resize(state: State<'_, AppState>, cols: u16, rows: u16) -> CommandResult<()> {
    state.terminal.resize(pty::Size { cols, rows })
}

/// Ends the embedded client; herdr's server and its agents keep running.
#[tauri::command]
pub fn terminal_close(state: State<'_, AppState>) {
    state.terminal.close();
}
