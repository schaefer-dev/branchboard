// An in-memory stand-in for the Rust backend, used by the Playwright tests.
// It answers the same commands with the same types and keeps state, so
// staging a file really moves it between the lists. Status and diff data come
// from fixtures recorded from the real backend.

import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

import type {
  AgentsSnapshot,
  AppError,
  BranchList,
  ChangeKind,
  DiffRequest,
  FileChange,
  FileDiff,
  HeadCommit,
  RepoView,
  ReviewDiffRequest,
  ReviewTarget,
  StatusSnapshot,
  WorkspaceSnapshot,
} from "../../src/lib/api";
import type { FakeControl, RecordedCall } from "./control";
import { fakeOid, headCommit, scenarios, type Scenario } from "./scenarios";

type Args = Record<string, unknown>;

interface MenuItem {
  text: string;
  run?: () => void;
}

class CommandFailure extends Error {
  readonly error: AppError;

  constructor(error: AppError) {
    super(error.message);
    this.error = error;
  }
}

const fail = (kind: AppError["kind"], message: string, detail: string | null = null): never => {
  throw new CommandFailure({ kind, message, detail });
};

const now = () => Math.floor(Date.now() / 1000);

const byPath = (a: FileChange, b: FileChange) => a.path.localeCompare(b.path);

/** Git lists tracked changes before untracked ones, each sorted by path. */
function sortUnstaged(changes: FileChange[]): FileChange[] {
  const tracked = changes.filter((c) => c.kind !== "untracked").sort(byPath);
  const untracked = changes.filter((c) => c.kind === "untracked").sort(byPath);
  return [...tracked, ...untracked];
}

const emptyDiff: FileDiff = {
  notes: [],
  binary: false,
  hunks: [],
  additions: 0,
  deletions: 0,
  tooLarge: false,
  lineCount: 0,
};

export class FakeBackend {
  private readonly state: Scenario;
  private readonly log: RecordedCall[] = [];
  private lastMenu: MenuItem[] = [];
  private confirm = true;
  private pickedFolders: string[] = [];
  private failures = new Map<string, string>();
  private nextFetch: Promise<void> | null = null;
  private finishFetch: (() => void) | null = null;
  /** What each commit made through the fake contains, for undoing it. */
  private committedFiles = new Map<string, FileChange[]>();
  private nextId = 1;
  /** The fake herdr client: what the app typed, its size and its output channel. */
  private terminal: {
    output: ChannelLike | null;
    input: string;
    size: { cols: number; rows: number } | null;
  } = { output: null, input: "", size: null };

  constructor(scenario: Scenario) {
    this.state = scenario;
  }

