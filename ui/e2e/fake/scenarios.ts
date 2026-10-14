// Starting states for the fake backend, selected with `?scenario=<name>`.

import type {
  AgentRow,
  AgentState,
  AgentsSnapshot,
  BranchCommits,
  BranchList,
  FileDiff,
  HeadCommit,
  RepoView,
  ReviewFiles,
  StatusSnapshot,
  SummaryState,
  WorkspaceSnapshot,
} from "../../src/lib/api";
import branchesFixture from "../fixtures/branches.json";
import changesFixture from "../fixtures/changes.json";

/** Recorded from the real backend by `src-tauri/tests/ipc.rs`. */
const recorded = changesFixture as unknown as {
  status: StatusSnapshot;
  diffs: Record<string, FileDiff>;
};

/** Recorded from the real backend; `lastFetched` is set per scenario. */
const recordedBranches = branchesFixture as unknown as {
  list: BranchList;
  commits: Record<string, BranchCommits>;
  /** Keyed by `branch:<name>` or `commit:<oid>`. */
  reviews: Record<string, ReviewFiles>;
  /** Keyed by `<from>..<to>:<path>`. */
  reviewDiffs: Record<string, FileDiff>;
};

/** Three minutes ago, so screenshots show a stable "Fetched 3 min ago". */
const recentFetch = () => Math.floor(Date.now() / 1000) - 3 * 60;

function singleBranch(name: string): BranchList {
  return {
    branches: [
      {
        name,
        current: true,
        upstream: { kind: "none" },
        mainline: null,
        committedAt: 1_767_225_600,
      },
    ],
    mainline: "origin/development",
    mainlineExists: false,
    lastFetched: null,
  };
}

/** More branches than the branch list shows without scrolling. */
function manyBranches(count: number): BranchList {
  return {
    branches: Array.from({ length: count }, (_, index) => ({
      name: `feature/topic-${String(index + 1).padStart(2, "0")}`,
      current: index === 0,
      upstream: { kind: "none" as const },
      mainline: null,
      committedAt: 1_767_225_600 - index * 3600,
    })),
    mainline: "origin/development",
    mainlineExists: false,
    lastFetched: null,
  };
}

export interface Scenario {
  workspace: WorkspaceSnapshot;
  statuses: Record<string, StatusSnapshot>;
  /** Keyed by repo id, then `${area}:${path}`. */
  diffs: Record<string, Record<string, FileDiff>>;
  summaries: Record<string, SummaryState>;
  branches: Record<string, BranchList>;
  /** Keyed by repo id, then branch name. */
  commits: Record<string, Record<string, BranchCommits>>;
  /** Reviews per repo id, keyed like the recorded fixture. */
  reviews: Record<string, Record<string, ReviewFiles>>;
  reviewDiffs: Record<string, Record<string, FileDiff>>;
  /** Commits of the current branch per repo id, oldest first; the last is HEAD. */
  history: Record<string, HeadCommit[]>;
  /** What herdr reports; unavailable unless a scenario runs herdr. */
  agents: AgentsSnapshot;
  /** herdr is installed, so the embedded terminal is available. */
  terminal: boolean;
  /** Repositories with a herdr workspace. */
  herdrWorkspaces: string[];
}

const noHerdr = (): AgentsSnapshot => ({
  available: false,
  repos: [],
  agents: [],
  focusedRepoId: null,
});

function repo(id: string, name: string, extra: Partial<RepoView> = {}): RepoView {
  return {
    id,
    name,
    customName: null,
    path: `/home/dev/code/${name}`,
    groupId: null,
    favorite: false,
    mainline: null,
    missing: false,
    ...extra,
  };
}

/** A stable, hex-looking object id derived from `seed`. */
export function fakeOid(seed: string): string {
  const hex = Array.from(seed, (c) => c.charCodeAt(0).toString(16)).join("");
  return hex.padEnd(40, "0").slice(0, 40);
}

export function headCommit(
  oid: string,
  message: string,
  extra: Partial<HeadCommit> = {},
): HeadCommit {
  return {
    oid,
    shortOid: oid.slice(0, 7),
    subject: message.split("\n")[0] ?? "",
    message,
    hasParent: true,
    pushed: true,
    ...extra,
  };
}

