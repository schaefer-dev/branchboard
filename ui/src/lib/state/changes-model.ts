// Pure helpers behind the changes list: flattening the status into rows and
// keeping the cursor and selection meaningful as the status changes.

import type { DiffArea, FileChange, StatusSnapshot } from "../api";

export interface ChangeItem {
  /** `${area}:${path}`, unique across both lists. */
  key: string;
  area: DiffArea;
  change: FileChange;
}

export function itemKey(area: DiffArea, path: string): string {
  return `${area}:${path}`;
}

export function toItems(status: StatusSnapshot | null): ChangeItem[] {
  if (!status) return [];
  const items = (area: DiffArea, changes: FileChange[]) =>
    changes.map((change) => ({ key: itemKey(area, change.path), area, change }));
  return [...items("staged", status.staged), ...items("unstaged", status.unstaged)];
}

/**
 * Where the cursor goes after the list changed. A file that is still listed
 * keeps it. Otherwise the cursor stays at the same position in the same list
 * (so staging files one after another just works), falls back to the other
 * list, and finally to the first item.
 */
export function nextCursor(
  before: ChangeItem[],
  after: ChangeItem[],
  cursor: string | null,
): string | null {
  if (cursor && after.some((item) => item.key === cursor)) return cursor;
  const previous = before.find((item) => item.key === cursor);
  if (previous) {
    const sameAreaBefore = before.filter((item) => item.area === previous.area);
    const index = sameAreaBefore.indexOf(previous);
    const sameAreaAfter = after.filter((item) => item.area === previous.area);
    const replacement = sameAreaAfter[Math.min(index, sameAreaAfter.length - 1)];
    if (replacement) return replacement.key;
    const other = after.find((item) => item.area !== previous.area);
    if (other) return other.key;
  }
  return after[0]?.key ?? null;
}

/** Keys from `anchor` to `target` (inclusive) within the anchor's list. */
export function rangeKeys(items: ChangeItem[], anchor: string, target: string): string[] {
  const from = items.findIndex((item) => item.key === anchor);
  const to = items.findIndex((item) => item.key === target);
  const area = items[from]?.area;
  if (from === -1 || to === -1 || area !== items[to]?.area) return [target];
  const [start, end] = from <= to ? [from, to] : [to, from];
  return items.slice(start, end + 1).map((item) => item.key);
}

/** Splits a repository-relative path into folder and file name. */
export function splitPath(path: string): { dir: string; name: string } {
  const slash = path.lastIndexOf("/");
  return slash === -1
    ? { dir: "", name: path }
    : { dir: path.slice(0, slash), name: path.slice(slash + 1) };
}
