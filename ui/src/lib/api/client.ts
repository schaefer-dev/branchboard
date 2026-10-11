import { invoke, type InvokeArgs } from "@tauri-apps/api/core";

import type { AppError } from "./generated/AppError";

/** Thrown by every API call. Wraps the backend's structured error. */
export class CommandError extends Error {
  readonly error: AppError;

  constructor(error: AppError) {
    super(error.message);
    this.name = "CommandError";
    this.error = error;
  }
}

function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    typeof value.kind === "string" &&
    "message" in value &&
    typeof value.message === "string"
  );
}

/** Normalizes anything an `invoke` call can reject with into an `AppError`. */
export function toAppError(raw: unknown): AppError {
  if (isAppError(raw)) {
    return { kind: raw.kind, message: raw.message, detail: raw.detail ?? null };
  }
  const message =
    typeof raw === "string" ? raw : raw instanceof Error ? raw.message : "Unexpected error";
  return { kind: "internal", message, detail: null };
}

/** Invokes a backend command. Rejects with a `CommandError`. */
export async function call<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    throw new CommandError(toAppError(raw));
  }
}
