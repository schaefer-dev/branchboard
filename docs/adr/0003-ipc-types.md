# 0003 – Generate IPC types with ts-rs

- Status: Accepted
- Date: 2026-10-14

## Context

Frontend and backend exchange typed data over Tauri's IPC. Hand-written TypeScript types drift
from the Rust structs. `tauri-specta` would also generate command functions, but its Tauri 2
compatible release is still a release candidate.

## Decision

- Every IPC type derives `ts_rs::TS` with `#[ts(export)]`. `pnpm bindings` regenerates them
  into `ui/src/lib/api/generated/` (committed).
- Command signatures live in `ui/src/lib/api/commands.ts`, one typed wrapper per
  `#[tauri::command]`.
- Commands return `Result<T, AppError>`; the frontend turns every rejection into a
  `CommandError` carrying an `AppError`.
- CI regenerates the bindings and fails if they differ from the committed files.

## Consequences

- Data types cannot drift; command names and argument lists are checked by review only.
- Revisit `tauri-specta` once it is stable.
