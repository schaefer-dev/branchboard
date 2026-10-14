//! Local branches with ahead/behind counts against a mainline and against
//! their upstream, branch switching and fetching.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use ts_rs::TS;

use crate::{Git, GitError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AheadBehind {
    pub ahead: u32,
    pub behind: u32,
}

/// How a branch relates to its upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum UpstreamState {
    /// No upstream configured: the branch exists only locally.
    None,
    /// The upstream is configured but no longer exists on the remote.
    Gone {
        name: String,
    },
    Tracking {
        name: String,
        counts: AheadBehind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Branch {
    pub name: String,
    /// `HEAD` points at this branch.
    pub current: bool,
    pub upstream: UpstreamState,
    /// Counts against the mainline; `None` if the mainline does not exist.
    pub mainline: Option<AheadBehind>,
    /// Committer date of the tip, in seconds since the Unix epoch.
    #[ts(type = "number")]
    pub committed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BranchList {
    pub branches: Vec<Branch>,
    /// The ref branches are compared with, e.g. `origin/development`.
    pub mainline: String,
    pub mainline_exists: bool,
    /// When the repository was last fetched (from any tool), in seconds
    /// since the Unix epoch.
    #[ts(type = "number | null")]
    pub last_fetched: Option<i64>,
}

const FETCH_TIMEOUT: Duration = Duration::from_secs(120);

impl Git {
    /// Lists local branches. `mainline` is any committish, typically a
    /// remote-tracking branch such as `origin/development`.
    pub async fn branches(&self, repo: &Path, mainline: &str) -> Result<BranchList, GitError> {
        let mainline_exists = self.ref_exists(repo, mainline).await?;
        let mut format = String::from(
            "%(refname:short)%00%(HEAD)%00%(upstream:short)%00%(upstream:track,nobracket)%00%(committerdate:unix)",
        );
        if mainline_exists {
            format.push_str("%00%(ahead-behind:");
            format.push_str(mainline);
            format.push(')');
        }
        let output = self
            .runner()
            .command(["for-each-ref", "refs/heads"])
            .args([format!("--format={format}")])
            .untranslated()
            .current_dir(repo)
            .output()
            .await?;
        let branches = parse_branches(&String::from_utf8_lossy(&output.stdout))?;
        Ok(BranchList {
            branches,
            mainline: mainline.to_owned(),
            mainline_exists,
            last_fetched: self.last_fetched(repo).await?,
        })
    }

    /// Switches to a local branch. Fails (leaving everything untouched) when
    /// local changes would be overwritten.
    pub async fn switch(&self, repo: &Path, branch: &str) -> Result<(), GitError> {
        self.runner()
            .command(["switch", "--quiet", "--no-guess", "--"])
            .args([branch])
            .current_dir(repo)
            .output()
            .await?;
        Ok(())
    }

    /// Fetches all remotes and prunes deleted remote branches.
    pub async fn fetch(&self, repo: &Path) -> Result<(), GitError> {
        self.runner()
            .command(["fetch", "--all", "--prune", "--quiet"])
            .timeout(FETCH_TIMEOUT)
            .current_dir(repo)
            .output()
            .await?;
        Ok(())
    }

    /// Modification time of `FETCH_HEAD`, written by every fetch.
    pub async fn last_fetched(&self, repo: &Path) -> Result<Option<i64>, GitError> {
        let output = self
            .runner()
            .command([
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                "FETCH_HEAD",
            ])
            .current_dir(repo)
            .output()
            .await?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let modified = tokio::fs::metadata(path)
            .await
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        Ok(modified.map(unix_seconds))
    }

    /// Whether `name` resolves to a commit in `repo`.
    pub async fn ref_exists(&self, repo: &Path, name: &str) -> Result<bool, GitError> {
        let output = self
            .runner()
            .command(["rev-parse", "--verify", "--quiet", "--end-of-options"])
            .args([format!("{name}^{{commit}}")])
            .success_codes(&[0, 1, 128])
            .current_dir(repo)
            .output()
            .await?;
        Ok(output.exit_code == 0)
    }
}

fn unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |duration| {
        i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
    })
}

/// Parses `for-each-ref` output with NUL-separated fields, one branch per line.
pub fn parse_branches(output: &str) -> Result<Vec<Branch>, GitError> {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split('\0').collect();
            let malformed = || GitError::Parse(format!("unexpected branch line {line:?}"));
            let [name, head, upstream, track, date, rest @ ..] = fields.as_slice() else {
                return Err(malformed());
            };
            let mainline = match rest {
                [] => None,
                [counts] => Some(parse_ahead_behind(counts).ok_or_else(malformed)?),
                _ => return Err(malformed()),
            };
            Ok(Branch {
                name: (*name).to_owned(),
                current: *head == "*",
                upstream: parse_upstream(upstream, track).ok_or_else(malformed)?,
                mainline,
                committed_at: date.parse().map_err(|_| malformed())?,
            })
        })
        .collect()
}

