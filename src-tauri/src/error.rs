use git_core::GitError;
use herdr::HerdrError;
use serde::Serialize;
use ts_rs::TS;
use workspace::WorkspaceError;

/// Error returned by every command. Serialized as a plain object so the
/// frontend can branch on `kind` and show `message`/`detail`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppError {
    pub kind: AppErrorKind,
    /// User-facing, complete sentence.
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
    NotARepository,
    NotFound,
    InvalidInput,
    EditorNotFound,
    /// No herdr session is running.
    HerdrUnavailable,
    /// herdr is running but the request failed.
    HerdrFailed,
    Io,
    /// Unexpected failures, e.g. a rejected IPC call or a crashed task.
    Internal,
}

impl AppError {
    /// Replaces the generic message of a failed git command with one that
    /// names what the user tried; git's explanation stays in `detail`.
    pub fn when_git_failed(mut self, message: impl Into<String>) -> Self {
        if self.kind == AppErrorKind::GitFailed {
            self.message = message.into();
        }
        self
    }

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
        use AppErrorKind as Kind;

        let technical = error.to_string();
        match error {
            GitError::NotFound => Self::new(
                Kind::GitNotFound,
                "Git was not found. Install git and restart Branchboard.",
                None,
            ),
            GitError::UnsupportedVersion { found, required } => Self::new(
                Kind::GitUnsupported,
                format!("Git {found} is too old. Update git to {required} or newer."),
                None,
            ),
            GitError::UnknownVersion { output } => Self::new(
                Kind::GitUnsupported,
                "The installed git did not report a version Branchboard understands.",
                Some(output),
            ),
            GitError::NotARepository { path, stderr } => Self::new(
                Kind::NotARepository,
                format!("{} is not a git repository.", path.display()),
                Some(stderr),
            ),
            GitError::BareRepository { path } => Self::new(
                Kind::NotARepository,
                format!(
                    "{} is a bare repository. Choose a folder with a working tree.",
                    path.display()
                ),
                None,
            ),
            GitError::Failed { stderr, .. } => {
                Self::new(Kind::GitFailed, "Git reported an error.", Some(stderr))
            }
            GitError::Spawn(source) => Self::new(
                Kind::GitFailed,
                "Git could not be started.",
                Some(source.to_string()),
            ),
            GitError::Timeout { .. } | GitError::Parse(_) => Self::new(
                Kind::GitFailed,
                "Git did not respond as expected.",
                Some(technical),
            ),
        }
    }
}

impl From<WorkspaceError> for AppError {
    fn from(error: WorkspaceError) -> Self {
        use AppErrorKind as Kind;

        let message = error.to_string();
        match error {
            WorkspaceError::Git(error) => error.into(),
            WorkspaceError::RepoNotFound | WorkspaceError::GroupNotFound => {
                Self::new(Kind::NotFound, message, None)
            }
            WorkspaceError::EmptyName
            | WorkspaceError::DuplicateGroup(_)
            | WorkspaceError::AlreadyRegistered(_) => Self::new(Kind::InvalidInput, message, None),
            WorkspaceError::Save(source) => Self::new(Kind::Io, message, Some(source.to_string())),
            WorkspaceError::Highlight(detail) => Self::new(Kind::Internal, message, Some(detail)),
            WorkspaceError::Trash(detail) | WorkspaceError::Editor(detail) => {
                Self::new(Kind::Io, message, Some(detail))
            }
            WorkspaceError::EditorNotFound => Self::new(Kind::EditorNotFound, message, None),
        }
    }
}

impl From<HerdrError> for AppError {
    fn from(error: HerdrError) -> Self {
        let kind = match error {
            HerdrError::NotRunning => AppErrorKind::HerdrUnavailable,
            _ => AppErrorKind::HerdrFailed,
        };
        Self::new(kind, error.to_string(), error.detail())
    }
}
