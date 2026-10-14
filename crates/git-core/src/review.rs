//! Reviewing committed work: the files and diffs between two commits, for a
//! whole branch against the mainline or for a single commit.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::changes::DiffSources;
use crate::diff::{DiffLimits, FileDiff, parse_diff};
use crate::status::ChangeKind;
use crate::{Git, GitError};

/// What to review.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ReviewTarget {
    /// Everything a local branch adds since it left the mainline.
    Branch { name: String },
    /// One commit, compared with its first parent.
    Commit { oid: String },
}

/// The two commits a review compares, resolved once so that its file diffs
/// stay consistent while the branch moves on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewRange {
    /// The merge base or parent; the empty tree for a root commit or
    /// unrelated histories.
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewFile {
    /// Path relative to the repository root, with `/` separators.
    pub path: String,
    /// Previous path of a rename or copy.
    pub orig_path: Option<String>,
    pub kind: ChangeKind,
    /// Added and deleted lines; `None` for binary files.
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewFiles {
    pub range: ReviewRange,
    pub files: Vec<ReviewFile>,
}

/// A request for the diff of one file of a review.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReviewDiffRequest {
    pub range: ReviewRange,
    pub path: String,
    pub orig_path: Option<String>,
    /// Ignore the size limits.
    pub full: bool,
}

const DIFF_ARGS: &[&str] = &["--no-ext-diff", "--no-color", "-M"];

impl Git {
    /// Resolves a review target to the commits it compares. A branch is
    /// compared from its merge base with `mainline`.
    pub async fn review_range(
        &self,
        repo: &Path,
        target: &ReviewTarget,
        mainline: &str,
    ) -> Result<ReviewRange, GitError> {
        match target {
            ReviewTarget::Branch { name } => {
                let to = self
                    .resolve_commit(repo, &format!("refs/heads/{name}"))
                    .await?
                    .ok_or_else(|| missing(name))?;
                let base = self
                    .runner()
                    .command(["merge-base", "--end-of-options", mainline, &to])
                    .success_codes(&[0, 1])
                    .current_dir(repo)
                    .output()
                    .await?;
                let from = match base.exit_code {
                    0 => String::from_utf8_lossy(&base.stdout).trim().to_owned(),
                    // No common history.
                    _ => self.empty_tree(repo).await?,
                };
                Ok(ReviewRange { from, to })
            }
            ReviewTarget::Commit { oid } => {
                let to = self
                    .resolve_commit(repo, oid)
                    .await?
                    .ok_or_else(|| missing(oid))?;
                let from = match self.resolve_commit(repo, &format!("{to}^1")).await? {
                    Some(parent) => parent,
                    None => self.empty_tree(repo).await?,
                };
                Ok(ReviewRange { from, to })
            }
        }
    }

    /// The files that differ between the two commits of `range`.
    pub async fn review_files(
        &self,
        repo: &Path,
        range: &ReviewRange,
    ) -> Result<Vec<ReviewFile>, GitError> {
        let output = self
            .runner()
            .command(["diff", "--raw", "--numstat", "-z"])
            .args(DIFF_ARGS)
            .args(["--end-of-options", &range.from, &range.to, "--"])
            .current_dir(repo)
            .output()
            .await?;
        parse_raw_numstat(&output.stdout)
    }

    /// The diff of one file between the two commits of `range`.
    pub async fn review_diff(
        &self,
        repo: &Path,
        request: &ReviewDiffRequest,
    ) -> Result<FileDiff, GitError> {
        let range = &request.range;
        let output = self
            .runner()
            .command(["diff", "-U3"])
            .args(DIFF_ARGS)
            .args(["--end-of-options", &range.from, &range.to, "--"])
            .args(request.orig_path.as_deref())
            .args([&request.path])
            .current_dir(repo)
            .output()
            .await?;
        let limits = if request.full {
            DiffLimits::UNLIMITED
        } else {
            DiffLimits::DEFAULT
        };
        parse_diff(&output.stdout, limits)
    }

    /// Both versions of the file a review diff shows; a side is `None` where
    /// the file does not exist.
    pub async fn review_sources(
        &self,
        repo: &Path,
        request: &ReviewDiffRequest,
    ) -> Result<DiffSources, GitError> {
        let old_path = request.orig_path.as_deref().unwrap_or(&request.path);
        Ok(DiffSources {
            old: self
                .blob(repo, &format!("{}:{old_path}", request.range.from))
                .await?,
            new: self
                .blob(repo, &format!("{}:{}", request.range.to, request.path))
                .await?,
        })
    }

