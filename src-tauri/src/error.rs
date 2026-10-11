use git_core::GitError;
use serde::Serialize;
use ts_rs::TS;

/// Error returned by every command. Serialized as a plain object so the
/// frontend can branch on `kind` and show `message`/`detail`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppError {
    pub kind: AppErrorKind,
    pub message: String,
    /// Technical details such as git's stderr.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AppErrorKind {
    GitNotFound,
    GitUnsupported,
    GitFailed,
    /// Unexpected failures, e.g. a rejected IPC call. Also produced by the
    /// frontend for errors that never reached a command.
    #[expect(dead_code, reason = "not yet produced by the backend")]
    Internal,
}

impl AppError {
    fn new(kind: AppErrorKind, message: impl Into<String>, detail: Option<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail,
        }
    }
}

impl From<GitError> for AppError {
    fn from(error: GitError) -> Self {
        let message = error.to_string();
        match error {
            GitError::NotFound => Self::new(AppErrorKind::GitNotFound, message, None),
            GitError::UnsupportedVersion { .. } | GitError::UnknownVersion { .. } => {
                Self::new(AppErrorKind::GitUnsupported, message, None)
            }
            GitError::Failed { stderr, .. } => {
                Self::new(AppErrorKind::GitFailed, message, Some(stderr))
            }
            GitError::Spawn(source) => {
                Self::new(AppErrorKind::GitFailed, message, Some(source.to_string()))
            }
            GitError::Timeout { .. } => Self::new(AppErrorKind::GitFailed, message, None),
        }
    }
}