const clean = (name: string): StatusSnapshot => ({
  head: { kind: "branch", name },
  headOid: fakeOid(name),
  staged: [],
  unstaged: [],
});

/** The current branch of `api`, oldest first; only its newest commit is not pushed. */
function apiHistory(): HeadCommit[] {
  const ahead = recordedBranches.commits["feature/login-rate-limit"]?.ahead.commits ?? [];
  return [...ahead]
    .reverse()
    .map((summary, index, all) =>
      headCommit(summary.oid, summary.subject, { pushed: index < all.length - 1 }),
    );
}

function ready(status: StatusSnapshot): SummaryState {
  return {
    status: "ready",
    summary: { head: status.head, dirty: status.staged.length + status.unstaged.length > 0 },
  };
}

function defaultScenario(): Scenario {
  const api = structuredClone(recorded.status);
  const history = apiHistory();
  api.head = { kind: "branch", name: "feature/login-rate-limit" };
  api.headOid = history.at(-1)?.oid ?? null;
  const statuses: Record<string, StatusSnapshot> = {
    api,
    web: clean("development"),
    infra: {
      head: { kind: "detached", oid: "b5b537a" },
      headOid: "b5b537a".padEnd(40, "0"),
      staged: [],
      unstaged: [],
    },
    dotfiles: clean("main"),
    blog: { head: { kind: "unborn", name: "main" }, headOid: null, staged: [], unstaged: [] },
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
      settings: { fetchIntervalMinutes: 10, terminalFontSize: 17 },
    },
    statuses,
    diffs: { api: structuredClone(recorded.diffs) },
    summaries,
    branches: {
      api: { ...structuredClone(recordedBranches.list), lastFetched: recentFetch() },
      web: singleBranch("development"),
      infra: manyBranches(12),
      dotfiles: singleBranch("main"),
    },
    commits: { api: structuredClone(recordedBranches.commits) },
    reviews: { api: structuredClone(recordedBranches.reviews) },
    reviewDiffs: { api: structuredClone(recordedBranches.reviewDiffs) },
    history: { api: history },
    agents: noHerdr(),
    terminal: false,
    herdrWorkspaces: [],
  };
}

/** The default workspace with agents running in herdr. */
function agentsScenario(): Scenario {
  const agent = (
    paneId: string,
    agent: string,
    state: AgentState,
    title: string | null,
    workspace: string,
    repoId: string | null,
    tab: string | null = null,
  ): AgentRow => ({ paneId, agent, state, title, workspace, tab, repoId });
  const list: AgentRow[] = [
    agent("w1:p2", "codex", "blocked", "Add rate limit headers", "api", "api", "agents"),
    agent("w2:p1", "claude", "done", "Fix the login form", "web", "web"),
    agent("w6:p1", "claude", "done", "Weekly review", "notes", null),
    agent("w1:p1", "claude", "working", "Refactor the login handler", "api", "api", "agents"),
    agent("w3:p1", "claude", "working", null, "infra", "infra"),
    agent("w4:p1", "claude", "idle", null, "dotfiles", "dotfiles"),
  ];
  const repos = ["api", "web", "infra", "dotfiles"].map((repoId) => {
    const agents = list
      .filter((row) => row.repoId === repoId)
      .map(({ paneId, agent, state, title }) => ({ paneId, agent, state, title }));
    return { repoId, state: agents[0]?.state ?? "idle", agents };
  });
  return {
    ...defaultScenario(),
    agents: { available: true, repos, agents: list, focusedRepoId: "api" },
    terminal: true,
    herdrWorkspaces: ["api", "web", "infra", "dotfiles"],
  };
}

function emptyScenario(): Scenario {
  return {
    workspace: {
      repos: [],
      groups: [],
      activeRepoId: null,
      settings: { fetchIntervalMinutes: 10, terminalFontSize: 17 },
    },
    statuses: {},
    diffs: {},
    summaries: {},
    branches: {},
    commits: {},
    reviews: {},
    reviewDiffs: {},
    history: {},
    agents: noHerdr(),
    terminal: false,
    herdrWorkspaces: [],
  };
}

export const scenarios: Record<string, () => Scenario> = {
  default: defaultScenario,
  empty: emptyScenario,
  agents: agentsScenario,
};
