import { describe, expect, it } from "vitest";

import type { BranchCommits, CommitSummary } from "../api";
import { chooseTarget, reviewTargets, targetKey } from "./review-model";

const commit = (oid: string): CommitSummary => ({
  oid,
  shortOid: oid.slice(0, 7),
  author: "Ada",
  authoredAt: 0,
  subject: `commit ${oid}`,
});

function commits(ahead: string[], behind: string[] | null): BranchCommits {
  return {
    branch: "feature",
    mainline: "origin/main",
    ahead: { commits: ahead.map(commit), truncated: false },
    behind: behind ? { commits: behind.map(commit), truncated: false } : null,
  };
}

describe("reviewTargets", () => {
  it("lists the whole branch, then ahead and behind commits", () => {
    expect(reviewTargets(commits(["a1", "a2"], ["b1"])).map(targetKey)).toEqual([
      "branch:feature",
      "commit:a1",
      "commit:a2",
      "commit:b1",
    ]);
  });

  it("offers single commits only without a mainline", () => {
    expect(reviewTargets(commits(["a1"], null)).map(targetKey)).toEqual(["commit:a1"]);
  });
});

describe("chooseTarget", () => {
  it("keeps a target that is still listed", () => {
    const current = { kind: "commit" as const, oid: "a2" };
    expect(chooseTarget(commits(["a1", "a2"], []), current)).toEqual(current);
  });

  it("falls back to the first target", () => {
    const gone = { kind: "commit" as const, oid: "zz" };
    expect(chooseTarget(commits(["a1"], []), gone)).toEqual({ kind: "branch", name: "feature" });
    expect(chooseTarget(commits([], null), null)).toBeNull();
  });
});
