// Review comments on diff lines: which lines they point to, finding those
// lines again after the file changed, and the prompt sent to an agent.

import type { DiffLine, RepoId } from "../api";

/**
 * Which diff a comment was written in: the working tree's, or a review's
 * (`review:` and the review target's key). Only a diff from the same source
 * can tell that a comment's lines changed.
 */
export type CommentSource = "unstaged" | "staged" | `review:${string}`;

/** A diff on screen that comments can be added to. */
export interface CommentTarget {
  repoId: RepoId;
  path: string;
  source: CommentSource;
  /** Branch whose commit list contained the review target. */
  reviewBranch?: string;
}

/** The old side holds removed lines, the new side added ones; context lines are on both. */
export type CommentSide = "old" | "new";

/** Consecutive lines of one side of a file, with their text when the comment was written. */
export interface LineRange {
  side: CommentSide;
  start: number;
  end: number;
  quoted: string[];
}

/** Written but not handed to an agent yet, or handed over (pasted or copied). */
export type CommentStatus = "pending" | "sent";

export interface ReviewComment {
  id: string;
  repoId: RepoId;
  path: string;
  source: CommentSource;
  reviewBranch?: string;
  /** `null` for a comment on the whole file. */
  lines: LineRange | null;
  body: string;
  status: CommentStatus;
  /** Its lines changed since it was written, so it no longer shows in the diff. */
  outdated: boolean;
}

/** A comment's lines found in a diff: indexes into the diff's lines, and where they are now. */
export interface Located {
  first: number;
  last: number;
  start: number;
  end: number;
}

/** How far lines may have moved and still be found. */
const SEARCH_DISTANCE = 40;

export function sideLine(line: DiffLine, side: CommentSide): number | null {
  return side === "new" ? line.newLine : line.oldLine;
}

/**
 * The range of selected lines (in diff order, from one hunk): the new side
 * if any of them is on it, else the old side (only removed lines).
 */
export function selectLines(lines: readonly DiffLine[]): LineRange | null {
  const side: CommentSide = lines.some((line) => line.newLine !== null) ? "new" : "old";
  const own = lines.filter((line) => sideLine(line, side) !== null);
  const first = own[0];
  const last = own.at(-1);
  if (!first || !last) return null;
  return {
    side,
    start: sideLine(first, side) ?? 0,
    end: sideLine(last, side) ?? 0,
    quoted: own.map((line) => line.text),
  };
}

/**
 * Finds the lines of `range` in a diff's `lines`: at their line numbers, or
 * moved up or down by edits above them. `null` if their text changed or
 * they are not part of the diff.
 */
export function locate(lines: readonly DiffLine[], range: LineRange): Located | null {
  const index = new Map<number, number>();
  lines.forEach((line, i) => {
    const number = sideLine(line, range.side);
    if (number !== null && !index.has(number)) index.set(number, i);
  });
  const matchesAt = (start: number) =>
    range.quoted.length > 0 &&
    range.quoted.every((text, offset) => {
      const i = index.get(start + offset);
      return i !== undefined && lines[i]?.text === text;
    });
  for (let distance = 0; distance <= SEARCH_DISTANCE; distance++) {
    for (const shift of distance === 0 ? [0] : [distance, -distance]) {
      const start = range.start + shift;
      if (!matchesAt(start)) continue;
      const end = start + range.quoted.length - 1;
      return { first: index.get(start) ?? 0, last: index.get(end) ?? 0, start, end };
    }
  }
  return null;
}

const LANGUAGES: Record<string, string> = {
  bash: "bash",
  c: "c",
  cpp: "cpp",
  cs: "csharp",
  css: "css",
  go: "go",
  h: "c",
  html: "html",
  java: "java",
  js: "js",
  json: "json",
  jsx: "jsx",
  kt: "kotlin",
  md: "markdown",
  mjs: "js",
  py: "python",
  rb: "ruby",
  rs: "rust",
  sh: "bash",
  sql: "sql",
  svelte: "svelte",
  swift: "swift",
  toml: "toml",
  ts: "ts",
  tsx: "tsx",
  vue: "vue",
  yaml: "yaml",
  yml: "yaml",
};

/** The language name for a code fence, from the file extension; "" if unknown. */
export function fenceLanguage(path: string): string {
  const name = path.split("/").pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? (LANGUAGES[name.slice(dot + 1).toLowerCase()] ?? "") : "";
}

/** e.g. "src/a.ts:15-18", "src/a.ts (removed, was line 3)" or "src/a.ts (whole file)". */
export function commentPlace(comment: Pick<ReviewComment, "path" | "lines">): string {
  const { path, lines } = comment;
  if (!lines) return `${path} (whole file)`;
  const numbers =
    lines.start === lines.end ? String(lines.start) : `${String(lines.start)}-${String(lines.end)}`;
  if (lines.side === "new") return `${path}:${numbers}`;
  return `${path} (removed, was line${lines.start === lines.end ? "" : "s"} ${numbers})`;
}

/** Files in order, a file's own comments first, then by line. */
export function byPlace(a: ReviewComment, b: ReviewComment): number {
  return a.path.localeCompare(b.path) || (a.lines?.start ?? 0) - (b.lines?.start ?? 0);
}

/** The review as one prompt: per comment its place, the quoted code and the comment. */
export function formatReview(comments: readonly ReviewComment[]): string {
  return [...comments]
    .sort(byPlace)
    .map((comment) => {
      const parts = [commentPlace(comment)];
      if (comment.lines) {
        const code = comment.lines.quoted.join("\n");
        const runs = code.match(/`+/g) ?? [];
        const fence = "`".repeat(Math.max(3, ...runs.map((run) => run.length + 1)));
        parts.push(`${fence}${fenceLanguage(comment.path)}\n${code}\n${fence}`);
      }
      parts.push(comment.body.trim());
      return parts.join("\n");
    })
    .join("\n\n");
}