  async handle(cmd: string, args: Args): Promise<unknown> {
    this.log.push({ cmd, args: structuredClone(stripChannels(args)) });
    const workspace = this.state.workspace;
    switch (cmd) {
      case "get_app_info":
        return { version: "0.0.0-e2e" };
      case "get_git_info":
        return { path: "/usr/bin/git", version: { major: 2, minor: 54, patch: 0 } };
      case "get_workspace":
        return this.snapshot();
      case "add_repo":
        return this.addRepo(String(args.path));
      case "relocate_repo": {
        const repo = this.repo(args.repoId);
        repo.path = String(args.path);
        repo.missing = false;
        return this.snapshot();
      }
      case "remove_repo":
        this.repo(args.repoId);
        workspace.repos = workspace.repos.filter((r) => r.id !== args.repoId);
        if (workspace.activeRepoId === args.repoId) workspace.activeRepoId = null;
        return this.snapshot();
      case "rename_repo": {
        const repo = this.repo(args.repoId);
        const name = typeof args.name === "string" ? args.name.trim() : "";
        repo.customName = name || null;
        repo.name = name || (repo.path.split("/").pop() ?? repo.path);
        return this.snapshot();
      }
      case "set_repo_favorite":
        this.repo(args.repoId).favorite = Boolean(args.favorite);
        return this.snapshot();
      case "set_repo_group":
        this.repo(args.repoId).groupId = (args.groupId as string | null) ?? null;
        return this.snapshot();
      case "set_active_repo":
        workspace.activeRepoId = (args.repoId as string | null) ?? null;
        return this.snapshot();
      case "create_group": {
        const name = String(args.name).trim();
        if (!name) fail("invalidInput", "Enter a name.");
        if (workspace.groups.some((g) => g.name.toLowerCase() === name.toLowerCase())) {
          fail("invalidInput", `A group named “${name}” already exists.`);
        }
        const id = `group-${String(this.nextId++)}`;
        workspace.groups.push({ id, name, collapsed: false });
        if (typeof args.repoId === "string") this.repo(args.repoId).groupId = id;
        return this.snapshot();
      }
      case "rename_group":
        this.group(args.groupId).name = String(args.name).trim();
        return this.snapshot();
      case "delete_group":
        this.group(args.groupId);
        workspace.groups = workspace.groups.filter((g) => g.id !== args.groupId);
        for (const repo of workspace.repos) if (repo.groupId === args.groupId) repo.groupId = null;
        return this.snapshot();
      case "set_group_collapsed":
        this.group(args.groupId).collapsed = Boolean(args.collapsed);
        return this.snapshot();
      case "refresh_summaries":
        void this.publishSummaries();
        return null;
      case "get_status":
        return this.status(String(args.repoId));
      case "get_diff":
        return this.diff(String(args.repoId), args.request as DiffRequest);
      case "stage_paths":
        return this.write(String(args.repoId), (status) => {
          stage(status, args.paths as string[], this.diffsOf(String(args.repoId)));
        });
      case "unstage_paths":
        return this.write(String(args.repoId), (status) => {
          unstage(status, args.paths as string[]);
        });
      case "discard_paths":
        return this.write(String(args.repoId), (status) => {
          const paths = new Set(args.paths as string[]);
          status.unstaged = status.unstaged.filter(
            (c) => !paths.has(c.path) || c.kind === "conflicted",
          );
        });
      case "commit":
        return this.commit(String(args.repoId), String(args.message), Boolean(args.amend));
      case "get_head_commit":
        return this.headOf(String(args.repoId));
      case "undo_last_commit":
        return this.undoCommit(String(args.repoId));
      case "get_branches":
        return this.branchList(String(args.repoId));
      case "open_in_editor":
        this.repo(args.repoId);
        return null;
      case "get_branch_commits": {
        const repo = this.repo(args.repoId);
        const branch = String(args.branch);
        const empty = { commits: [], truncated: false };
        return (
          this.state.commits[repo.id]?.[branch] ?? {
            branch,
            mainline: this.mainlineOf(repo),
            ahead: empty,
            behind: null,
          }
        );
      }
      case "get_review_files": {
        const repo = this.repo(args.repoId);
        const target = args.target as ReviewTarget;
        const key = target.kind === "branch" ? `branch:${target.name}` : `commit:${target.oid}`;
        return (
          this.state.reviews[repo.id]?.[key] ??
          fail("gitFailed", "Git did not respond as expected.", `${key} does not name a commit`)
        );
      }
      case "get_review_diff": {
        const repo = this.repo(args.repoId);
        const request = args.request as ReviewDiffRequest;
        const key = `${request.range.from}..${request.range.to}:${request.path}`;
        return this.state.reviewDiffs[repo.id]?.[key] ?? emptyDiff;
      }
      case "switch_branch":
        return this.switchBranch(String(args.repoId), String(args.branch));
      case "fetch_repo":
        return this.fetchRepo(String(args.repoId));
      case "set_mainline": {
        const repo = this.repo(args.repoId);
        const mainline = typeof args.mainline === "string" ? args.mainline.trim() : "";
        repo.mainline = mainline || null;
        return this.snapshot();
      }
      case "set_fetch_interval":
        workspace.settings.fetchIntervalMinutes = Number(args.minutes);
        return this.snapshot();
      case "set_terminal_font_size":
        workspace.settings.terminalFontSize = Math.min(32, Math.max(9, Number(args.size)));
        return this.snapshot();
      case "get_agents":
        return structuredClone(this.state.agents);
      case "focus_agent":
        if (!this.state.agents.agents.some((agent) => agent.paneId === args.paneId)) {
          fail("herdrFailed", "herdr reported an error.", "not_found: agent not found");
        }
        return null;
      case "terminal_info":
        return { available: this.state.terminal };
      case "terminal_open": {
        if (!this.state.terminal) fail("herdrUnavailable", "herdr was not found.");
        this.terminal = {
          output: args.output as ChannelLike,
          input: "",
          size: { cols: Number(args.cols), rows: Number(args.rows) },
        };
        // Like herdr: draw something once attached.
        this.sendTerminal("herdr (fake)\r\n$ ");
        return null;
      }
      case "terminal_write":
        if (!this.terminal.output) fail("terminalFailed", "The terminal has closed.");
        this.terminal.input += String(args.data);
        return null;
      case "terminal_resize":
        this.terminal.size = { cols: Number(args.cols), rows: Number(args.rows) };
        return null;
      case "terminal_close":
        this.terminal.output = null;
        return null;
      case "open_in_herdr": {
        const repo = this.repo(args.repoId);
        if (!this.state.agents.available) {
          fail("herdrUnavailable", "herdr is not running. Start herdr and try again.");
        }
        if (!this.state.herdrWorkspaces.includes(repo.id)) this.state.herdrWorkspaces.push(repo.id);
        await this.herdrFocus(repo.id);
        return null;
      }
      case "focus_repo_in_herdr": {
        const repo = this.repo(args.repoId);
        if (!this.state.herdrWorkspaces.includes(repo.id)) return false;
        await this.herdrFocus(repo.id);
        return true;
      }
      case "plugin:dialog|open":
        return this.pickedFolders.length > 0 ? this.pickedFolders : null;
      case "plugin:dialog|message": {
        const options = args as { buttons?: { OkCancelCustom?: [string, string] } };
        const [ok, cancel] = options.buttons?.OkCancelCustom ?? ["Ok", "Cancel"];
        return this.confirm ? ok : cancel;
      }
      case "plugin:menu|new":
        if (args.kind === "Menu") this.lastMenu = flattenMenu(args.options);
        return [this.nextId++, `menu-${String(this.nextId)}`];
      case "plugin:menu|popup":
      case "plugin:resources|close":
        return null;
      default:
        return fail("internal", `The fake backend does not implement ${cmd}`);
    }
  }

