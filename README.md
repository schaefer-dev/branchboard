# Branchboard

A lean git GUI for macOS and Windows: switch between repositories quickly, review and stage
changes per file, and switch branches with ahead/behind counts against your mainline.

See [docs/roadmap.md](docs/roadmap.md) for scope and milestones and [docs/adr](docs/adr) for
architecture decisions.

## Prerequisites

- Rust via [rustup](https://rustup.rs) (the toolchain in `rust-toolchain.toml` installs itself)
- Node.js 24+ and pnpm (`npm install -g pnpm`)
- git 2.41 or newer
- Windows: WebView2 (preinstalled on Windows 10/11) and the MSVC build tools
- macOS: Xcode command line tools

## Development

```sh
pnpm install
pnpm app:dev        # run the app with hot reload
pnpm app:build      # release build and installers
```

| Task                        | Command                                                |
| --------------------------- | ------------------------------------------------------ |
| Rust tests                  | `cargo test --workspace`                               |
| Rust lint / format          | `cargo clippy --workspace --all-targets` / `cargo fmt` |
| Regenerate TS bindings      | `pnpm bindings`                                        |
| Frontend lint / format      | `pnpm lint` / `pnpm format`                            |
| Frontend type check / tests | `pnpm check` / `pnpm test`                             |

Set `BRANCHBOARD_WORKSPACE_FILE=/path/to/workspace.json` to run against a separate repository
list instead of your own.

## Layout

```
crates/git-core/   git domain types and CLI adapter (no Tauri dependency)
crates/workspace/  repository registry, persistence, background summaries
src-tauri/         Tauri shell: commands, state, wiring
ui/src/            Svelte 5 frontend
  lib/api/         typed command wrappers and generated bindings
  lib/state/       stores (runes classes)
  lib/ui/          design tokens and base components
  features/        feature components (repos, diff, shell, ...)
docs/              roadmap and ADRs
```
