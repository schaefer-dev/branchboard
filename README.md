# Branchboard

A lean Git GUI for macOS and Windows, built for working across multiple repositories.
Switch projects, review diffs, stage files, and see where your branches stand at a glance.

![Branchboard in dark mode, showing grouped repositories, staged and unstaged files, and a syntax-highlighted diff](assets/readme-hero.png)

Branchboard handles the overview and review loop. Keep committing in your editor or terminal.
Built with Tauri 2, Rust, Svelte 5, and TypeScript, using your installed Git CLI.

## Features

- **Your repositories, organized.** Add existing repositories through a folder picker or drag
  and drop, arrange them in groups, pin favorites, and jump between them with the quick switcher.
- **Review and stage.** Inspect syntax-highlighted unified diffs, stage or unstage individual
  files or a selection, and discard unstaged changes with confirmation. Untracked files are
  moved to the trash when discarded.
- **Know where branches stand.** View ahead/behind counts against a configurable comparison
  branch, inspect commits, and switch branches. Fetch manually or in the background.
- **Stay on the keyboard.** Use the command palette and shortcuts to navigate, fetch, or open
  a repository or changed file in VS Code.
- **Light and dark themes.** The interface follows your system appearance.

## Getting started

Branchboard is in active development. To try the current version, build it from source.

### Prerequisites

- Git 2.41 or newer, available on `PATH`.
- Node.js 24 or newer and pnpm; use the version pinned in `package.json`.
- Rust via [rustup](https://rustup.rs). The toolchain and components are pinned in
  `rust-toolchain.toml`.
- **macOS:** Xcode command line tools (`xcode-select --install`).
- **Windows:** MSVC C++ build tools, the Windows SDK, and the WebView2 runtime.
- **Optional:** VS Code for the editor integration.

### Run locally

```sh
git clone https://github.com/schaefer-dev/branchboard.git
cd branchboard
pnpm install --frozen-lockfile
pnpm app:dev
```

To create a release build and platform installers:

```sh
pnpm app:build
```

Build artifacts are written under `target/release/`; installers are under
`target/release/bundle/`. Code signing and automatic updates are not currently configured.

### First steps

1. Add an existing repository using the **+** button or drag its folder into the window.
2. Select a file in **Changes** to review its diff. Double-click or press `Space` to stage or
   unstage it; use the context menu for more actions.
3. Open **Branches** to compare and switch branches. In the branch options, use
   **Compare branches with…** to set your mainline, such as `origin/main`.

The default comparison branch is `origin/development`. Background fetching defaults to every
10 minutes for the active repository; change the interval or turn it off in the branch options.

### Keyboard shortcuts

Use `⌘` on macOS and `Ctrl` on Windows.

| Action            | macOS       | Windows        |
| ----------------- | ----------- | -------------- |
| Switch repository | `⌘ P`       | `Ctrl P`       |
| Command palette   | `⌘ K`       | `Ctrl K`       |
| Add repositories  | `⌘ O`       | `Ctrl O`       |
| Show changes      | `⌘ 1`       | `Ctrl 1`       |
| Show branches     | `⌘ 2`       | `Ctrl 2`       |
| Fetch             | `⌘ Shift F` | `Ctrl Shift F` |
| Open in VS Code   | `⌘ Shift E` | `Ctrl Shift E` |

## Current scope

Branchboard focuses on repository navigation, review, file-level staging, and branch switching.
It does not currently provide committing, push/pull, hunk- or line-level staging, side-by-side
diffs, a history graph, or conflict resolution.

## Development

| Task                               | Command                                                 |
| ---------------------------------- | ------------------------------------------------------- |
| Rust tests                         | `cargo test --workspace`                                |
| Rust lint                          | `cargo clippy --workspace --all-targets -- -D warnings` |
| Rust format check                  | `cargo fmt --all --check`                               |
| Regenerate TS bindings             | `pnpm bindings`                                         |
| Frontend lint and formatting check | `pnpm lint`                                             |
| Frontend type check                | `pnpm check`                                            |
| Frontend tests                     | `pnpm test`                                             |
| UI end-to-end tests                | `pnpm e2e`                                              |
| Capture UI screenshots             | `pnpm e2e screenshots`                                  |

For end-to-end tests, install the browser runtimes once with
`pnpm exec playwright install chromium webkit`. Screenshot tests write review artifacts to
`e2e-results/screenshots/`. CI checks and builds the app on macOS and Windows.

Set `BRANCHBOARD_WORKSPACE_FILE` to an absolute path to a separate JSON file when developing
or testing with a different repository list. This keeps that workspace separate from your
usual Branchboard configuration.

### Project layout

```text
crates/git-core/   Git domain types and CLI adapter (no Tauri dependency)
crates/workspace/  Repository registry, persistence, monitoring, editor integration
crates/highlight/  Syntax highlighting for diffs
src-tauri/        Tauri shell: commands, state, events
ui/src/           Svelte 5 frontend
  lib/api/        Typed command wrappers and generated bindings
  lib/state/      Application state
  lib/ui/         Design tokens and shared components
  features/       Repositories, changes, branches, diffs, and app shell
ui/e2e/           Playwright specs, fake backend, and recorded fixtures
```