  control(): FakeControl {
    return {
      calls: () => structuredClone(this.log),
      menu: () => this.lastMenu.map((item) => item.text),
      clickMenu: (text) => {
        const item = this.lastMenu.find((candidate) => candidate.text === text);
        if (!item?.run) throw new Error(`No menu item ${text}`);
        item.run();
      },
      setConfirm: (answer) => {
        this.confirm = answer;
      },
      setPickedFolders: (paths) => {
        this.pickedFolders = paths;
      },
      failNext: (command, stderr) => {
        this.failures.set(command, stderr);
      },
      holdNextFetch: () => {
        if (this.nextFetch !== null || this.finishFetch !== null) {
          throw new Error("A fetch is already held");
        }
        this.nextFetch = new Promise<void>((resolve) => {
          this.finishFetch = resolve;
        });
      },
      releaseFetch: () => {
        if (this.nextFetch !== null || this.finishFetch === null) {
          throw new Error("No held fetch is running");
        }
        this.finishFetch();
        this.finishFetch = null;
      },
      drop: async (paths) => {
        const position = { x: 100, y: 100 };
        await emit("tauri://drag-enter", { paths, position });
        await emit("tauri://drag-drop", { paths, position });
      },
      terminalInput: () => this.terminal.input,
      terminalSize: () => this.terminal.size,
      terminalOutput: (text) => {
        this.sendTerminal(text);
      },
      terminalExit: (code) => {
        const output = this.terminal.output;
        this.terminal.output = null;
        output?.onmessage({ exit: code });
      },
      herdrFocus: (repoId) => this.herdrFocus(repoId),
      setAgents: async (agents: AgentsSnapshot) => {
        this.state.agents = agents;
        await emit("repo-agents", agents);
      },
      replaceStatus: async (repoId, unstagedPaths) => {
        const status = this.status(repoId);
        status.unstaged = sortUnstaged(
          unstagedPaths.map((path) => ({ path, origPath: null, kind: "modified" })),
        );
        await emit("repo-status", { repoId, status });
      },
    };
  }