    /// The full object id of the commit `rev` names, or `None`.
    async fn resolve_commit(&self, repo: &Path, rev: &str) -> Result<Option<String>, GitError> {
        let output = self
            .runner()
            .command(["rev-parse", "--verify", "--quiet", "--end-of-options"])
            .args([format!("{rev}^{{commit}}")])
            .success_codes(&[0, 1])
            .current_dir(repo)
            .output()
            .await?;
        Ok((output.exit_code == 0)
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned()))
    }

    /// The id of the empty tree in this repository's hash format.
    async fn empty_tree(&self, repo: &Path) -> Result<String, GitError> {
        let output = self
            .runner()
            .command(["hash-object", "-t", "tree", "--stdin"])
            .stdin(Vec::new())
            .current_dir(repo)
            .output()
            .await?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
}

fn missing(rev: &str) -> GitError {
    GitError::Parse(format!("{rev} does not name a commit"))
}

/// Parses `git diff --raw --numstat -z`: all raw records, then one numstat
/// record per file in the same order.
pub fn parse_raw_numstat(raw: &[u8]) -> Result<Vec<ReviewFile>, GitError> {
    let mut tokens = raw
        .split(|&byte| byte == 0)
        .map(|token| String::from_utf8_lossy(token).into_owned());
    let mut files = Vec::new();
    let mut counted = 0;
    let malformed = |what: &str| GitError::Parse(format!("unexpected diff record {what:?}"));

    while let Some(token) = tokens.next() {
        if token.is_empty() {
            continue;
        }
        if let Some(header) = token.strip_prefix(':') {
            // `:old_mode new_mode old_oid new_oid status`
            let status = header.rsplit(' ').next().ok_or_else(|| malformed(&token))?;
            let kind = match status.as_bytes().first() {
                Some(b'A') => ChangeKind::Added,
                Some(b'D') => ChangeKind::Deleted,
                Some(b'R') => ChangeKind::Renamed,
                Some(b'C') => ChangeKind::Copied,
                Some(b'T') => ChangeKind::TypeChanged,
                _ => ChangeKind::Modified,
            };
            let first = tokens.next().ok_or_else(|| malformed(&token))?;
            let (path, orig_path) = if matches!(kind, ChangeKind::Renamed | ChangeKind::Copied) {
                (tokens.next().ok_or_else(|| malformed(&token))?, Some(first))
            } else {
                (first, None)
            };
            files.push(ReviewFile {
                path,
                orig_path,
                kind,
                additions: None,
                deletions: None,
            });
            continue;
        }

        // `added\tdeleted\tpath`, or `added\tdeleted\t` followed by both
        // paths of a rename; `-` counts for binary files.
        let mut fields = token.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Err(malformed(&token));
        };
        if path.is_empty() {
            // Skip the two paths; the raw record has them already.
            tokens.next();
            tokens.next();
        }
        let file = files.get_mut(counted).ok_or_else(|| malformed(&token))?;
        file.additions = added.parse().ok();
        file.deletions = deleted.parse().ok();
        counted += 1;
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_and_numstat_records() {
        let raw = [
            ":100644 100644 aaa bbb M",
            "src/a b.rs",
            ":000000 100644 000 ccc A",
            "logo.png",
            ":100644 100644 ddd eee R087",
            "old.txt",
            "new.txt",
            "3\t1\tsrc/a b.rs",
            "-\t-\tlogo.png",
            "1\t1\t",
            "old.txt",
            "new.txt",
            "",
        ]
        .join("\0");
        let files = parse_raw_numstat(raw.as_bytes()).unwrap();
        assert_eq!(
            files,
            vec![
                ReviewFile {
                    path: "src/a b.rs".into(),
                    orig_path: None,
                    kind: ChangeKind::Modified,
                    additions: Some(3),
                    deletions: Some(1),
                },
                ReviewFile {
                    path: "logo.png".into(),
                    orig_path: None,
                    kind: ChangeKind::Added,
                    additions: None,
                    deletions: None,
                },
                ReviewFile {
                    path: "new.txt".into(),
                    orig_path: Some("old.txt".into()),
                    kind: ChangeKind::Renamed,
                    additions: Some(1),
                    deletions: Some(1),
                },
            ]
        );
    }

    #[test]
    fn parses_an_empty_diff() {
        assert_eq!(parse_raw_numstat(b"").unwrap(), vec![]);
    }
}
