import { describe, expect, it } from "vitest";

import {
  commitLabel,
  composeMessage,
  isEmptyDraft,
  parseDraft,
  splitMessage,
} from "./commit-model";

describe("composeMessage", () => {
  it("separates summary and description with a blank line", () => {
    expect(composeMessage({ summary: " Fix login ", description: "\nWhy it broke.\n" })).toBe(
      "Fix login\n\nWhy it broke.",
    );
  });

  it("leaves out an empty description", () => {
    expect(composeMessage({ summary: "Fix login", description: "  " })).toBe("Fix login");
  });
});

describe("splitMessage", () => {
  it("is the inverse of composeMessage", () => {
    const draft = { summary: "Fix login", description: "Line one\n\nLine three" };
    expect(splitMessage(composeMessage(draft))).toEqual(draft);
  });

  it("handles a summary without description", () => {
    expect(splitMessage("Fix login")).toEqual({ summary: "Fix login", description: "" });
  });
});

describe("parseDraft", () => {
  it("falls back to an empty draft for missing or broken data", () => {
    expect(parseDraft(null)).toEqual({ summary: "", description: "" });
    expect(parseDraft("{not json")).toEqual({ summary: "", description: "" });
    expect(parseDraft('{"summary": 3}')).toEqual({ summary: "", description: "" });
  });

  it("reads a stored draft", () => {
    expect(parseDraft('{"summary":"Fix","description":"More"}')).toEqual({
      summary: "Fix",
      description: "More",
    });
  });
});

describe("isEmptyDraft", () => {
  it("ignores whitespace", () => {
    expect(isEmptyDraft({ summary: " ", description: "\n" })).toBe(true);
    expect(isEmptyDraft({ summary: "", description: "x" })).toBe(false);
  });
});

describe("commitLabel", () => {
  it("counts staged files", () => {
    expect(commitLabel(0, false)).toBe("Commit");
    expect(commitLabel(1, false)).toBe("Commit 1 file");
    expect(commitLabel(3, false)).toBe("Commit 3 files");
    expect(commitLabel(3, true)).toBe("Amend commit");
  });
});
