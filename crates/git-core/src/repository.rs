//! Repository-level queries.

use std::path::{Path, PathBuf};

use crate::summary::{RepoSummary, parse_summary};
use crate::{Git, GitError};

impl Git {
    /// Resolves `path` (the repository root or any folder inside it) to the
    /// root of its working tree. Bare repositories are rejected.
    pub async fn resolve_work_tree(&self, path: &Path) -> Result<PathBuf, GitError> {
        if !path.is_dir() {
            return Err(GitError::NotARepository {
                path: path.to_owned(),
                stderr: "not a folder".into(),
            });
        }
        let not_a_repository = |error: GitError| match error {
            GitError::Failed { stderr, .. } => GitError::NotARepository {
                path: path.to_owned(),
                stderr,
            },
            other => other,
        };

        let bare = self
            .runner()
            .command(["rev-parse", "--is-bare-repository"])
            .current_dir(path)
            .output()
            .await
            .map_err(not_a_repository)?;
        if String::from_utf8_lossy(&bare.stdout).trim() == "true" {
            return Err(GitError::BareRepository {
                path: path.to_owned(),
            });
        }

        let toplevel = self
            .runner()
            .command(["rev-parse", "--show-toplevel"])
            .current_dir(path)
            .output()
            .await
            .map_err(not_a_repository)?;
        let toplevel = String::from_utf8(toplevel.stdout)
            .map_err(|_| GitError::Parse("repository path is not valid UTF-8".into()))?;
        Ok(PathBuf::from(toplevel.trim_end_matches(['\r', '\n'])))
    }

    /// Current HEAD and dirty state of the working tree at `repo`.
    pub async fn summary(&self, repo: &Path) -> Result<RepoSummary, GitError> {
        let output = self
            .runner()
            .command([
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
                "--untracked-files=normal",
            ])
            .current_dir(repo)
            .output()
            .await?;
        parse_summary(&output.stdout)
    }
}
