//! Working tree status, parsed from `git status --porcelain=v2 --branch -z`.
//!
//! The status is presented as two lists, like the index itself: changes that
//! are staged (index vs. HEAD) and changes that are not (working tree vs.
//! index, plus untracked and conflicted files). A file with staged and
//! further unstaged edits appears in both.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::GitError;
use crate::summary::Head;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileChange {
    /// Path relative to the repository root, with `/` separators.
    pub path: String,
    /// Previous path of a rename or copy.
    pub orig_path: Option<String>,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StatusSnapshot {
    pub head: Head,
    /// The commit `HEAD` points to; `None` on a branch without commits.
    pub head_oid: Option<String>,
    pub staged: Vec<FileChange>,
    pub unstaged: Vec<FileChange>,
}

impl StatusSnapshot {
    pub fn is_dirty(&self) -> bool {
        !self.staged.is_empty() || !self.unstaged.is_empty()
    }

    pub fn find_staged(&self, path: &str) -> Option<&FileChange> {
        self.staged.iter().find(|change| change.path == path)
    }

    pub fn find_unstaged(&self, path: &str) -> Option<&FileChange> {
        self.unstaged.iter().find(|change| change.path == path)
    }
}

/// Parses the output of `git status --porcelain=v2 --branch -z`.
pub fn parse_status(raw: &[u8]) -> Result<StatusSnapshot, GitError> {
    let mut records = raw
        .split(|&byte| byte == 0)
        .filter(|record| !record.is_empty());
    let mut oid = None;
    let mut head = None;
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();

    while let Some(record) = records.next() {
        let line = String::from_utf8_lossy(record);
        let malformed = || GitError::Parse(format!("unexpected status line {line:?}"));

        if let Some(header) = line.strip_prefix("# ") {
            if let Some(value) = header.strip_prefix("branch.oid ") {
                oid = Some(value.to_owned());
            } else if let Some(value) = header.strip_prefix("branch.head ") {
                head = Some(value.to_owned());
            }
            continue;
        }

        match line.as_bytes().first() {
            // `1 XY sub mH mI mW hH hI path`
            Some(b'1') => {
                let fields: Vec<&str> = line.splitn(9, ' ').collect();
                let [_, xy, .., path] = fields.as_slice() else {
                    return Err(malformed());
                };
                push_ordinary(xy, path, None, &mut staged, &mut unstaged).ok_or_else(malformed)?;
            }
            // `2 XY sub mH mI mW hH hI Xscore path`, then the original path
            Some(b'2') => {
                let fields: Vec<&str> = line.splitn(10, ' ').collect();
                let [_, xy, .., path] = fields.as_slice() else {
                    return Err(malformed());
                };
                let orig = records
                    .next()
                    .map(|orig| String::from_utf8_lossy(orig).into_owned())
                    .ok_or_else(malformed)?;
                push_ordinary(xy, path, Some(&orig), &mut staged, &mut unstaged)
                    .ok_or_else(malformed)?;
            }
            // `u XY sub m1 m2 m3 mW h1 h2 h3 path`
            Some(b'u') => {
                let path = line.splitn(11, ' ').nth(10).ok_or_else(malformed)?;
                unstaged.push(change(path, None, ChangeKind::Conflicted));
            }
            Some(b'?') => {
                let path = line.get(2..).ok_or_else(malformed)?;
                unstaged.push(change(path, None, ChangeKind::Untracked));
            }
            // Ignored files are only listed with `--ignored`.
            Some(b'!') => {}
            _ => return Err(malformed()),
        }
    }

    let (Some(oid), Some(head)) = (oid, head) else {
        return Err(GitError::Parse("status output lacks branch headers".into()));
    };
    Ok(StatusSnapshot {
        head: Head::from_status_headers(&oid, &head),
        head_oid: (oid != "(initial)").then_some(oid),
        staged,
        unstaged,
    })
}

fn push_ordinary(
    xy: &str,
    path: &str,
    orig: Option<&str>,
    staged: &mut Vec<FileChange>,
    unstaged: &mut Vec<FileChange>,
) -> Option<()> {
    let mut codes = xy.chars();
    let (index, worktree) = (codes.next()?, codes.next()?);
    if let Some(kind) = kind_of(index) {
        let orig = matches!(kind, ChangeKind::Renamed | ChangeKind::Copied)
            .then(|| orig.map(str::to_owned))
            .flatten();
        staged.push(change(path, orig, kind));
    }
    if let Some(kind) = kind_of(worktree) {
        unstaged.push(change(path, None, kind));
    }
    Some(())
}

fn kind_of(code: char) -> Option<ChangeKind> {
    Some(match code {
        'A' => ChangeKind::Added,
        'M' => ChangeKind::Modified,
        'D' => ChangeKind::Deleted,
        'R' => ChangeKind::Renamed,
        'C' => ChangeKind::Copied,
        'T' => ChangeKind::TypeChanged,
        _ => return None,
    })
}

fn change(path: &str, orig_path: Option<String>, kind: ChangeKind) -> FileChange {
    FileChange {
        path: path.to_owned(),
        orig_path,
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(records: &[&str]) -> StatusSnapshot {
        let mut raw = records.join("\0");
        raw.push('\0');
        parse_status(raw.as_bytes()).unwrap()
    }

    const HEADERS: [&str; 2] = ["# branch.oid 0123456789abcdef", "# branch.head main"];

    fn with_headers<'a>(entries: &[&'a str]) -> Vec<&'a str> {
        HEADERS
            .iter()
            .copied()
            .chain(entries.iter().copied())
            .collect()
    }

    #[test]
    fn splits_staged_and_unstaged_changes() {
        let status = parse(&with_headers(&[
            "1 M. N... 100644 100644 100644 aaa bbb staged only.txt",
            "1 .M N... 100644 100644 100644 aaa aaa unstaged.txt",
            "1 MM N... 100644 100644 100644 aaa bbb both.txt",
            "1 A. N... 000000 100644 100644 000 bbb new.txt",
            "1 .D N... 100644 100644 000000 aaa aaa gone.txt",
            "? untracked dir/ä.txt",
        ]));
        let paths = |changes: &[FileChange]| -> Vec<(String, ChangeKind)> {
            changes.iter().map(|c| (c.path.clone(), c.kind)).collect()
        };
        assert_eq!(
            paths(&status.staged),
            vec![
                ("staged only.txt".into(), ChangeKind::Modified),
                ("both.txt".into(), ChangeKind::Modified),
                ("new.txt".into(), ChangeKind::Added),
            ]
        );
        assert_eq!(
            paths(&status.unstaged),
            vec![
                ("unstaged.txt".into(), ChangeKind::Modified),
                ("both.txt".into(), ChangeKind::Modified),
                ("gone.txt".into(), ChangeKind::Deleted),
                ("untracked dir/ä.txt".into(), ChangeKind::Untracked),
            ]
        );
        assert!(status.is_dirty());
    }

    #[test]
    fn keeps_the_original_path_of_renames() {
        let status = parse(&with_headers(&[
            "2 RM N... 100644 100644 100644 aaa bbb R87 new name.txt",
            "old name.txt",
            "? after.txt",
        ]));
        assert_eq!(
            status.staged,
            vec![FileChange {
                path: "new name.txt".into(),
                orig_path: Some("old name.txt".into()),
                kind: ChangeKind::Renamed,
            }]
        );
        assert_eq!(status.unstaged.len(), 2);
        assert_eq!(status.unstaged[0].orig_path, None);
        assert_eq!(status.unstaged[1].path, "after.txt");
    }

    #[test]
    fn lists_conflicts_as_unstaged() {
        let status = parse(&with_headers(&[
            "u UU N... 100644 100644 100644 100644 aaa bbb ccc src/main rs.rs",
        ]));
        assert_eq!(status.staged, vec![]);
        assert_eq!(
            status.unstaged,
            vec![change("src/main rs.rs", None, ChangeKind::Conflicted)]
        );
    }

    #[test]
    fn reports_the_head_commit() {
        let status = parse(&HEADERS);
        assert_eq!(status.head_oid.as_deref(), Some("0123456789abcdef"));
        let unborn = parse(&["# branch.oid (initial)", "# branch.head main"]);
        assert_eq!(unborn.head_oid, None);
    }

    #[test]
    fn rejects_unknown_records() {
        let mut raw = HEADERS.join("\0");
        raw.push_str("\0x what\0");
        assert!(parse_status(raw.as_bytes()).is_err());
    }
}