  /** herdr shows the workspace of `repoId`, as all its clients follow. */
  private async herdrFocus(repoId: string): Promise<void> {
    if (this.state.agents.focusedRepoId === repoId) return;
    this.state.agents.focusedRepoId = repoId;
    await emit("repo-agents", structuredClone(this.state.agents));
  }

  private sendTerminal(text: string): void {
    const bytes = new TextEncoder().encode(text);
    // The real channel delivers raw output as an `ArrayBuffer`.
    this.terminal.output?.onmessage(bytes.buffer.slice(0, bytes.byteLength));
  }

  private snapshot(): WorkspaceSnapshot {
    return structuredClone(this.state.workspace);
  }

  private repo(id: unknown): RepoView {
    return (
      this.state.workspace.repos.find((repo) => repo.id === id) ??
      fail("notFound", "This repository is no longer in your list.")
    );
  }

  private group(id: unknown) {
    return (
      this.state.workspace.groups.find((group) => group.id === id) ??
      fail("notFound", "This group no longer exists.")
    );
  }

  private addRepo(path: string) {
    if (path.includes("not-a-repo")) {
      fail("notARepository", `${path} is not a git repository.`);
    }
    const workspace = this.state.workspace;
    const existing = workspace.repos.find((repo) => repo.path === path);
    const name = path.split("/").filter(Boolean).pop() ?? path;
    const id = existing?.id ?? `repo-${String(this.nextId++)}`;
    if (!existing) {
      workspace.repos.push({
        id,
        name,
        customName: null,
        path,
        groupId: null,
        favorite: false,
        mainline: null,
        missing: false,
      });
      this.state.statuses[id] = {
        head: { kind: "branch", name: "development" },
        headOid: fakeOid(id),
        staged: [],
        unstaged: [],
      };
      this.state.summaries[id] = {
        status: "ready",
        summary: { head: { kind: "branch", name: "development" }, dirty: false },
      };
      void this.publishSummaries([id]);
    }
    workspace.activeRepoId = id;
    return { workspace: this.snapshot(), repoId: id, alreadyAdded: Boolean(existing) };
  }

  /** Like the backend: the chosen mainline, else origin/development where it
   * exists, else origin/main where it exists, else origin/master. */
  private mainlineOf(repo: RepoView): string {
    if (repo.mainline) return repo.mainline;
    const hasDevelopment = this.state.branches[repo.id]?.branches.some((b) => b.mainline);
    return hasDevelopment ? "origin/development" : "origin/master";
  }

  private branchList(repoId: string): BranchList {
    const repo = this.repo(repoId);
    const list = (this.state.branches[repoId] ??= {
      branches: [],
      mainline: this.mainlineOf(repo),
      mainlineExists: false,
      lastFetched: null,
    });
    // The fake only knows origin/development, and only where branches have
    // counts against it; any other ref "does not exist".
    const mainline = this.mainlineOf(repo);
    const exists = mainline === "origin/development" && list.branches.some((b) => b.mainline);
    return structuredClone({ ...list, mainline, mainlineExists: exists });
  }

  private takeFailure(command: string, message: string): void {
    const stderr = this.failures.get(command);
    if (stderr === undefined) return;
    this.failures.delete(command);
    fail("gitFailed", message, stderr);
  }

  private async switchBranch(repoId: string, name: string): Promise<BranchList> {
    this.takeFailure("switch_branch", `Could not switch to ${name}.`);
    const list = this.state.branches[repoId];
    if (!list) return this.branchList(repoId);
    for (const branch of list.branches) branch.current = branch.name === name;
    const status = this.status(repoId);
    status.head = { kind: "branch", name };
    await emit("repo-status", { repoId, status: structuredClone(status) });
    return this.branchList(repoId);
  }

  private async fetchRepo(repoId: string): Promise<BranchList> {
    const completion = this.nextFetch;
    this.nextFetch = null;
    await emit("repo-fetch", { repoId, state: { status: "running" } });
    await (completion ?? new Promise<void>((resolve) => setTimeout(resolve, 150)));
    const stderr = this.failures.get("fetch_repo");
    if (stderr !== undefined) {
      await emit("repo-fetch", {
        repoId,
        state: { status: "failed", message: "Fetching failed.", detail: stderr },
      });
    }
    this.takeFailure("fetch_repo", "Fetching failed.");
    const list = this.state.branches[repoId];
    if (list) list.lastFetched = now();
    await emit("repo-fetch", { repoId, state: { status: "done" } });
    return this.branchList(repoId);
  }

