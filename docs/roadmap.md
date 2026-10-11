# Roadmap

Branchboard is a lean git GUI for macOS and Windows, built around one workflow: many
repositories, a fast status/diff overview, file-level staging and quick branch switching with
ahead/behind counts against a mainline branch (default `origin/development`). Commits stay in
the editor or terminal.

## Milestones

- **M0 – Foundation:** Cargo workspace, Tauri + Svelte shell, tooling (rustfmt, clippy,
  ESLint, Prettier, svelte-check, Vitest), CI for macOS and Windows, ADRs, git runner with
  discovery and version check, typed IPC.
- **M1 – Repositories:** add/remove (dialog, drag and drop, validation), groups, favorites,
  persistence, sidebar, quick switcher.
- **M2 – Status & diff:** status parser, changes view, unified diff, stage/unstage/discard
  (multi-select; untracked files go to the trash), file watcher, status cache.
- **M3 – Branches:** branch board with ahead/behind against `origin/<mainline>`, per-repo
  mainline and remote, switch with clear errors, manual and periodic fetch.
- **M4 – Polish:** side-by-side diff, command palette, open in VS Code, syntax highlighting
  (computed in Rust), performance hardening on Windows.

## Out of scope for now

Committing, hunk/line staging, push/pull, history graph, conflict resolution, code signing and
auto-update, scanning folders for repositories.

## Performance budgets

- Cold start to interactive < 500 ms; idle memory < 80 MB including the WebView; idle CPU ≈ 0 %.
- Status refresh < 100 ms for ~10k files; repository switch (from cache) < 50 ms.
- Diff display < 50 ms for typical files; 10k-line diffs scroll at 60 fps.
