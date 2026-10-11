use std::time::Duration;

use crate::GitVersion;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git executable not found")]
    NotFound,

    #[error("git {found} is not supported, at least {required} is required")]
    UnsupportedVersion {
        found: GitVersion,
        required: GitVersion,
    },

    #[error("could not determine the git version from {output:?}")]
    UnknownVersion { output: String },

    #[error("failed to start git")]
    Spawn(#[source] std::io::Error),

    #[error("`git {command}` timed out after {timeout:?}")]
    Timeout { command: String, timeout: Duration },

    #[error("`git {command}` failed with exit code {code:?}")]
    Failed {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
}
