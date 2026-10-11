# 0001 – Tauri 2, Rust core and Svelte 5

- Status: Accepted
- Date: 2026-10-14

## Context

Branchboard is a personal git GUI for macOS and Windows. Low resource usage and high
performance on both platforms are the top requirements, followed by a good UI for diffs and
lists and long-term maintainability.

Options considered: Electron (bundles Chromium, 150–300 MB idle), a native Rust UI such as
GPUI/Slint/iced (leanest, but much higher UI effort and young ecosystems), Avalonia/.NET
(runtime overhead, weaker git ecosystem) and Tauri 2.

## Decision

- **Tauri 2** with the system WebView (WKWebView on macOS, WebView2 on Windows).
- **Rust** for all git work, file watching and persistence.
- **Svelte 5 + TypeScript** on plain Vite (no SvelteKit; there is no routing need).
  Svelte compiles to small bundles without a virtual DOM; runes fit store-style state.

## Consequences

- Small install and memory footprint; native performance in the core.
- UI is built with web technology: fast iteration on diff rendering and virtualised lists.
- Two languages in one repo; the IPC boundary must stay typed (see 0003).
- WebView differences between WebKit and Chromium must be tested on both platforms.
