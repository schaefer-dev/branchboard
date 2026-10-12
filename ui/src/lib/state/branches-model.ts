import type { Branch } from "../api";

export type BranchSort = "recent" | "name";

const byName = (a: Branch, b: Branch) =>
  a.name.localeCompare(b.name, undefined, { sensitivity: "base", numeric: true });

/** Most recently committed first, or alphabetical. */
export function sortBranches(branches: readonly Branch[], sort: BranchSort): Branch[] {
  const sorted = [...branches];
  if (sort === "name") return sorted.sort(byName);
  return sorted.sort((a, b) => b.committedAt - a.committedAt || byName(a, b));
}

/** "just now", "5 min ago", "3 h ago", "2 days ago" for a Unix timestamp. */
export function timeAgo(seconds: number, now: number = Date.now() / 1000): string {
  const elapsed = Math.max(0, now - seconds);
  if (elapsed < 60) return "just now";
  const minutes = Math.floor(elapsed / 60);
  if (minutes < 60) return `${String(minutes)} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${String(hours)} h ago`;
  const days = Math.floor(hours / 24);
  return days === 1 ? "1 day ago" : `${String(days)} days ago`;
}
