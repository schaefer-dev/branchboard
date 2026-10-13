import { describe, expect, it } from "vitest";

import type { FileDiff } from "../../lib/api";
import { firstChangedLine } from "./open-in-editor";

const diff = (lines: FileDiff["hunks"][number]["lines"], newStart = 10): FileDiff => ({
  notes: [],
  binary: false,
  tooLarge: false,
  additions: 0,
  deletions: 0,
  lineCount: lines.length,
  hunks: [{ header: "@@", oldStart: newStart, newStart, lines }],
});

describe("firstChangedLine", () => {
  it("finds the first added line", () => {
    expect(
      firstChangedLine(
        diff([
          { kind: "context", oldLine: 10, newLine: 10, text: "", noNewline: false },
          { kind: "added", oldLine: null, newLine: 11, text: "", noNewline: false },
        ]),
      ),
    ).toBe(11);
  });

  it("uses the line following a removal", () => {
    expect(
      firstChangedLine(
        diff([
          { kind: "removed", oldLine: 5, newLine: null, text: "", noNewline: false },
          { kind: "added", oldLine: null, newLine: 5, text: "", noNewline: false },
        ]),
      ),
    ).toBe(5);
  });

  it("falls back to the hunk start for pure deletions and to null without a diff", () => {
    expect(
      firstChangedLine(
        diff([{ kind: "removed", oldLine: 4, newLine: null, text: "", noNewline: false }], 4),
      ),
    ).toBe(4);
    expect(firstChangedLine(null)).toBeNull();
  });
});