  private status(repoId: string): StatusSnapshot {
    this.repo(repoId);
    return (this.state.statuses[repoId] ??= {
      head: { kind: "branch", name: "development" },
      headOid: fakeOid(repoId),
      staged: [],
      unstaged: [],
    });
  }

  /** Commits of the current branch, oldest first. Repos without a recorded
   * history have one pushed commit at their `headOid`. */
  private historyOf(repoId: string): HeadCommit[] {
    const oid = this.status(repoId).headOid;
    return (this.state.history[repoId] ??= oid
      ? [headCommit(oid, "Initial commit", { hasParent: false })]
      : []);
  }

  private headOf(repoId: string): HeadCommit | null {
    return structuredClone(this.historyOf(repoId).at(-1) ?? null);
  }

  private async commit(repoId: string, message: string, amend: boolean) {
    this.takeFailure("commit", "Commit failed.");
    const text = message.trim();
    if (!text) fail("invalidInput", "Enter a commit message.");
    const status = this.status(repoId);
    const history = this.historyOf(repoId);
    if (!amend && status.staged.length === 0) {
      fail("gitFailed", "Commit failed.", "nothing added to commit");
    }
    const previous = amend ? history.pop() : undefined;
    const files = [
      ...(previous ? (this.committedFiles.get(previous.oid) ?? []) : []),
      ...status.staged,
    ];
    const oid = fakeOid(`commit-${String(this.nextId++)}`);
    const head = headCommit(oid, text, {
      hasParent: amend ? (previous?.hasParent ?? false) : history.length > 0,
      pushed: false,
    });
    history.push(head);
    this.committedFiles.set(oid, files);
    if (!amend) this.countNewCommit(repoId);
    status.staged = [];
    status.headOid = oid;
    const snapshot = structuredClone(status);
    await emit("repo-status", { repoId, status: snapshot });
    return { status: snapshot, head: structuredClone(head) };
  }

  private async undoCommit(repoId: string) {
    const history = this.historyOf(repoId);
    const undone = history.at(-1);
    if (!undone?.hasParent) fail("invalidInput", "There is no earlier commit to go back to.");
    if (undone?.pushed) {
      fail(
        "invalidInput",
        "This commit is already on a remote. Undoing it would rewrite shared history.",
      );
    }
    history.pop();
    const head = history.at(-1) ?? fail("internal", "The fake history is empty.");
    const status = this.status(repoId);
    for (const file of this.committedFiles.get(undone?.oid ?? "") ?? []) {
      if (!status.staged.some((c) => c.path === file.path)) status.staged.push(file);
    }
    status.staged.sort(byPath);
    status.headOid = head.oid;
    const snapshot = structuredClone(status);
    await emit("repo-status", { repoId, status: snapshot });
    return {
      status: snapshot,
      head: structuredClone(head),
      message: undone?.message ?? "",
    };
  }

  /** A new commit puts the current branch one further ahead. */
  private countNewCommit(repoId: string): void {
    const branch = this.state.branches[repoId]?.branches.find((b) => b.current);
    if (!branch) return;
    if (branch.mainline) branch.mainline.ahead += 1;
    if (branch.upstream.kind === "tracking") branch.upstream.counts.ahead += 1;
    branch.committedAt = now();
  }

  private diffsOf(repoId: string): Record<string, FileDiff> {
    return (this.state.diffs[repoId] ??= {});
  }

  private diff(repoId: string, request: DiffRequest): FileDiff {
    const diff = this.diffsOf(repoId)[`${request.area}:${request.path}`] ?? emptyDiff;
    if (request.full && diff.tooLarge) {
      return { ...diff, tooLarge: false, hunks: syntheticHunks(diff.lineCount) };
    }
    return diff;
  }

  private async write(repoId: string, change: (status: StatusSnapshot) => void) {
    const status = this.status(repoId);
    change(status);
    const snapshot = structuredClone(status);
    await emit("repo-status", { repoId, status: snapshot });
    return snapshot;
  }

