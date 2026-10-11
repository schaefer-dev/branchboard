// Typed subscriptions to backend events. Event names must match
// `src-tauri/src/events.rs`.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { SummaryUpdate } from "./generated/SummaryUpdate";

export function onRepoSummary(handler: (update: SummaryUpdate) => void): Promise<UnlistenFn> {
  return listen<SummaryUpdate>("repo-summary", (event) => {
    handler(event.payload);
  });
}
