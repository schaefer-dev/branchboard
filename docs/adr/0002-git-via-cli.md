# 0002 – Talk to git through its CLI

- Status: Accepted
- Date: 2026-10-14

## Context

Branchboard must behave exactly like git in the terminal and in VS Code: respect config,
attributes, filters, LFS, fsmonitor, credential helpers, SSH agents and hooks. Candidates were
libgit2 (`git2`), gitoxide (`gix`) and the git CLI.

## Decision

All git operations run through the user's **git executable**, wrapped by `git-core`'s
`GitRunner`:

- Machine-readable output only (`--porcelain=v2 -z`, `for-each-ref --format`), never parsed
  error messages; exit codes decide success.
- Fixed global options: `--no-optional-locks` (no lock contention with editors polling git),
  `core.quotepath=false`, `color.ui=never`; `GIT_TERMINAL_PROMPT=0` so nothing waits on a
  prompt that can never be answered.
- No console windows on Windows (`CREATE_NO_WINDOW`). On macOS the login shell's `PATH` is
  merged in, because GUI apps do not inherit it.
- Minimum version **2.41**, for `for-each-ref`'s `%(ahead-behind:)` atom: ahead/behind counts
  for all branches in a single process.

## Consequences

- Identical results to the user's own git; no reimplementation of git semantics.
- One process per operation (≈5–15 ms on macOS, more on Windows). Refreshes are debounced
  and coalesced to keep this cheap.
- `--no-optional-locks` means `git status` does not refresh the index stat cache, the same
  trade-off VS Code makes.
- Reads may later move to gitoxide behind the same interface if benchmarks justify it.
