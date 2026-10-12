//! Reading and changing the working tree and the index of a repository.

use std::path::Path;

use serde::Deserialize;
use ts_rs::TS;

use crate::diff::{DiffLimits, FileDiff, parse_diff};
use crate::status::{ChangeKind, StatusSnapshot, parse_status};
use crate::{Git, GitError};

/// Which side of the index a diff shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DiffArea {
    /// Index vs. `HEAD`.
    Staged,
    /// Working tree vs. index.
    Unstaged,
}

/// A request for the diff of one listed change.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiffRequest {
    pub path: String,
    pub orig_path: Option<String>,
    pub area: DiffArea,
    pub kind: ChangeKind,
    /// Ignore the size limits.
    pub full: bool,
}

const DIFF_ARGS: &[&str] = &["--no-ext-diff", "--no-color", "-U3"];

impl Git {
    pub async fn status(&self, repo: &Path) -> Result<StatusSnapshot, GitError> {
        let output = self
            .runner()
            .command([
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
                "--untracked-files=all",
            ])
            .current_dir(repo)
            .output()
            .await?;
        parse_status(&output.stdout)
    }

    pub async fn diff(&self, repo: &Path, request: &DiffRequest) -> Result<FileDiff, GitError> {
        let path = request.path.as_str();
        let command = match (request.area, request.kind) {
            // `git diff --no-index` exits with 1 when the files differ.
            (DiffArea::Unstaged, ChangeKind::Untracked) => self
                .runner()
                .command(["diff", "--no-index"])
                .args(DIFF_ARGS)
                .args(["--", "/dev/null", path])
                .success_codes(&[0, 1]),
            // A conflicted file is shown against HEAD rather than as a
            // combined diff of both sides.
            (DiffArea::Unstaged, ChangeKind::Conflicted) => self
                .runner()
                .command(["diff", "HEAD"])
                .args(DIFF_ARGS)
                .args(["--", path]),
            (DiffArea::Unstaged, _) => self
                .runner()
                .command(["diff"])
                .args(DIFF_ARGS)
                .args(["--", path]),
            (DiffArea::Staged, _) => self
                .runner()
                .command(["diff", "--cached", "-M"])
                .args(DIFF_ARGS)
                .args(["--", path])
                .args(request.orig_path.as_deref()),
        };
        let output = command.current_dir(repo).output().await?;
        let limits = if request.full {
            DiffLimits::UNLIMITED
        } else {
            DiffLimits::DEFAULT
        };
        parse_diff(&output.stdout, limits)
    }

    /// Stages the given paths, including deletions.
    pub async fn stage(&self, repo: &Path, paths: &[String]) -> Result<(), GitError> {
        if paths.is_empty() {
            // An empty pathspec would mean "everything".
            return Ok(());
        }
        self.runner()
            .command(["add", "--all"])
            .pathspecs_from_stdin(paths)
            .current_dir(repo)
            .output()
            .await?;
        Ok(())
    }

    /// Removes the given paths from the index, keeping the working tree.
    /// Renames must include their original path to be undone completely.
    pub async fn unstage(
        &self,
        repo: &Path,
        paths: &[String],
        unborn: bool,
    ) -> Result<(), GitError> {
        if paths.is_empty() {
            return Ok(());
        }
        // Without a commit there is nothing to restore from; drop the entries.
        let command: &[&str] = if unborn {
            &["rm", "--cached", "--quiet", "-r", "--ignore-unmatch"]
        } else {
            &["restore", "--staged"]
        };
        self.runner()
            .command(command)
            .pathspecs_from_stdin(paths)
            .current_dir(repo)
            .output()
            .await?;
        Ok(())
    }

    /// Discards unstaged changes to tracked files by restoring them from the
    /// index.
    pub async fn restore_worktree(&self, repo: &Path, paths: &[String]) -> Result<(), GitError> {
        if paths.is_empty() {
            return Ok(());
        }
        self.runner()
            .command(["restore", "--worktree"])
            .pathspecs_from_stdin(paths)
            .current_dir(repo)
            .output()
            .await?;
        Ok(())
    }
}
