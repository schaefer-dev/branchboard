// Typed wrappers for all backend commands. Each entry must match a
// `#[tauri::command]` in `src-tauri/src/commands.rs`; argument and result
// types come from the generated ts-rs bindings.

import { call } from "./client";
import type { AppInfo } from "./generated/AppInfo";
import type { GitInfo } from "./generated/GitInfo";

export const api = {
  getAppInfo: () => call<AppInfo>("get_app_info"),
  getGitInfo: () => call<GitInfo>("get_git_info"),
} as const;
