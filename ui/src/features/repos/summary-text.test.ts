import { describe, expect, it } from "vitest";

import { summaryText } from "./summary-text";

describe("summaryText", () => {
  it("describes each state", () => {
    expect(summaryText(undefined)).toBe("");
    expect(summaryText({ status: "missing" })).toBe("Folder not found");
    expect(summaryText({ status: "failed", message: "x" })).toBe("Status unavailable");
    const ready = (head: Parameters<typeof summaryText>[0] & { status: "ready" }) =>
      summaryText(head);
    expect(
      ready({
        status: "ready",
        summary: { head: { kind: "branch", name: "development" }, dirty: false },
      }),
    ).toBe("development");
    expect(
      ready({ status: "ready", summary: { head: { kind: "unborn", name: "main" }, dirty: false } }),
    ).toBe("main (no commits)");
    expect(
      ready({
        status: "ready",
        summary: { head: { kind: "detached", oid: "abc1234" }, dirty: true },
      }),
    ).toBe("Detached at abc1234");
  });
});
