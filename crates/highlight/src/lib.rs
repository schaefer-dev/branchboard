//! Syntax highlighting for diffs, based on tree-sitter.
//!
//! The old and the new version of a file are highlighted as a whole, so
//! constructs that start outside a hunk (block comments, strings) are still
//! recognized, and the tokens are then attached to the diff lines by line
//! number. Captures are mapped to a small set of [`TokenKind`]s instead of
//! colors, so the UI colors them from its own design tokens.

mod languages;

use std::sync::OnceLock;

use git_core::{DiffLineKind, FileDiff, Token, TokenKind};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent};

use crate::languages::Language;

/// Files larger than this are not highlighted.
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;

/// Capture names the queries use, and the token kind each maps to. A capture
/// such as `function.method` matches its longest listed prefix (`function`).
/// Operators, punctuation, variables and properties stay plain on purpose.
const CAPTURES: &[(&str, TokenKind)] = &[
    ("comment", TokenKind::Comment),
    ("keyword", TokenKind::Keyword),
    ("import", TokenKind::Keyword),
    ("media", TokenKind::Keyword),
    ("keyframes", TokenKind::Keyword),
    ("supports", TokenKind::Keyword),
    ("charset", TokenKind::Keyword),
    ("text.title", TokenKind::Keyword),
    ("string", TokenKind::String),
    ("escape", TokenKind::String),
    ("text.literal", TokenKind::String),
    ("text.uri", TokenKind::String),
    ("number", TokenKind::Number),
    ("boolean", TokenKind::Number),
    ("constant", TokenKind::Number),
    ("function", TokenKind::Function),
    ("constructor", TokenKind::Type),
    ("type", TokenKind::Type),
    ("tag", TokenKind::Tag),
    ("attribute", TokenKind::Attribute),
];

/// Highlight configurations, compiled on first use of each language so
/// languages the user never looks at cost neither time nor memory.
pub struct Highlighter {
    languages: Vec<(Language, OnceLock<HighlightConfiguration>)>,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    pub fn new() -> Self {
        Self {
            languages: Language::ALL
                .iter()
                .map(|language| (*language, OnceLock::new()))
                .collect(),
        }
    }

    /// Whether files at `path` (repository-relative) can be highlighted.
    pub fn supports(path: &str) -> bool {
        Language::for_path(path).is_some()
    }

    /// Fills in the tokens of the diff lines from the highlighted `old` and
    /// `new` versions of the file. A line whose text does not match its
    /// source line (e.g. because of git filters) stays plain.
    pub fn highlight_diff(
        &self,
        path: &str,
        diff: &mut FileDiff,
        old: Option<&str>,
        new: Option<&str>,
    ) {
        if diff.binary || diff.too_large {
            return;
        }
        let Some(language) = Language::for_path(path) else {
            return;
        };
        let old = old.and_then(|source| self.highlight_source(language, source));
        let new = new.and_then(|source| self.highlight_source(language, source));

        for line in diff.hunks.iter_mut().flat_map(|hunk| &mut hunk.lines) {
            let (side, number) = match line.kind {
                DiffLineKind::Removed => (&old, line.old_line),
                DiffLineKind::Added | DiffLineKind::Context => (&new, line.new_line),
            };
            let Some(source_line) = side
                .as_ref()
                .zip(number)
                .and_then(|(side, number)| side.get((number as usize).checked_sub(1)?))
            else {
                continue;
            };
            if source_line.text == line.text {
                line.tokens.clone_from(&source_line.tokens);
            }
        }
    }

    /// Highlights a whole file and returns its lines with their tokens.
    fn highlight_source<'a>(
        &self,
        language: Language,
        source: &'a str,
    ) -> Option<Vec<SourceLine<'a>>> {
        if source.len() > MAX_SOURCE_BYTES {
            return None;
        }
        let config = self.config(language)?;
        let mut highlighter = tree_sitter_highlight::Highlighter::new();
        let events = highlighter
            .highlight(config, source.as_bytes(), None, None, |name| {
                Language::for_injection(name).and_then(|injected| self.config(injected))
            })
            .ok()?;

        let mut lines = split_lines(source);
        let mut stack: Vec<usize> = Vec::new();
        for event in events {
            match event.ok()? {
                HighlightEvent::HighlightStart(highlight) => stack.push(highlight.0),
                HighlightEvent::HighlightEnd => {
                    stack.pop();
                }
                HighlightEvent::Source { start, end } => {
                    if let Some(&capture) = stack.last() {
                        add_token(&mut lines, start, end, CAPTURES[capture].1);
                    }
                }
            }
        }
        Some(lines)
    }

    /// The compiled configuration of `language`.
    ///
    /// # Panics
    ///
    /// Panics if a bundled query does not compile, which the tests rule out.
    fn config(&self, language: Language) -> Option<&HighlightConfiguration> {
        let (_, cell) = self
            .languages
            .iter()
            .find(|(candidate, _)| *candidate == language)?;
        Some(cell.get_or_init(|| {
            let names: Vec<&str> = CAPTURES.iter().map(|(name, _)| *name).collect();
            let mut config = language
                .configuration()
                .unwrap_or_else(|error| panic!("{language:?} queries: {error}"));
            config.configure(&names);
            config
        }))
    }
}

/// One line of a source file: where it starts, its text without the line
/// break, and the tokens found in it.
struct SourceLine<'a> {
    start: usize,
    text: &'a str,
    tokens: Vec<Token>,
}

fn split_lines(source: &str) -> Vec<SourceLine<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    for line in source.split_inclusive('\n') {
        let text = line.strip_suffix('\n').unwrap_or(line);
        let text = text.strip_suffix('\r').unwrap_or(text);
        lines.push(SourceLine {
            start,
            text,
            tokens: Vec::new(),
        });
        start += line.len();
    }
    lines
}

/// Adds the byte range `start..end` of the source as tokens, split at line
/// breaks. Offsets become UTF-16 code units within each line.
fn add_token(lines: &mut [SourceLine<'_>], start: usize, end: usize, kind: TokenKind) {
    let first = lines
        .partition_point(|line| line.start <= start)
        .saturating_sub(1);
    for line in lines.iter_mut().skip(first) {
        if line.start >= end {
            break;
        }
        let from = start.max(line.start) - line.start;
        let to = (end - line.start).min(line.text.len());
        if from >= to || !line.text.is_char_boundary(from) || !line.text.is_char_boundary(to) {
            continue;
        }
        let token_start = utf16_len(&line.text[..from]);
        let token_end = token_start + utf16_len(&line.text[from..to]);
        match line.tokens.last_mut() {
            Some(last) if last.kind == kind && last.end == token_start => last.end = token_end,
            _ => line.tokens.push(Token {
                start: token_start,
                end: token_end,
                kind,
            }),
        }
    }
}

fn utf16_len(text: &str) -> u32 {
    u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests;
