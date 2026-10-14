import { describe, expect, it } from "vitest";

import type { RepoAgents } from "../../lib/api";
import {
  agentName,
  agentPlace,
  describeAgent,
  describeAgentRow,
  describeAgents,
} from "./agent-text";

describe("agent text", () => {
  it("names agents", () => {
    expect(agentName("claude")).toBe("Claude");
    expect(agentName("")).toBe("");
  });

  it("describes an agent with its topic", () => {
    expect(
      describeAgent({
        paneId: "w1:p1",
        agent: "claude",
        state: "blocked",
        title: "Fix the login form",
      }),
    ).toBe("Claude needs input · Fix the login form");
    expect(describeAgent({ paneId: "w1:p2", agent: "codex", state: "working", title: null })).toBe(
      "Codex is working",
    );
  });

  it("lists every agent of a repository", () => {
    const repo: RepoAgents = {
      repoId: "api",
      state: "done",
      agents: [
        { paneId: "w1:p1", agent: "claude", state: "done", title: null },
        { paneId: "w1:p2", agent: "pi", state: "unknown", title: null },
      ],
    };
    expect(describeAgents(repo)).toBe("Claude is done\nPi is running");
  });

  it("names where an agent runs", () => {
    const row = {
      paneId: "w2:p1",
      agent: "claude",
      state: "done" as const,
      title: null,
      workspace: "notes",
      tab: null,
      repoId: null,
    };
    expect(agentPlace(row)).toBe("notes");
    expect(describeAgentRow({ ...row, tab: "agents" })).toBe("Claude is done, in notes · agents");
  });
});
