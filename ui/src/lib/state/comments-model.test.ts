import { describe, expect, it } from "vitest";

import type { DiffLine } from "../api";
import {
  commentPlace,
  fenceLanguage,
  formatReview,
  locate,
  selectLines,
  type ReviewComment,
} from "./comments-model";

function line(
  kind: DiffLine["kind"],
  oldLine: number | null,
  newLine: number | null,
  text: string,
) {
  return { kind, oldLine, newLine, text, noNewline: false, tokens: [] } satisfies DiffLine;
}

const lines: DiffLine[] = [
  line("context", 10, 10, "import a;"),
  line("removed", 11, null, "let x = 1;"),
  line("removed", 12, null, "let y = 2;"),
  line("added", null, 11, "let x = 3;"),
  line("context", 13, 12, "run(x);"),
];

function comment(extra: Partial<ReviewComment>): ReviewComment {
  return {
    id: "c1",
    repoId: "api",
    path: "src/a.ts",
    source: "unstaged",
    lines: null,
    body: "Why?",
    status: "pending",
    outdated: false,
    ...extra,
  };
}

describe("selectLines", () => {
  it("prefers the new side and quotes its lines", () => {
    expect(selectLines(lines.slice(1, 5))).toEqual({
      side: "new",
      start: 11,
      end: 12,
      quoted: ["let x = 3;", "run(x);"],
    });
  });

  it("uses the old side for removed lines only", () => {
    expect(selectLines(lines.slice(1, 3))).toEqual({
      side: "old",
      start: 11,
      end: 12,
      quoted: ["let x = 1;", "let y = 2;"],
    });
  });

  it("is null without lines", () => {
    expect(selectLines([])).toBeNull();
  });
});

describe("locate", () => {
  const range = { side: "new" as const, start: 11, end: 12, quoted: ["let x = 3;", "run(x);"] };

  it("finds lines where they were", () => {
    expect(locate(lines, range)).toEqual({ first: 3, last: 4, start: 11, end: 12 });
  });

  it("follows lines moved by edits above them", () => {
    const moved = lines.map((l) => ({ ...l, newLine: l.newLine === null ? null : l.newLine + 5 }));
    expect(locate(moved, range)).toEqual({ first: 3, last: 4, start: 16, end: 17 });
  });

  it("is null once the text changed or the lines left the diff", () => {
    const changed = lines.map((l) => (l.text === "run(x);" ? { ...l, text: "run(y);" } : l));
    expect(locate(changed, range)).toBeNull();
    expect(locate(lines.slice(0, 3), range)).toBeNull();
  });

  it("finds removed lines on the old side", () => {
    const removed = { side: "old" as const, start: 12, end: 12, quoted: ["let y = 2;"] };
    expect(locate(lines, removed)).toEqual({ first: 2, last: 2, start: 12, end: 12 });
  });
});

describe("commentPlace", () => {
  it("names lines, removed lines and whole files", () => {
    const at = (side: "old" | "new", start: number, end: number) =>
      commentPlace(comment({ lines: { side, start, end, quoted: [] } }));
    expect(at("new", 3, 3)).toBe("src/a.ts:3");
    expect(at("new", 15, 18)).toBe("src/a.ts:15-18");
    expect(at("old", 3, 3)).toBe("src/a.ts (removed, was line 3)");
    expect(at("old", 3, 4)).toBe("src/a.ts (removed, was lines 3-4)");
    expect(commentPlace(comment({}))).toBe("src/a.ts (whole file)");
  });
});

describe("fenceLanguage", () => {
  it("comes from the extension", () => {
    expect(fenceLanguage("src/a.ts")).toBe("ts");
    expect(fenceLanguage("lib.RS")).toBe("rust");
    expect(fenceLanguage("Makefile")).toBe("");
    expect(fenceLanguage(".gitignore")).toBe("");
  });
});

describe("formatReview", () => {
  it("lists comments by file and line with the quoted code", () => {
    const review = formatReview([
      comment({
        id: "2",
        path: "src/b.rs",
        lines: { side: "new", start: 4, end: 5, quoted: ["fn a() {", "}"] },
        body: " Rename this. \n",
      }),
      comment({ id: "1", path: "src/a.ts", body: "Still imported by the worker." }),
      comment({
        id: "3",
        path: "src/b.rs",
        lines: { side: "new", start: 1, end: 1, quoted: ["use x;"] },
        body: "Unused.",
      }),
    ]);
    expect(review).toBe(
      [
        "src/a.ts (whole file)\nStill imported by the worker.",
        "src/b.rs:1\n```rust\nuse x;\n```\nUnused.",
        "src/b.rs:4-5\n```rust\nfn a() {\n}\n```\nRename this.",
      ].join("\n\n"),
    );
  });

  it("uses a longer fence around code with fences", () => {
    const review = formatReview([
      comment({
        path: "README.md",
        lines: { side: "new", start: 1, end: 1, quoted: ["```sh"] },
      }),
    ]);
    expect(review).toBe("README.md:1\n````markdown\n```sh\n````\nWhy?");
  });
});
