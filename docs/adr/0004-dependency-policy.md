# 0004 – Pin toolchains and dependencies exactly

- Status: Accepted
- Date: 2026-10-14

## Context

Builds must stay reproducible on two platforms and in CI, and upgrades should be deliberate.

## Decision

- Rust toolchain pinned in `rust-toolchain.toml`; JS packages pinned to exact versions;
  `pnpm-lock.yaml` and `Cargo.lock` are committed; `packageManager` pins pnpm.
- TypeScript stays on 6.0.x until svelte-check and typescript-eslint support 7.x.
- No release-candidate dependencies without an ADR.
- Upgrades happen in dedicated changes that run the full CI.

## Consequences

- Reproducible builds; upgrades are explicit and reviewable.
- Upgrades are manual work.
