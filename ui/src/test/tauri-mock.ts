// Replaces the Tauri IPC layer in component tests. Each test configures
// command results via `respond` and inspects calls via `invoke`.

import { vi } from "vitest";

export const handlers = new Map<string, (args: Record<string, unknown>) => unknown>();

export const invoke = vi.fn((command: string, args: Record<string, unknown> = {}) => {
  const handler = handlers.get(command);
  if (!handler) return Promise.reject(new Error(`No mock for command ${command}`));
  return Promise.resolve(handler(args));
});

export function respond(command: string, handler: (args: Record<string, unknown>) => unknown) {
  handlers.set(command, handler);
}

export function resetTauriMock() {
  handlers.clear();
  invoke.mockClear();
}

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(() => Promise.resolve(() => {})) }));
