# Branchboard – notes for Claude

Lean git GUI (Tauri 2 + Rust + Svelte 5/TS) for macOS and Windows. Scope and milestones:
`docs/roadmap.md`. Architecture decisions: `docs/adr/`.

## Working agreements

- **Never commit.** The user does all commits. Suggest a commit at a natural checkpoint and wait.
- Ask the user via the AskUserQuestion tool whenever input is needed, and wait for the answer.
- Record significant architecture decisions as a new ADR (`docs/adr/template.md`).

## Architecture rules

- `crates/git-core` must not depend on Tauri. All git access goes through `GitRunner`;
  parse only machine-readable output (`--porcelain=v2 -z`, `--format`), never error text.
- `src-tauri` stays thin: commands translate between IPC types and core crates.
- Every IPC type derives `ts_rs::TS` + `#[ts(export)]`; after changing one run
  `pnpm bindings`. Every command gets a typed wrapper in `ui/src/lib/api/commands.ts`.
- Commands return `Result<T, AppError>`; the frontend sees `CommandError`.
- Frontend: stores in `ui/src/lib/state/*.svelte.ts` talk to the backend only via `lib/api`;
  components never call `invoke` directly.
- Mutating commands return the new snapshot; background results arrive as events, and a store
  subscribes before it requests a refresh (see ADR 0006).
- Native context menus (`lib/platform/menu.ts`) and dialogs (`lib/platform/dialogs.ts`) over
  custom ones.
- No idle polling. React to file watcher events and window focus instead.
- Styling uses the tokens in `ui/src/lib/ui/tokens.css` ("Signal-Board": neutral greys, color
  only with meaning, a single amber accent for "you are here"). UI copy is English, sentence
  case.

## Checks (all must pass, CI runs them on macOS and Windows)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm bindings && cargo test --workspace
pnpm lint && pnpm check && pnpm test
```

Component tests mount Svelte components in happy-dom with the Tauri IPC mocked via
`ui/src/test/tauri-mock.ts` (import it first). To try the app without touching the user's own
repository list, set `BRANCHBOARD_WORKSPACE_FILE` to a scratch file.

The shell may need `source ~/.cargo/env` before cargo is on `PATH`.
`rm` is aliased to `rm -i` in the user's shell; use `/bin/rm` for scripted deletes.
