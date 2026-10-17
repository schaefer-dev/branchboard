//! Pinned, topo-ordered history. Search is literal OR across the full message,
//! author name and full hash (hash substrings, including abbreviations).
use std::{path::Path, time::Duration};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{CommitSummary, Git, GitError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryCursor {
    pub tip: String,
    pub query: String,
    pub reference: String,
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryRequest {
    /// HEAD or a fully qualified refs/heads/ name. Never checked out.
    pub reference: String,
    pub query: String,
    pub tip: Option<String>,
    pub cursor: Option<HistoryCursor>,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryPage {
    pub commits: Vec<CommitSummary>,
    pub tip: Option<String>,
    pub cursor: Option<HistoryCursor>,
}

fn invalid() -> GitError {
    GitError::Parse("invalid history reference, query or cursor".into())
}
fn oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

impl HistoryRequest {
    fn offset(&self, tip: Option<&String>, query: &str) -> Result<u32, GitError> {
        match &self.cursor {
            Some(cursor)
                if Some(&cursor.tip) == tip
                    && cursor.query == query
                    && cursor.reference == self.reference
                    && self.tip.is_some()
                    && cursor.offset > 0 =>
            {
                Ok(cursor.offset)
            }
            Some(_) => Err(invalid()),
            None => Ok(0),
        }
    }
}

impl Git {
    pub async fn history_tip(
        &self,
        repo: &Path,
        reference: &str,
    ) -> Result<Option<String>, GitError> {
        if reference != "HEAD" {
            if !reference.starts_with("refs/heads/") {
                return Err(invalid());
            }
            self.runner()
                .command(["check-ref-format", reference])
                .output()
                .await?;
        }
        let out = self
            .runner()
            .command([
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &format!("{reference}^{{commit}}"),
            ])
            .current_dir(repo)
            .success_codes(&[0, 1])
            .output()
            .await?;
        if out.exit_code == 1 {
            // Only an unborn HEAD is empty. A deleted branch is an error.
            if reference == "HEAD" {
                let symbolic = self
                    .runner()
                    .command(["symbolic-ref", "--quiet", "HEAD"])
                    .current_dir(repo)
                    .success_codes(&[0, 1])
                    .output()
                    .await?;
                if symbolic.exit_code == 0 {
                    return Ok(None);
                }
            }
            return Err(GitError::Parse(
                "The selected history ref no longer exists.".into(),
            ));
        }
        Ok(Some(String::from_utf8_lossy(&out.stdout).trim().to_owned()))
    }

    pub async fn history(
        &self,
        repo: &Path,
        request: &HistoryRequest,
    ) -> Result<HistoryPage, GitError> {
        if request.limit == 0
            || request.limit > 100
            || request.query.len() > 4096
            || request.query.contains('\0')
        {
            return Err(invalid());
        }
        // Validate the selector even when reading a pinned object, but do not
        // resolve it: deleted/moved refs must not break an existing snapshot.
        if request.reference != "HEAD" {
            if !request.reference.starts_with("refs/heads/") {
                return Err(invalid());
            }
            self.runner()
                .command(["check-ref-format", &request.reference])
                .output()
                .await?;
        }
        let tip = match &request.tip {
            Some(tip) if oid(tip) => Some(tip.clone()),
            Some(_) => return Err(invalid()),
            None => self.history_tip(repo, &request.reference).await?,
        };
        let query = request.query.to_lowercase();
        let offset = request.offset(tip.as_ref(), &query)?;
        let Some(tip) = tip else {
            return Ok(HistoryPage {
                commits: vec![],
                tip: None,
                cursor: None,
            });
        };
        let mut commits = Vec::new();
        let mut fields = Vec::new();
        let mut scanned = offset;
        let mut next = None;
        self.runner()
            .command([
                "log",
                "--topo-order",
                "--no-color",
                "--no-show-signature",
                "--no-notes",
                "-z",
                "--format=%H%x00%h%x00%an%x00%at%x00%s%x00%B",
            ])
            .args([
                format!("--skip={offset}"),
                "--end-of-options".into(),
                tip.clone(),
                "--".into(),
            ])
            .current_dir(repo)
            .timeout(Duration::from_secs(300))
            .visit_nul(|field| {
                fields.push(String::from_utf8_lossy(field).into_owned());
                if fields.len() != 6 {
                    return Ok(true);
                }
                let matches = query.is_empty()
                    || [0, 2, 5]
                        .iter()
                        .any(|&i| fields[i].to_lowercase().contains(&query));
                if matches && commits.len() == request.limit as usize {
                    next = Some(HistoryCursor {
                        tip: tip.clone(),
                        query: query.clone(),
                        reference: request.reference.clone(),
                        offset: scanned,
                    });
                    return Ok(false);
                }
                scanned = scanned.checked_add(1).ok_or_else(invalid)?;
                if matches {
                    commits.push(CommitSummary {
                        oid: fields[0].clone(),
                        short_oid: fields[1].clone(),
                        author: fields[2].clone(),
                        authored_at: fields[3].parse().map_err(|_| invalid())?,
                        subject: fields[4].clone(),
                    });
                }
                fields.clear();
                Ok(true)
            })
            .await?;
        if next.is_none() && !fields.is_empty() {
            return Err(GitError::Parse("incomplete history record".into()));
        }
        Ok(HistoryPage {
            commits,
            tip: Some(tip),
            cursor: next,
        })
    }
}
