import { describe, expect, it } from "vitest";

import type { FileDiff } from "../../lib/api";
import type { ReviewComment } from "../../lib/state/comments-model";
import { FILE_ANNOTATIONS, maxLineWidth, unifiedRows } from "./diff-rows";

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
        {
          kind: "context",
          oldLine: 1,
          newLine: 1,
          text: "\tindented",
          noNewline: false,
          tokens: [],
        },
        { kind: "removed", oldLine: 2, newLine: null, text: "old", noNewline: false, tokens: [] },
        { kind: "added", oldLine: null, newLine: 2, text: "new", noNewline: false, tokens: [] },
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

  it("puts annotations below their line, and file annotations first", () => {
    const comment: ReviewComment = {
      id: "comment-1",
      repoId: "api",
      path: "a.txt",
      source: "unstaged",
      lines: null,
      body: "Why?",
      status: "pending",
      outdated: false,
    };
    const rows = unifiedRows(
      diff,
      new Map([
        [FILE_ANNOTATIONS, [{ kind: "comment", comment }]],
        [1, [{ kind: "editor" }]],
      ]),
    );
    expect(rows.map((r) => r.kind)).toEqual(["comment", "hunk", "line", "line", "editor", "line"]);
    expect(rows[0]?.key).toBe("c:comment-1");
    expect(rows.filter((r) => r.kind === "line").map((r) => r.index)).toEqual([0, 1, 2]);
  });
});

describe("maxLineWidth", () => {
  it("counts tabs as four columns", () => {
    expect(maxLineWidth(diff)).toBe(12);
  });
});
