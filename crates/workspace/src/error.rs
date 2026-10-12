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

    #[error("A group named \u{201c}{0}\u{201d} already exists.")]
    DuplicateGroup(String),

    #[error("{} is already in your list.", .0.display())]
    AlreadyRegistered(PathBuf),

    #[error(transparent)]
    Git(#[from] GitError),

    #[error("The files could not be moved to the trash.")]
    Trash(String),

    #[error("Your repository list could not be saved.")]
    Save(#[source] std::io::Error),
}
