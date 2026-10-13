import { describe, expect, it } from "vitest";

import { segments } from "./segments";

describe("segments", () => {
  it("splits text along tokens and keeps the gaps plain", () => {
    expect(
      segments("let x = 42;", [
        { start: 0, end: 3, kind: "keyword" },
        { start: 8, end: 10, kind: "number" },
      ]),
    ).toEqual([
      { text: "let", kind: "keyword" },
      { text: " x = ", kind: null },
      { text: "42", kind: "number" },
      { text: ";", kind: null },
    ]);
  });

  it("returns the whole line without tokens and ignores out-of-range tokens", () => {
    expect(segments("plain", [])).toEqual([{ text: "plain", kind: null }]);
    expect(segments("ab", [{ start: 1, end: 9, kind: "string" }])).toEqual([
      { text: "a", kind: null },
      { text: "b", kind: "string" },
    ]);
  });

  it("slices by UTF-16 code units like the backend counts them", () => {
    const text = "🦀 = 'ä'";
    expect(segments(text, [{ start: 5, end: 8, kind: "string" }]).at(-1)).toEqual({
      text: "'ä'",
      kind: "string",
    });
  });
});
