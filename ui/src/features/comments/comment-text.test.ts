import { describe, expect, it } from "vitest";

import { agentChoice, countsLabel, linesLabel } from "./comment-text";

describe("linesLabel", () => {
  it("names lines, removed lines and files", () => {
    expect(linesLabel({ side: "new", start: 3, end: 3, quoted: [] })).toBe("Line 3");
    expect(linesLabel({ side: "new", start: 3, end: 5, quoted: [] })).toBe("Lines 3–5");
    expect(linesLabel({ side: "old", start: 3, end: 3, quoted: [] })).toBe("Removed line 3");
    expect(linesLabel(null)).toBe("File");
  });
});

describe("countsLabel", () => {
  it("leaves out what there is none of", () => {
    expect(countsLabel(2, 1)).toBe("2 pending · 1 sent");
    expect(countsLabel(0, 3)).toBe("3 sent");
    expect(countsLabel(1, 0)).toBe("1 pending");
  });
});

describe("agentChoice", () => {
  it("names the agent and marks one waiting for input", () => {
    const agent = { paneId: "w1:p1", agent: "claude", state: "working" as const, title: "Fix" };
    expect(agentChoice(agent)).toBe("Claude · Fix");
    expect(agentChoice({ ...agent, state: "blocked", title: null })).toBe(
      "Claude (waiting for input)",
    );
  });
});
