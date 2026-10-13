import { describe, expect, it } from "vitest";

import type { Branch } from "../api";
import { sortBranches, timeAgo } from "./branches-model";

const branch = (name: string, committedAt: number): Branch => ({
  name,
  current: false,
  upstream: { kind: "none" },
  mainline: null,
  committedAt,
});

describe("sortBranches", () => {
  const branches = [
    branch("b", 100),
    branch("a", 300),
    branch("feature10", 200),
    branch("feature2", 200),
  ];

  it("puts the most recently committed first, then by name", () => {
    expect(sortBranches(branches, "recent").map((b) => b.name)).toEqual([
      "a",
      "feature2",
      "feature10",
      "b",
    ]);
  });

  it("sorts alphabetically with natural numbers", () => {
    expect(sortBranches(branches, "name").map((b) => b.name)).toEqual([
      "a",
      "b",
      "feature2",
      "feature10",
    ]);
  });
});

describe("timeAgo", () => {
  it("describes elapsed time coarsely", () => {
    const now = 1_000_000;
    expect(timeAgo(now - 20, now)).toBe("just now");
    expect(timeAgo(now - 5 * 60, now)).toBe("5 min ago");
    expect(timeAgo(now - 3 * 3600, now)).toBe("3 h ago");
    expect(timeAgo(now - 26 * 3600, now)).toBe("1 day ago");
    expect(timeAgo(now - 3 * 86400, now)).toBe("3 days ago");
    expect(timeAgo(1_767_866_400, 1_767_866_400 + 40 * 86400)).toBe("Jan 8, 2026");
  });
});
