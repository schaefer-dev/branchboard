// Typed wrappers for all backend commands. Each entry must match a
// `#[tauri::command]` in `src-tauri/src/commands.rs`; argument and result
// types come from the generated ts-rs bindings.

import { call } from "./client";
import type { AddRepoResult } from "./generated/AddRepoResult";
import type { AppInfo } from "./generated/AppInfo";
import type { GitInfo } from "./generated/GitInfo";
import type { GroupId } from "./generated/GroupId";
import type { RepoId } from "./generated/RepoId";
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
} as const;
