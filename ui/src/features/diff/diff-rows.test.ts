import { describe, expect, it } from "vitest";

import type { FileDiff } from "../../lib/api";
import { maxLineWidth, unifiedRows } from "./diff-rows";

const diff: FileDiff = {
  notes: [],
  binary: false,
  tooLarge: false,
  additions: 1,
  deletions: 1,
  lineCount: 3,
  hunks: [
    {
      header: "@@ -1,2 +1,2 @@",
      oldStart: 1,
      newStart: 1,
      lines: [
        { kind: "context", oldLine: 1, newLine: 1, text: "\tindented", noNewline: false },
        { kind: "removed", oldLine: 2, newLine: null, text: "old", noNewline: false },
        { kind: "added", oldLine: null, newLine: 2, text: "new", noNewline: false },
      ],
    },
  ],
};

describe("unifiedRows", () => {
  it("emits a header row per hunk followed by its lines", () => {
    const rows = unifiedRows(diff);
    expect(rows.map((r) => r.kind)).toEqual(["hunk", "line", "line", "line"]);
    expect(new Set(rows.map((r) => r.key)).size).toBe(rows.length);
  });
});

describe("maxLineWidth", () => {
  it("counts tabs as four columns", () => {
    expect(maxLineWidth(diff)).toBe(12);
  });
});
