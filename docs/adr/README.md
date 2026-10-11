# Architecture Decision Records

Each ADR captures one significant decision: its context, the decision and its consequences.
ADRs are immutable once accepted; a later ADR supersedes an earlier one instead of editing it.

| #                                     | Title                                          | Status   |
| ------------------------------------- | ---------------------------------------------- | -------- |
| [0001](0001-tech-stack.md)            | Tauri 2, Rust core and Svelte 5                | Accepted |
| [0002](0002-git-via-cli.md)           | Talk to git through its CLI                    | Accepted |
| [0003](0003-ipc-types.md)             | Generate IPC types with ts-rs                  | Accepted |
| [0004](0004-dependency-policy.md)     | Pin toolchains and dependencies exactly        | Accepted |
| [0005](0005-workspace-persistence.md) | Persist the workspace as a versioned JSON file | Accepted |
| [0006](0006-frontend-state.md)        | Frontend state, backend events and native UI   | Accepted |

New ADRs copy [`template.md`](template.md) and take the next free number.
