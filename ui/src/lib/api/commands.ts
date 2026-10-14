// Typed wrappers for all backend commands. Each entry must match a
// `#[tauri::command]` in `src-tauri/src/commands.rs`; argument and result
// types come from the generated ts-rs bindings.

import type { Channel } from "@tauri-apps/api/core";

import { call } from "./client";
import type { AddRepoResult } from "./generated/AddRepoResult";
import type { AgentsSnapshot } from "./generated/AgentsSnapshot";
import type { AppInfo } from "./generated/AppInfo";
import type { BranchCommits } from "./generated/BranchCommits";
import type { BranchList } from "./generated/BranchList";
import type { DiffRequest } from "./generated/DiffRequest";
import type { FileDiff } from "./generated/FileDiff";
import type { GitInfo } from "./generated/GitInfo";
import type { GroupId } from "./generated/GroupId";
import type { RepoId } from "./generated/RepoId";
import type { StatusSnapshot } from "./generated/StatusSnapshot";
import type { TerminalExit } from "./generated/TerminalExit";
import type { TerminalInfo } from "./generated/TerminalInfo";
import type { WorkspaceSnapshot } from "./generated/WorkspaceSnapshot";

export const api = {
  getAppInfo: () => call<AppInfo>("get_app_info"),
  getGitInfo: () => call<GitInfo>("get_git_info"),

  getWorkspace: () => call<WorkspaceSnapshot>("get_workspace"),
  addRepo: (path: string) => call<AddRepoResult>("add_repo", { path }),
  relocateRepo: (repoId: RepoId, path: string) =>
    call<WorkspaceSnapshot>("relocate_repo", { repoId, path }),
  removeRepo: (repoId: RepoId) => call<WorkspaceSnapshot>("remove_repo", { repoId }),
  renameRepo: (repoId: RepoId, name: string | null) =>
    call<WorkspaceSnapshot>("rename_repo", { repoId, name }),
  setRepoFavorite: (repoId: RepoId, favorite: boolean) =>
    call<WorkspaceSnapshot>("set_repo_favorite", { repoId, favorite }),
  setRepoGroup: (repoId: RepoId, groupId: GroupId | null) =>
    call<WorkspaceSnapshot>("set_repo_group", { repoId, groupId }),
  setActiveRepo: (repoId: RepoId | null) => call<WorkspaceSnapshot>("set_active_repo", { repoId }),
  createGroup: (name: string, repoId: RepoId | null) =>
    call<WorkspaceSnapshot>("create_group", { name, repoId }),
  renameGroup: (groupId: GroupId, name: string) =>
    call<WorkspaceSnapshot>("rename_group", { groupId, name }),
  deleteGroup: (groupId: GroupId) => call<WorkspaceSnapshot>("delete_group", { groupId }),
  setGroupCollapsed: (groupId: GroupId, collapsed: boolean) =>
    call<WorkspaceSnapshot>("set_group_collapsed", { groupId, collapsed }),
  refreshSummaries: () => call<null>("refresh_summaries"),

  getStatus: (repoId: RepoId) => call<StatusSnapshot>("get_status", { repoId }),
  getDiff: (repoId: RepoId, request: DiffRequest) =>
    call<FileDiff>("get_diff", { repoId, request }),
  stagePaths: (repoId: RepoId, paths: string[]) =>
    call<StatusSnapshot>("stage_paths", { repoId, paths }),
  unstagePaths: (repoId: RepoId, paths: string[]) =>
    call<StatusSnapshot>("unstage_paths", { repoId, paths }),
  discardPaths: (repoId: RepoId, paths: string[]) =>
    call<StatusSnapshot>("discard_paths", { repoId, paths }),

  openInEditor: (repoId: RepoId, path: string | null, line: number | null) =>
    call<null>("open_in_editor", { repoId, path, line }),

  getBranches: (repoId: RepoId) => call<BranchList>("get_branches", { repoId }),
  getBranchCommits: (repoId: RepoId, branch: string) =>
    call<BranchCommits>("get_branch_commits", { repoId, branch }),
  switchBranch: (repoId: RepoId, branch: string) =>
    call<BranchList>("switch_branch", { repoId, branch }),
  fetchRepo: (repoId: RepoId) => call<BranchList>("fetch_repo", { repoId }),
  setMainline: (repoId: RepoId, mainline: string | null) =>
    call<WorkspaceSnapshot>("set_mainline", { repoId, mainline }),
  setFetchInterval: (minutes: number) => call<WorkspaceSnapshot>("set_fetch_interval", { minutes }),
  setTerminalFontSize: (size: number) =>
    call<WorkspaceSnapshot>("set_terminal_font_size", { size }),

  getAgents: () => call<AgentsSnapshot>("get_agents"),
  openInHerdr: (repoId: RepoId) => call<null>("open_in_herdr", { repoId }),
  focusAgent: (paneId: string) => call<null>("focus_agent", { paneId }),
  /** Shows the repository's herdr workspace if it has one; never creates one. */
  focusRepoInHerdr: (repoId: RepoId) => call<boolean>("focus_repo_in_herdr", { repoId }),

  terminalInfo: () => call<TerminalInfo>("terminal_info"),
  /** Output arrives on `output` as `ArrayBuffer`s, then one `TerminalExit`. */
  terminalOpen: (cols: number, rows: number, output: Channel<ArrayBuffer | TerminalExit>) =>
    call<null>("terminal_open", { cols, rows, output }),
  terminalWrite: (data: string) => call<null>("terminal_write", { data }),
  terminalResize: (cols: number, rows: number) => call<null>("terminal_resize", { cols, rows }),
  terminalClose: () => call<null>("terminal_close"),
} as const;
