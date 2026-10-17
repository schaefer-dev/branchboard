// Flattens a diff into rows for the virtualized unified view: fixed-height
// hunk headers and lines, and review comments of varying height.

import type { DiffLine, FileDiff } from "../../lib/api";
import type { ReviewComment } from "../../lib/state/comments-model";

export type DiffRow =
  | { kind: "hunk"; key: string; header: string }
  /** `index` counts the lines of all hunks, as `diffLines` lists them. */
  | { kind: "line"; key: string; line: DiffLine; index: number; hunk: number }
  | { kind: "comment"; key: string; comment: ReviewComment }
  | { kind: "editor"; key: string };

/** What is shown below a line, apart from the diff itself. */
export type Annotation = { kind: "comment"; comment: ReviewComment } | { kind: "editor" };

/** Annotations above all lines, e.g. comments on the whole file. */
export const FILE_ANNOTATIONS = -1;

export function diffLines(diff: FileDiff): DiffLine[] {
  return diff.hunks.flatMap((hunk) => hunk.lines);
}

/** The diff's rows, with `annotations` below the line of the same index. */
export function unifiedRows(
  diff: FileDiff,
  annotations: ReadonlyMap<number, readonly Annotation[]> = new Map(),
): DiffRow[] {
  const rows: DiffRow[] = [];
  const annotate = (index: number) => {
    for (const annotation of annotations.get(index) ?? []) {
      rows.push(
        annotation.kind === "editor"
          ? { kind: "editor", key: "editor" }
          : { kind: "comment", key: `c:${annotation.comment.id}`, comment: annotation.comment },
      );
    }
  };
  annotate(FILE_ANNOTATIONS);
  let index = 0;
  diff.hunks.forEach((hunk, h) => {
    rows.push({ kind: "hunk", key: `h${String(h)}`, header: hunk.header });
    hunk.lines.forEach((line, l) => {
      rows.push({ kind: "line", key: `h${String(h)}l${String(l)}`, line, index, hunk: h });
      annotate(index);
      index++;
    });
  });
  return rows;
}

const TAB_WIDTH = 4;

/** Widest line in characters, counting tabs as `TAB_WIDTH`. */
export function maxLineWidth(diff: FileDiff): number {
  let max = 0;
  for (const hunk of diff.hunks) {
    for (const line of hunk.lines) {
      let width = 0;
      for (const char of line.text) width += char === "\t" ? TAB_WIDTH : 1;
      if (width > max) max = width;
    }
  }
  return max;
}
