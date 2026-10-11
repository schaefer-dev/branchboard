//! A cheap per-repository overview: current HEAD and whether the working
//! tree has changes. Parsed from `git status --porcelain=v2 --branch -z`.

use serde::Serialize;
use ts_rs::TS;

use crate::GitError;

/// What `HEAD` points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Head {
    /// On a branch with at least one commit.
    Branch { name: String },
    /// On a branch that has no commits yet.
    Unborn { name: String },
    /// Detached at a commit (abbreviated object id).
    Detached { oid: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RepoSummary {
    pub head: Head,
    /// Staged, unstaged, untracked or conflicted changes exist.
    pub dirty: bool,
}

const SHORT_OID_LEN: usize = 7;

/// Parses the output of `git status --porcelain=v2 --branch -z`.
pub fn parse_summary(raw: &[u8]) -> Result<RepoSummary, GitError> {
    let mut oid = None;
    let mut head = None;
    let mut dirty = false;

    for record in raw.split(|&byte| byte == 0) {
        if let Some(header) = record.strip_prefix(b"# ") {
            let header = String::from_utf8_lossy(header);
            if let Some(value) = header.strip_prefix("branch.oid ") {
                oid = Some(value.to_owned());
            } else if let Some(value) = header.strip_prefix("branch.head ") {
                head = Some(value.to_owned());
            }
        } else if !record.is_empty() {
            // Any entry (changed, renamed, unmerged or untracked) makes the
            // tree dirty. Rename entries are followed by a second record with
            // the original path, which does not change the result.
            dirty = true;
        }
    }

    let (Some(oid), Some(head)) = (oid, head) else {
        return Err(GitError::Parse("status output lacks branch headers".into()));
    };
    let head = match (oid.as_str(), head.as_str()) {
        ("(initial)", name) => Head::Unborn {
            name: name.to_owned(),
        },
        (oid, "(detached)") => Head::Detached {
            oid: oid.chars().take(SHORT_OID_LEN).collect(),
        },
        (_, name) => Head::Branch {
            name: name.to_owned(),
        },
    };
    Ok(RepoSummary { head, dirty })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(records: &[&str]) -> RepoSummary {
        let mut raw = records.join("\0");
        raw.push('\0');
        parse_summary(raw.as_bytes()).unwrap()
    }

    #[test]
    fn clean_branch() {
        let summary = parse(&["# branch.oid 0123456789abcdef", "# branch.head main"]);
        assert_eq!(
            summary,
            RepoSummary {
                head: Head::Branch {
                    name: "main".into()
                },
                dirty: false
            }
        );
    }

    #[test]
    fn dirty_with_untracked_and_rename() {
        let summary = parse(&[
            "# branch.oid 0123456789abcdef",
            "# branch.head feature/äö",
            "# branch.upstream origin/feature/äö",
            "# branch.ab +1 -0",
            "2 R. N... 100644 100644 100644 abc abc R100 new name.txt",
            "old name.txt",
            "? untracked.txt",
        ]);
        assert_eq!(
            summary.head,
            Head::Branch {
                name: "feature/äö".into()
            }
        );
        assert!(summary.dirty);
    }

    #[test]
    fn unborn_and_detached() {
        let unborn = parse(&["# branch.oid (initial)", "# branch.head development"]);
        assert_eq!(
            unborn.head,
            Head::Unborn {
                name: "development".into()
            }
        );

        let detached = parse(&["# branch.oid 0123456789abcdef", "# branch.head (detached)"]);
        assert_eq!(
            detached.head,
            Head::Detached {
                oid: "0123456".into()
            }
        );
    }

    #[test]
    fn missing_headers_is_an_error() {
        assert!(parse_summary(b"? file\0").is_err());
    }
}
