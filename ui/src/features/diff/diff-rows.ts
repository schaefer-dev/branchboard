// Flattens a diff into fixed-height rows for the virtualized unified view.

import type { DiffLine, FileDiff } from "../../lib/api";

export type DiffRow =
  { kind: "hunk"; key: string; header: string } | { kind: "line"; key: string; line: DiffLine };

export function unifiedRows(diff: FileDiff): DiffRow[] {
  const rows: DiffRow[] = [];
  diff.hunks.forEach((hunk, h) => {
    rows.push({ kind: "hunk", key: `h${String(h)}`, header: hunk.header });
    hunk.lines.forEach((line, l) => {
      rows.push({ kind: "line", key: `h${String(h)}l${String(l)}`, line });
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
