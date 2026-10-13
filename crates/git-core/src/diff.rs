//! Structured diffs of single files, parsed from git's unified diff output.
//! Unified and side-by-side views are both rendered from this model.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::GitError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
    /// The line has no trailing newline in its file.
    pub no_newline: bool,
    /// Syntax tokens, filled in by the highlighter; empty if not highlighted.
    pub tokens: Vec<Token>,
}

/// A highlighted range of a line. Offsets count UTF-16 code units, so the
/// frontend can slice JavaScript strings with them directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Token {
    pub start: u32,
    pub end: u32,
    pub kind: TokenKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TokenKind {
    Comment,
    Keyword,
    String,
    Number,
    Function,
    Type,
    Tag,
    Attribute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiffHunk {
    /// The full `@@ … @@` line, including the function context git adds.
    pub header: String,
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileDiff {
    /// Extended header information, e.g. `new file mode 100644` or
    /// `rename from …`.
    pub notes: Vec<String>,
    pub binary: bool,
    pub hunks: Vec<DiffHunk>,
    pub additions: u32,
    pub deletions: u32,
    /// The diff exceeds the display limits; `hunks` is empty until it is
    /// requested in full.
    pub too_large: bool,
    /// Number of diff lines (context, added and removed).
    pub line_count: u32,
}

/// Diffs larger than this are only sent when explicitly requested.
#[derive(Debug, Clone, Copy)]
pub struct DiffLimits {
    pub max_lines: u32,
    pub max_bytes: usize,
}

impl DiffLimits {
    pub const DEFAULT: Self = Self {
        max_lines: 20_000,
        max_bytes: 4 * 1024 * 1024,
    };
    pub const UNLIMITED: Self = Self {
        max_lines: u32::MAX,
        max_bytes: usize::MAX,
    };
}

/// Parses `git diff` output for a single file.
pub fn parse_diff(raw: &[u8], limits: DiffLimits) -> Result<FileDiff, GitError> {
    let text = String::from_utf8_lossy(raw);
    let mut diff = FileDiff::default();
    let mut in_hunks = false;
    let (mut old_line, mut new_line) = (0, 0);

    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.starts_with("@@") {
            let (old_start, new_start) = parse_hunk_header(line)
                .ok_or_else(|| GitError::Parse(format!("bad hunk header {line:?}")))?;
            old_line = old_start;
            new_line = new_start;
            in_hunks = true;
            diff.hunks.push(DiffHunk {
                header: line.to_owned(),
                old_start,
                new_start,
                lines: Vec::new(),
            });
            continue;
        }
        if !in_hunks {
            parse_header_line(line, &mut diff);
            continue;
        }
        let Some(hunk) = diff.hunks.last_mut() else {
            continue;
        };
        let (kind, content) = match line.as_bytes().first() {
            Some(b' ') => (DiffLineKind::Context, &line[1..]),
            Some(b'+') => (DiffLineKind::Added, &line[1..]),
            Some(b'-') => (DiffLineKind::Removed, &line[1..]),
            Some(b'\\') => {
                if let Some(last) = hunk.lines.last_mut() {
                    last.no_newline = true;
                }
                continue;
            }
            // The empty string after the final newline, or the start of the
            // next file (not expected for single-file diffs).
            _ => continue,
        };
        let (old, new) = match kind {
            DiffLineKind::Context => (Some(old_line), Some(new_line)),
            DiffLineKind::Added => (None, Some(new_line)),
            DiffLineKind::Removed => (Some(old_line), None),
        };
        if old.is_some() {
            old_line += 1;
        }
        if new.is_some() {
            new_line += 1;
        }
        match kind {
            DiffLineKind::Added => diff.additions += 1,
            DiffLineKind::Removed => diff.deletions += 1,
            DiffLineKind::Context => {}
        }
        diff.line_count += 1;
        hunk.lines.push(DiffLine {
            kind,
            old_line: old,
            new_line: new,
            text: content.to_owned(),
            no_newline: false,
            tokens: Vec::new(),
        });
    }

    if diff.line_count > limits.max_lines || raw.len() > limits.max_bytes {
        diff.too_large = true;
        diff.hunks.clear();
    }
    Ok(diff)
}

fn parse_header_line(line: &str, diff: &mut FileDiff) {
    if line.starts_with("Binary files ") || line == "GIT binary patch" {
        diff.binary = true;
    } else if [
        "new file mode",
        "deleted file mode",
        "old mode",
        "new mode",
        "similarity index",
        "rename from",
        "rename to",
        "copy from",
        "copy to",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
    {
        diff.notes.push(line.to_owned());
    }
}

/// Parses `@@ -a[,b] +c[,d] @@ …` into the start lines `(a, c)`.
fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    let mut ranges = line.strip_prefix("@@ ")?.split(' ');
    let start = |range: &str| range.split(',').next()?.parse::<u32>().ok();
    let old = start(ranges.next()?.strip_prefix('-')?)?;
    let new = start(ranges.next()?.strip_prefix('+')?)?;
    Some((old, new))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,5 @@ fn main() {
 one
-two
+zwei
+drei
 three
 four
@@ -10,2 +11,2 @@
 ten
-eleven
\\ No newline at end of file
+elf
\\ No newline at end of file
";

    #[test]
    fn parses_hunks_with_line_numbers() {
        let diff = parse_diff(SAMPLE.as_bytes(), DiffLimits::DEFAULT).unwrap();
        assert_eq!(diff.hunks.len(), 2);
        assert_eq!((diff.additions, diff.deletions, diff.line_count), (3, 2, 9));

        let first = &diff.hunks[0];
        assert_eq!(first.header, "@@ -1,4 +1,5 @@ fn main() {");
        let numbers: Vec<_> = first
            .lines
            .iter()
            .map(|l| (l.kind, l.old_line, l.new_line, l.text.as_str()))
            .collect();
        assert_eq!(
            numbers,
            vec![
                (DiffLineKind::Context, Some(1), Some(1), "one"),
                (DiffLineKind::Removed, Some(2), None, "two"),
                (DiffLineKind::Added, None, Some(2), "zwei"),
                (DiffLineKind::Added, None, Some(3), "drei"),
                (DiffLineKind::Context, Some(3), Some(4), "three"),
                (DiffLineKind::Context, Some(4), Some(5), "four"),
            ]
        );

        let second = &diff.hunks[1];
        assert_eq!((second.old_start, second.new_start), (10, 11));
        assert!(second.lines[1].no_newline && second.lines[2].no_newline);
        assert!(!second.lines[0].no_newline);
    }

    #[test]
    fn detects_binary_files_and_notes() {
        let raw = "diff --git a/logo.png b/logo.png\nnew file mode 100644\nindex 0000000..1111111\nBinary files /dev/null and b/logo.png differ\n";
        let diff = parse_diff(raw.as_bytes(), DiffLimits::DEFAULT).unwrap();
        assert!(diff.binary);
        assert_eq!(diff.notes, vec!["new file mode 100644"]);
        assert_eq!(diff.hunks, []);
    }

    #[test]
    fn strips_carriage_returns_and_handles_single_line_ranges() {
        let raw = "@@ -1 +1 @@\r\n-a\r\n+b\r\n";
        let diff = parse_diff(raw.as_bytes(), DiffLimits::DEFAULT).unwrap();
        assert_eq!(diff.hunks[0].lines[0].text, "a");
        assert_eq!(diff.hunks[0].lines[1].new_line, Some(1));
    }

    #[test]
    fn drops_hunks_of_oversized_diffs_but_keeps_counts() {
        let limits = DiffLimits {
            max_lines: 5,
            max_bytes: usize::MAX,
        };
        let diff = parse_diff(SAMPLE.as_bytes(), limits).unwrap();
        assert!(diff.too_large);
        assert_eq!(diff.hunks, []);
        assert_eq!(diff.line_count, 9);
    }
}
