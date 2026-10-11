import { describe, expect, it } from "vitest";

import { fuzzyFilter, fuzzyScore } from "./fuzzy";

describe("fuzzyScore", () => {
  it("matches characters in order, case-insensitively", () => {
    expect(fuzzyScore("bkd", "Backend")).not.toBeNull();
    expect(fuzzyScore("dkb", "Backend")).toBeNull();
    expect(fuzzyScore("", "anything")).toBe(0);
  });

  it("prefers word starts and consecutive runs", () => {
    const wordStart = fuzzyScore("pa", "payment-api");
    const scattered = fuzzyScore("pa", "superapp");
    expect(wordStart).toBeGreaterThan(scattered ?? Infinity);
  });
});

describe("fuzzyFilter", () => {
  it("returns matches best first using the best field", () => {
    const repos = [
      { name: "web", path: "/code/frontend/web" },
      { name: "api", path: "/code/backend/api" },
      { name: "infra", path: "/ops/infra" },
    ];
    const result = fuzzyFilter(repos, "api", (repo) => [repo.name, repo.path]);
    expect(result.map((repo) => repo.name)).toEqual(["api"]);
    expect(fuzzyFilter(repos, "code", (repo) => [repo.name, repo.path])).toHaveLength(2);
  });
});
