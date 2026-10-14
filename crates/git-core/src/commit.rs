//! Creating commits and taking back the last one.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use ts_rs::TS;

use crate::{Git, GitError};

/// Commit hooks may run formatters or tests; give them time.
const COMMIT_TIMEOUT: Duration = Duration::from_mins(5);

/// The commit `HEAD` points to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HeadCommit {
    pub oid: String,
    pub short_oid: String,
    pub subject: String,
    /// The full message, without trailing whitespace.
    pub message: String,
    /// `false` for the first commit of a history.
    pub has_parent: bool,
    /// A remote-tracking branch already contains the commit, so rewriting it
    /// affects others.
    pub pushed: bool,
}

const FIELD: char = '\u{1f}';

impl Git {
    /// Commits the index with `message`, or replaces the last commit with
    /// `amend`. Hooks and signing run as configured.
    pub async fn commit(&self, repo: &Path, message: &str, amend: bool) -> Result<(), GitError> {
        self.runner()
            .command(["commit", "--quiet", "--file=-", "--cleanup=whitespace"])
            .args(amend.then_some("--amend"))
            .stdin(message.as_bytes().to_vec())
            .timeout(COMMIT_TIMEOUT)
            .current_dir(repo)
            .output()
            .await?;
        Ok(())
    }

    /// The commit `HEAD` points to, or `None` on a branch without commits.
    pub async fn head_commit(&self, repo: &Path) -> Result<Option<HeadCommit>, GitError> {
        let resolved = self
            .runner()
            .command(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
            .success_codes(&[0, 1])
            .current_dir(repo)
            .output()
            .await?;
        if resolved.exit_code != 0 {
            return Ok(None);
        }
        let oid = String::from_utf8_lossy(&resolved.stdout).trim().to_owned();

        let output = self
            .runner()
            .command(["log", "-1", "--no-color"])
            .args([
                format!("--format=%h{FIELD}%P{FIELD}%s{FIELD}%B"),
                "--end-of-options".to_owned(),
                oid.clone(),
                "--".to_owned(),
            ])
            .current_dir(repo)
            .output()
            .await?;
        let record = String::from_utf8_lossy(&output.stdout);
        let fields: Vec<&str> = record.splitn(4, FIELD).collect();
        let [short_oid, parents, subject, message] = fields.as_slice() else {
            return Err(GitError::Parse(format!("unexpected log record {record:?}")));
        };

        let containing = self
            .runner()
            .command(["for-each-ref", "--count=1", "--format=%(refname)"])
            .args([format!("--contains={oid}"), "refs/remotes".to_owned()])
            .current_dir(repo)
            .output()
            .await?;

        Ok(Some(HeadCommit {
            short_oid: (*short_oid).to_owned(),
            has_parent: !parents.trim().is_empty(),
            subject: (*subject).to_owned(),
            message: message.trim_end().to_owned(),
            pushed: !containing.stdout.trim_ascii().is_empty(),
            oid,
        }))
    }

    /// Moves the branch back to the parent of `HEAD`, keeping its changes
    /// staged.
    pub async fn undo_last_commit(&self, repo: &Path) -> Result<(), GitError> {
        self.runner()
            .command(["reset", "--soft", "--quiet", "HEAD~1"])
            .current_dir(repo)
            .output()
            .await?;
        Ok(())
    }
}
