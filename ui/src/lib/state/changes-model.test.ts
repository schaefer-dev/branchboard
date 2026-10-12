import { describe, expect, it } from "vitest";

import type { FileChange, StatusSnapshot } from "../api";
import { nextCursor, rangeKeys, splitPath, toItems } from "./changes-model";

const file = (path: string, kind: FileChange["kind"] = "modified"): FileChange => ({
  path,
  origPath: null,
  kind,
});

function status(staged: string[], unstaged: string[]): StatusSnapshot {
  return {
    head: { kind: "branch", name: "main" },
    staged: staged.map((p) => file(p)),
    unstaged: unstaged.map((p) => file(p)),
  };
}

describe("toItems", () => {
  it("lists staged before unstaged with unique keys", () => {
    const items = toItems(status(["a"], ["a", "b"]));
    expect(items.map((i) => i.key)).toEqual(["staged:a", "unstaged:a", "unstaged:b"]);
  });
});

describe("nextCursor", () => {
  it("keeps the cursor on a file that is still listed", () => {
    const before = toItems(status([], ["a", "b"]));
    const after = toItems(status(["x"], ["a", "b"]));
    expect(nextCursor(before, after, "unstaged:b")).toBe("unstaged:b");
  });

  it("moves to the next file in the same list after staging", () => {
    const before = toItems(status([], ["a", "b", "c"]));
    const after = toItems(status(["b"], ["a", "c"]));
    expect(nextCursor(before, after, "unstaged:b")).toBe("unstaged:c");
  });

  it("clamps to the last file and falls back to the other list", () => {
    const before = toItems(status([], ["a", "b"]));
    expect(nextCursor(before, toItems(status(["b"], ["a"])), "unstaged:b")).toBe("unstaged:a");
    expect(nextCursor(before, toItems(status(["a", "b"], [])), "unstaged:b")).toBe("staged:a");
  });

  it("selects the first file when nothing was selected", () => {
    expect(nextCursor([], toItems(status([], ["a"])), null)).toBe("unstaged:a");
    expect(nextCursor([], [], null)).toBeNull();
  });
});

describe("rangeKeys", () => {
  it("selects a range within one list only", () => {
    const items = toItems(status(["s1", "s2"], ["u1", "u2", "u3"]));
    expect(rangeKeys(items, "unstaged:u3", "unstaged:u1")).toEqual([
      "unstaged:u1",
      "unstaged:u2",
      "unstaged:u3",
    ]);
    expect(rangeKeys(items, "staged:s1", "unstaged:u2")).toEqual(["unstaged:u2"]);
  });
});

describe("splitPath", () => {
  it("separates folder and file name", () => {
    expect(splitPath("src/lib/main.rs")).toEqual({ dir: "src/lib", name: "main.rs" });
    expect(splitPath("README.md")).toEqual({ dir: "", name: "README.md" });
  });
});