  private async publishSummaries(only?: string[]) {
    for (const [repoId, state] of Object.entries(this.state.summaries)) {
      if (only && !only.includes(repoId)) continue;
      await emit("repo-summary", { repoId, state });
    }
  }
}

function stage(status: StatusSnapshot, paths: string[], diffs: Record<string, FileDiff>) {
  for (const path of paths) {
    const change = status.unstaged.find((c) => c.path === path);
    if (!change) continue;
    status.unstaged = status.unstaged.filter((c) => c !== change);
    const kind: ChangeKind =
      change.kind === "untracked"
        ? "added"
        : change.kind === "conflicted"
          ? "modified"
          : change.kind;
    if (!status.staged.some((c) => c.path === path)) {
      status.staged.push({ path, origPath: null, kind });
    }
    const unstagedDiff = diffs[`unstaged:${path}`];
    if (unstagedDiff) diffs[`staged:${path}`] ??= unstagedDiff;
  }
  status.staged.sort(byPath);
}

function unstage(status: StatusSnapshot, paths: string[]) {
  for (const path of paths) {
    const change = status.staged.find((c) => c.path === path);
    if (!change) continue;
    status.staged = status.staged.filter((c) => c !== change);
    const back: FileChange[] =
      change.kind === "added"
        ? [{ path, origPath: null, kind: "untracked" }]
        : change.kind === "renamed" && change.origPath
          ? [
              { path: change.origPath, origPath: null, kind: "deleted" },
              { path, origPath: null, kind: "untracked" },
            ]
          : [{ path, origPath: null, kind: change.kind }];
    for (const entry of back) {
      if (!status.unstaged.some((c) => c.path === entry.path)) status.unstaged.push(entry);
    }
  }
  status.unstaged = sortUnstaged(status.unstaged);
}

function syntheticHunks(lines: number): FileDiff["hunks"] {
  return [
    {
      header: `@@ -0,0 +1,${String(lines)} @@`,
      oldStart: 0,
      newStart: 1,
      lines: Array.from({ length: lines }, (_, i) => ({
        kind: "added" as const,
        oldLine: null,
        newLine: i + 1,
        text: `row ${String(i + 1)}`,
        noNewline: false,
        tokens: [],
      })),
    },
  ];
}

interface ChannelLike {
  onmessage: (message: unknown) => void;
}

interface MenuOptions {
  text?: string;
  items?: MenuOptions[];
  handler?: ChannelLike;
}

function flattenMenu(options: unknown, prefix = ""): MenuItem[] {
  const items = (options as MenuOptions | undefined)?.items ?? [];
  return items.flatMap((item) => {
    if (!item.text) return [];
    const text = `${prefix}${item.text}`;
    const own: MenuItem = { text };
    const handler = item.handler;
    if (handler && !item.items) {
      own.run = () => {
        handler.onmessage(text);
      };
    }
    return [own, ...(item.items ? flattenMenu(item, `${text}/`) : [])];
  });
}

/** Channels (menu handlers) cannot be cloned; record them as placeholders. */
function stripChannels(value: unknown): Args {
  return JSON.parse(
    JSON.stringify(value ?? {}, (_key, inner: unknown) =>
      inner && typeof inner === "object" && "onmessage" in inner ? "[channel]" : inner,
    ),
  ) as Args;
}

/** Replaces Tauri's IPC with the fake backend. */
export function installFakeBackend(): void {
  const name = new URLSearchParams(window.location.search).get("scenario") ?? "default";
  const scenario = scenarios[name];
  if (!scenario) throw new Error(`Unknown scenario ${name}`);
  const backend = new FakeBackend(scenario());

  mockWindows("main");
  mockIPC(
    async (cmd, args) => {
      try {
        return await backend.handle(cmd, (args ?? {}) as Args);
      } catch (error) {
        // The real IPC rejects with the plain `AppError` object.
        // eslint-disable-next-line @typescript-eslint/only-throw-error
        if (error instanceof CommandFailure) throw error.error;
        throw error;
      }
    },
    { shouldMockEvents: true },
  );
  window.__fake = backend.control();
}
