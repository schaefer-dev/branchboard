// Starting states for the fake backend, selected with `?scenario=<name>`.

import type {
  FileDiff,
  RepoView,
  StatusSnapshot,
  SummaryState,
  WorkspaceSnapshot,
} from "../../src/lib/api";
import changesFixture from "../fixtures/changes.json";

/** Recorded from the real backend by `src-tauri/tests/ipc.rs`. */
const recorded = changesFixture as unknown as {
  status: StatusSnapshot;
  diffs: Record<string, FileDiff>;
};

export interface Scenario {
  workspace: WorkspaceSnapshot;
  statuses: Record<string, StatusSnapshot>;
  /** Keyed by repo id, then `${area}:${path}`. */
  diffs: Record<string, Record<string, FileDiff>>;
  summaries: Record<string, SummaryState>;
}

function repo(id: string, name: string, extra: Partial<RepoView> = {}): RepoView {
  return {
    id,
    name,
    customName: null,
    path: `/home/dev/code/${name}`,
    groupId: null,
    favorite: false,
    missing: false,
    ...extra,
  };
}

const clean = (name: string): StatusSnapshot => ({
  head: { kind: "branch", name },
  staged: [],
  unstaged: [],
});

function ready(status: StatusSnapshot): SummaryState {
  return {
    status: "ready",
    summary: { head: status.head, dirty: status.staged.length + status.unstaged.length > 0 },
  };
}

function defaultScenario(): Scenario {
  const api = structuredClone(recorded.status);
  api.head = { kind: "branch", name: "feature/login-rate-limit" };
  const statuses: Record<string, StatusSnapshot> = {
    api,
    web: clean("development"),
    infra: { head: { kind: "detached", oid: "b5b537a" }, staged: [], unstaged: [] },
    dotfiles: clean("main"),
    blog: { head: { kind: "unborn", name: "main" }, staged: [], unstaged: [] },
  };
  const summaries: Record<string, SummaryState> = Object.fromEntries(
    Object.entries(statuses).map(([id, status]) => [id, ready(status)]),
  );
  summaries["old-service"] = { status: "missing" };

  return {
    workspace: {
      repos: [
        repo("api", "api", { groupId: "work" }),
        repo("web", "web", { groupId: "work" }),
        repo("infra", "infra", {
          groupId: "work",
          customName: "Infrastructure",
          name: "Infrastructure",
        }),
        repo("dotfiles", "dotfiles", { favorite: true }),
        repo("blog", "blog", { groupId: "side" }),
        repo("old-service", "old-service", { missing: true }),
      ],
      groups: [
        { id: "work", name: "Work", collapsed: false },
        { id: "side", name: "Side projects", collapsed: false },
      ],
      activeRepoId: "api",
    },
    statuses,
    diffs: { api: structuredClone(recorded.diffs) },
    summaries,
  };
}

function emptyScenario(): Scenario {
  return {
    workspace: { repos: [], groups: [], activeRepoId: null },
    statuses: {},
    diffs: {},
    summaries: {},
  };
}

export const scenarios: Record<string, () => Scenario> = {
  default: defaultScenario,
  empty: emptyScenario,
};
