use std::path::PathBuf;

use git_core::GitError;

/// Errors of workspace operations. Messages are written for end users.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("This repository is no longer in your list.")]
    RepoNotFound,

    #[error("This group no longer exists.")]
    GroupNotFound,

    #[error("Enter a name.")]
    EmptyName,

    #[error("Enter a commit message.")]
    EmptyMessage,

    #[error("A group named \u{201c}{0}\u{201d} already exists.")]
    DuplicateGroup(String),

    #[error("{} is already in your list.", .0.display())]
    AlreadyRegistered(PathBuf),

    #[error(transparent)]
    Git(#[from] GitError),

    #[error("This commit is not available.")]
    InvalidRevision,

    #[error("There is no earlier commit to go back to.")]
    NothingToUndo,

    #[error("This commit is already on a remote. Undoing it would rewrite shared history.")]
    CommitPublished,

    #[error(
        "VS Code was not found. In VS Code, run \"Shell Command: Install 'code' command in PATH\"."
    )]
    EditorNotFound,

    #[error("The editor could not be started.")]
    Editor(String),

    #[error("The diff could not be prepared.")]
    Highlight(String),

    #[error("The files could not be moved to the trash.")]
    Trash(String),

    #[error("Your repository list could not be saved.")]
    Save(#[source] std::io::Error),
}
