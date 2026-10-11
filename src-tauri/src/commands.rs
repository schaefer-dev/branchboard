//! Commands invoked from the frontend. Keep these thin: translate between
//! IPC types and the core crates, nothing else.

use tauri::{AppHandle, State};

use crate::dto::{AppInfo, GitInfo};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri injects command arguments by value"
)]
pub fn get_app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub async fn get_git_info(state: State<'_, AppState>) -> Result<GitInfo, AppError> {
    let git = state.git().await?;
    Ok(GitInfo {
        path: git.program().display().to_string(),
        version: git.version(),
    })
}
