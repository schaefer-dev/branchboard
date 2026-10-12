// Typed subscriptions to backend events. Event names must match
// `src-tauri/src/events.rs`.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { StatusUpdate } from "./generated/StatusUpdate";
import type { SummaryUpdate } from "./generated/SummaryUpdate";

export function onRepoSummary(handler: (update: SummaryUpdate) => void): Promise<UnlistenFn> {
  return listen<SummaryUpdate>("repo-summary", (event) => {
    handler(event.payload);
  });
}

export function onRepoStatus(handler: (update: StatusUpdate) => void): Promise<UnlistenFn> {
  return listen<StatusUpdate>("repo-status", (event) => {
    handler(event.payload);
  });
}
