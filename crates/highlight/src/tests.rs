use std::fmt::Write as _;

use git_core::{DiffLimits, DiffLine, parse_diff};

use super::*;

/// A diff that adds every line of `source`, highlighted as a new file.
fn new_file(path: &str, source: &str) -> FileDiff {
    let mut raw = format!("@@ -0,0 +1,{} @@\n", source.lines().count());
    for line in source.lines() {
        writeln!(raw, "+{line}").unwrap();
    }
    let mut diff = parse_diff(raw.as_bytes(), DiffLimits::DEFAULT).unwrap();
    Highlighter::new().highlight_diff(path, &mut diff, None, Some(source));
    diff
}

/// The highlighted pieces of a line as (text, kind).
fn pieces(line: &DiffLine) -> Vec<(String, TokenKind)> {
    let units: Vec<u16> = line.text.encode_utf16().collect();
    line.tokens
        .iter()
        .map(|t| {
            let text = String::from_utf16_lossy(&units[t.start as usize..t.end as usize]);
            (text, t.kind)
        })
        .collect()
}

fn all_pieces(diff: &FileDiff) -> Vec<(String, TokenKind)> {
    diff.hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .flat_map(pieces)
        .collect()
}

#[track_caller]
fn assert_has(diff: &FileDiff, expected: &[(&str, TokenKind)]) {
    let found = all_pieces(diff);
    for (text, kind) in expected {
        assert!(
            found.iter().any(|(t, k)| t == text && k == kind),
            "missing {text:?} as {kind:?} in {found:?}"
        );
    }
}

#[test]
fn every_language_compiles_and_highlights() {
    use TokenKind::{Attribute, Comment, Function, Keyword, Number, String, Tag, Type};

    /// File path, source, and pieces that must be highlighted.
    type Case = (
        &'static str,
        &'static str,
        &'static [(&'static str, TokenKind)],
    );
    let cases: &[Case] = &[
        (
            "app.py",
            "def greet(name):\n    return \"hi\"  # note\n",
            &[
                ("def", Keyword),
                ("greet", Function),
                ("\"hi\"", String),
                ("# note", Comment),
            ],
        ),
        (
            "A.java",
            "public class A {\n  int x = 1; // c\n}\n",
            &[("public", Keyword), ("1", Number), ("// c", Comment)],
        ),
        ("README.md", "# Title\n\nText\n", &[("Title", Keyword)]),
        (
            "run.sh",
            "echo \"hi\" # c\n",
            &[("\"hi\"", String), ("# c", Comment)],
        ),
        (
            "ci.yaml",
            "key: \"value\" # c\n",
            &[("\"value\"", String), ("# c", Comment)],
        ),
        (
            "Cargo.toml",
            "port = 8080 # c\n",
            &[("8080", Number), ("# c", Comment)],
        ),
        (
            "Dockerfile",
            "FROM rust:1\nRUN echo hi\n",
            &[("FROM", Keyword), ("RUN", Keyword)],
        ),
        (
            "app.ts",
            "const x: number = 42; // c\n",
            &[
                ("const", Keyword),
                ("number", Type),
                ("42", Number),
                ("// c", Comment),
            ],
        ),
        (
            "view.tsx",
            "const a = <div className=\"x\" />;\n",
            &[("div", Tag), ("className", Attribute)],
        ),
        (
            "app.js",
            "function f() { return 'a'; }\n",
            &[("function", Keyword), ("f", Function), ("'a'", String)],
        ),
        (
            "index.html",
            "<p class=\"a\">x</p>\n<script>let y = 1;</script>\n",
            &[("p", Tag), ("class", Attribute), ("let", Keyword)],
        ),
        (
            "site.css",
            "a { color: red; } /* c */\n",
            &[("/* c */", Comment)],
        ),
    ];
    for (path, source, expected) in cases {
        let diff = new_file(path, source);
        let found = all_pieces(&diff);
        for (text, kind) in *expected {
            assert!(
                found.iter().any(|(t, k)| t == text && k == kind),
                "{path}: missing {text:?} as {kind:?} in {found:?}"
            );
        }
    }
}

