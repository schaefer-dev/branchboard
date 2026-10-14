// Per-device UI preferences in localStorage. Storage can be unavailable, so
// every access falls back to the default.

const PREFIX = "branchboard.";

export function readPref<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  try {
    const value = localStorage.getItem(PREFIX + key);
    return allowed.find((candidate) => candidate === value) ?? fallback;
  } catch {
    return fallback;
  }
}

export function writePref(key: string, value: string): void {
  try {
    localStorage.setItem(PREFIX + key, value);
  } catch {
    // Not persisted; the in-memory value still applies.
  }
}

export function readText(key: string): string | null {
  try {
    return localStorage.getItem(PREFIX + key);
  } catch {
    return null;
  }
}

export function removePref(key: string): void {
  try {
    localStorage.removeItem(PREFIX + key);
  } catch {
    // Nothing stored that could be removed.
  }
}