/// `%(ahead-behind:…)` prints `<ahead> <behind>`.
fn parse_ahead_behind(field: &str) -> Option<AheadBehind> {
    let (ahead, behind) = field.split_once(' ')?;
    Some(AheadBehind {
        ahead: ahead.parse().ok()?,
        behind: behind.parse().ok()?,
    })
}

/// `%(upstream:track,nobracket)` prints `ahead N`, `behind M`,
/// `ahead N, behind M`, `gone` or nothing (in the C locale).
fn parse_upstream(upstream: &str, track: &str) -> Option<UpstreamState> {
    if upstream.is_empty() {
        return Some(UpstreamState::None);
    }
    let name = upstream.to_owned();
    if track == "gone" {
        return Some(UpstreamState::Gone { name });
    }
    let mut counts = AheadBehind {
        ahead: 0,
        behind: 0,
    };
    for part in track.split(", ").filter(|part| !part.is_empty()) {
        match part.split_once(' ')? {
            ("ahead", n) => counts.ahead = n.parse().ok()?,
            ("behind", n) => counts.behind = n.parse().ok()?,
            _ => return None,
        }
    }
    Some(UpstreamState::Tracking { name, counts })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(fields: &[&str]) -> String {
        fields.join("\0") + "\n"
    }

    #[test]
    fn parses_branches_with_upstream_and_mainline() {
        let output = [
            line(&[
                "development",
                "*",
                "origin/development",
                "",
                "1700000300",
                "0 0",
            ]),
            line(&[
                "feature/x",
                " ",
                "origin/feature/x",
                "ahead 2, behind 1",
                "1700000200",
                "3 1",
            ]),
            line(&["old", " ", "origin/old", "gone", "1700000100", "0 7"]),
            line(&["spike", " ", "", "", "1700000000", "1 0"]),
        ]
        .concat();
        let branches = parse_branches(&output).unwrap();

        assert_eq!(branches.len(), 4);
        assert!(branches[0].current && !branches[1].current);
        assert_eq!(
            branches[1].upstream,
            UpstreamState::Tracking {
                name: "origin/feature/x".into(),
                counts: AheadBehind {
                    ahead: 2,
                    behind: 1
                },
            }
        );
        assert_eq!(
            branches[1].mainline,
            Some(AheadBehind {
                ahead: 3,
                behind: 1
            })
        );
        assert_eq!(
            branches[2].upstream,
            UpstreamState::Gone {
                name: "origin/old".into()
            }
        );
        assert_eq!(branches[3].upstream, UpstreamState::None);
        assert_eq!(branches[3].committed_at, 1_700_000_000);
    }

    #[test]
    fn mainline_counts_are_optional() {
        let branches = parse_branches(&line(&["main", "*", "", "", "1"])).unwrap();
        assert_eq!(branches[0].mainline, None);
    }

    #[test]
    fn parses_partial_track_strings() {
        assert_eq!(
            parse_upstream("o/x", "behind 4"),
            Some(UpstreamState::Tracking {
                name: "o/x".into(),
                counts: AheadBehind {
                    ahead: 0,
                    behind: 4
                },
            })
        );
        assert_eq!(parse_upstream("o/x", "voraus 1"), None);
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(parse_branches("main\0*\n").is_err());
        assert!(parse_branches(&line(&["main", "*", "", "", "x"])).is_err());
    }
}