#[test]
fn knows_files_by_name_and_extension() {
    for path in [
        "Dockerfile",
        "docker/Dockerfile.dev",
        "x.mts",
        "a.YAML",
        "b.jsx",
        ".zshrc",
    ] {
        assert!(Highlighter::supports(path), "{path}");
    }
    for path in [
        "main.rs",
        "App.svelte",
        "data.json",
        "Makefile",
        "noextension",
    ] {
        assert!(!Highlighter::supports(path), "{path}");
    }
}

#[test]
fn uses_the_whole_file_for_constructs_outside_the_hunk() {
    // The comment opens two lines before the hunk; only whole-file
    // highlighting knows the changed line is inside it.
    let old = "/*\n  notes\n  old\n*/\nconst a = 1;\n";
    let new = "/*\n  notes\n  new\n*/\nconst a = 1;\n";
    let raw = "@@ -3,1 +3,1 @@\n-  old\n+  new\n";
    let mut diff = parse_diff(raw.as_bytes(), DiffLimits::DEFAULT).unwrap();
    Highlighter::new().highlight_diff("a.ts", &mut diff, Some(old), Some(new));

    let lines = &diff.hunks[0].lines;
    assert_eq!(
        pieces(&lines[0]),
        vec![("  old".into(), TokenKind::Comment)]
    );
    assert_eq!(
        pieces(&lines[1]),
        vec![("  new".into(), TokenKind::Comment)]
    );
}

#[test]
fn skips_lines_that_differ_from_the_source() {
    let raw = "@@ -0,0 +1,1 @@\n+const a = 1;\n";
    let mut diff = parse_diff(raw.as_bytes(), DiffLimits::DEFAULT).unwrap();
    Highlighter::new().highlight_diff("a.ts", &mut diff, None, Some("let b = 2;\n"));
    assert_eq!(diff.hunks[0].lines[0].tokens, []);
}

#[test]
fn counts_offsets_in_utf16_and_handles_crlf() {
    let diff = new_file("a.ts", "const 🦀 = \"ä\"; // ok\r\n");
    assert_has(
        &diff,
        &[("\"ä\"", TokenKind::String), ("// ok", TokenKind::Comment)],
    );
}

#[test]
fn leaves_unsupported_files_plain() {
    let diff = new_file("main.rs", "fn main() {}\n");
    assert_eq!(diff.hunks[0].lines[0].tokens, []);
}

/// Timing check, run with
/// `cargo test -p highlight --release -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark"]
fn highlights_large_files_quickly() {
    use std::time::Instant;

    let started = Instant::now();
    let highlighter = Highlighter::new();
    let loaded = started.elapsed();

    let mut old = String::new();
    let mut new = String::new();
    let mut raw = String::from("@@ -1,5000 +1,5000 @@\n");
    for n in 0..5000 {
        let before = format!("  const value{n} = compute(\"old\", {n}); // before");
        let after = format!("  const value{n} = compute(\"new\", {n} * 2); // after");
        writeln!(old, "{before}").unwrap();
        writeln!(new, "{after}").unwrap();
        writeln!(raw, "-{before}").unwrap();
        writeln!(raw, "+{after}").unwrap();
    }
    let limits = DiffLimits::UNLIMITED;
    let mut diff = parse_diff(raw.as_bytes(), limits).unwrap();
    let started = Instant::now();
    highlighter.highlight_diff("big.ts", &mut diff, Some(&old), Some(&new));
    println!(
        "compile queries {loaded:?}, highlight 2 × 5000 lines {:?}",
        started.elapsed()
    );
    assert_ne!(diff.hunks[0].lines[0].tokens, []);
}
