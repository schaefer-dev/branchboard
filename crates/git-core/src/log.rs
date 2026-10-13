//! Commit lists between two refs.

use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use crate::{Git, GitError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitSummary {
    pub oid: String,
    pub short_oid: String,
    pub author: String,
    /// Author date in seconds since the Unix epoch.
    #[ts(type = "number")]
    pub authored_at: i64,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitList {
    /// Newest first, at most the requested number.
    pub commits: Vec<CommitSummary>,
    /// More commits exist than were listed.
    pub truncated: bool,
}

const FIELD: char = '\u{1f}';
const RECORD: char = '\u{1e}';

impl Git {
    /// Commits reachable from `tip` but not from `base` (`base..tip`), or all
    /// commits of `tip` when `base` is `None`. Newest first.
    pub async fn log(
        &self,
        repo: &Path,
        base: Option<&str>,
        tip: &str,
        limit: usize,
    ) -> Result<CommitList, GitError> {
        let range = match base {
            Some(base) => format!("{base}..{tip}"),
            None => tip.to_owned(),
        };
        let output = self
            .runner()
            .command(["log", "--no-color"])
            .args([
                format!("--max-count={}", limit + 1),
                format!("--format=%H{FIELD}%h{FIELD}%an{FIELD}%at{FIELD}%s{RECORD}"),
                "--end-of-options".to_owned(),
                range,
                "--".to_owned(),
            ])
            .current_dir(repo)
            .output()
            .await?;
        let mut commits = parse_log(&String::from_utf8_lossy(&output.stdout))?;
        let truncated = commits.len() > limit;
        commits.truncate(limit);
        Ok(CommitList { commits, truncated })
    }
}

pub fn parse_log(output: &str) -> Result<Vec<CommitSummary>, GitError> {
    output
        .split(RECORD)
        .map(|record| record.trim_start_matches('\n'))
        .filter(|record| !record.is_empty())
        .map(|record| {
            let malformed = || GitError::Parse(format!("unexpected log record {record:?}"));
            let fields: Vec<&str> = record.split(FIELD).collect();
            let [oid, short_oid, author, date, subject] = fields.as_slice() else {
                return Err(malformed());
            };
            Ok(CommitSummary {
                oid: (*oid).to_owned(),
                short_oid: (*short_oid).to_owned(),
                author: (*author).to_owned(),
                authored_at: date.parse().map_err(|_| malformed())?,
                subject: (*subject).to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_records() {
        let output = format!(
            "aaaa{FIELD}aa{FIELD}Ada{FIELD}100{FIELD}Fix: a|b{RECORD}\nbbbb{FIELD}bb{FIELD}Bo{FIELD}50{FIELD}{RECORD}\n"
        );
        let commits = parse_log(&output).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].subject, "Fix: a|b");
        assert_eq!(commits[1].subject, "");
        assert_eq!(commits[1].authored_at, 50);
    }

    #[test]
    fn rejects_malformed_records() {
        assert!(parse_log(&format!("x{FIELD}y{RECORD}")).is_err());
    }
}
