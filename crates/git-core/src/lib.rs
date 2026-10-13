//! Git domain types and the git CLI adapter.
//!
//! This crate has no dependency on Tauri so it can be tested in isolation.
//! All interaction with git goes through the [`GitRunner`], which spawns the
//! user's git executable with a fixed, safe set of global options.

mod branches;
mod changes;
mod diff;
mod discovery;
mod error;
mod log;
mod repository;
mod runner;
mod status;
mod summary;
mod version;

use std::path::Path;

pub use branches::{AheadBehind, Branch, BranchList, UpstreamState};
pub use changes::{DiffArea, DiffRequest, DiffSources};
pub use diff::{
    DiffHunk, DiffLimits, DiffLine, DiffLineKind, FileDiff, Token, TokenKind, parse_diff,
};
pub use discovery::DiscoveryOptions;
pub use error::GitError;
pub use log::{CommitList, CommitSummary};
pub use runner::{GitCommand, GitOutput, GitRunner};
pub use status::{ChangeKind, FileChange, StatusSnapshot};
pub use summary::{Head, RepoSummary};
pub use version::GitVersion;

/// A located and verified git installation.
#[derive(Debug, Clone)]
pub struct Git {
    runner: GitRunner,
    version: GitVersion,
}

impl Git {
    /// Locates the git executable, checks its version and returns a handle
    /// that is ready to run commands.
    pub async fn discover(options: DiscoveryOptions) -> Result<Self, GitError> {
        let search_path = discovery::effective_search_path().await;
        let program = discovery::locate_git(&options, &search_path)?;
        let runner = GitRunner::new(program, search_path);

        let output = runner.command(["version"]).output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = GitVersion::parse(&stdout).ok_or_else(|| GitError::UnknownVersion {
            output: stdout.trim().to_owned(),
        })?;
        if version < GitVersion::MIN_SUPPORTED {
            return Err(GitError::UnsupportedVersion {
                found: version,
                required: GitVersion::MIN_SUPPORTED,
            });
        }

        tracing::info!(program = %runner.program().display(), %version, "git discovered");
        Ok(Self { runner, version })
    }

    pub fn version(&self) -> GitVersion {
        self.version
    }

    pub fn program(&self) -> &Path {
        self.runner.program()
    }

    pub fn runner(&self) -> &GitRunner {
        &self.runner
    }
}
