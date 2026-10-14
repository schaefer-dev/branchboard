import { SvelteMap, SvelteSet } from "svelte/reactivity";

import {
  api,
  CommandError,
  onRepoFetch,
  onRepoStatus,
  toAppError,
  type AppError,
  type BranchCommits,
  type BranchList,
  type FetchUpdate,
  type RepoId,
} from "../api";
import { sortBranches, type BranchSort } from "./branches-model";
import { notices } from "./notices.svelte";
import { readPref, writePref } from "./prefs";

const SORTS = ["recent", "name"] as const;

/** Local branches of the repository on screen, and fetch state of all repos. */
export class BranchesStore {
  repoId = $state<RepoId | null>(null);
  list = $state<BranchList | null>(null);
  error = $state<AppError | null>(null);
  cursor = $state<string | null>(null);
  sort = $state<BranchSort>(readPref("branchSort", SORTS, "recent"));
  /** Commits of the branch under the cursor. */
  commits = $state<BranchCommits | null>(null);
  commitsError = $state<AppError | null>(null);
  /** Branch being switched to. */
  switching = $state<string | null>(null);
  fetching = new SvelteSet<RepoId>();
  /** Last background fetch failure per repository. */
  fetchErrors = new SvelteMap<RepoId, AppError>();

  readonly branches = $derived(this.list ? sortBranches(this.list.branches, this.sort) : []);
  readonly current = $derived(this.list?.branches.find((branch) => branch.current) ?? null);

  private token = 0;
  private commitsToken = 0;
  /** The last reported HEAD commit; `undefined` until the first status event. */
  private headOid: string | null | undefined = undefined;

  async init(): Promise<void> {
    await onRepoFetch((update) => {
      this.onFetch(update);
    });
    await onRepoStatus((update) => {
      if (update.repoId !== this.repoId) return;
      // A switch or a new commit (here or in a terminal) changes the list.
      const head = update.status.head;
      const name = head.kind === "detached" ? null : head.name;
      const oid = update.status.headOid;
      // Until the first status the HEAD is unknown; reloading once is cheap.
      const moved = oid !== this.headOid;
      this.headOid = oid;
      if (moved || name !== (this.current?.name ?? null)) void this.reload();
    });
  }

  async show(repoId: RepoId | null): Promise<void> {
    if (repoId === this.repoId) return;
    this.repoId = repoId;
    this.headOid = undefined;
    this.list = null;
    this.error = null;
    this.cursor = null;
    this.commits = null;
    await this.reload();
  }

  async reload(): Promise<void> {
    const repoId = this.repoId;
    if (!repoId) return;
    const token = ++this.token;
    try {
      const list = await api.getBranches(repoId);
      if (token === this.token) this.setList(list);
    } catch (error) {
      if (token === this.token) this.error = asAppError(error);
    }
  }

  setSort(sort: BranchSort): void {
    this.sort = sort;
    writePref("branchSort", sort);
  }

  select(name: string): void {
    if (name === this.cursor) return;
    this.cursor = name;
    void this.loadCommits();
  }

  move(delta: number): void {
    const index = this.branches.findIndex((branch) => branch.name === this.cursor);
    const next = this.branches[Math.max(0, Math.min(this.branches.length - 1, index + delta))];
    if (next) this.select(next.name);
  }

  moveTo(position: "first" | "last"): void {
    const branch = position === "first" ? this.branches[0] : this.branches.at(-1);
    if (branch) this.select(branch.name);
  }

  async loadCommits(): Promise<void> {
    const repoId = this.repoId;
    const name = this.cursor;
    const token = ++this.commitsToken;
    if (!repoId || !name) {
      this.commits = null;
      return;
    }
    try {
      const commits = await api.getBranchCommits(repoId, name);
      if (token === this.commitsToken) {
        this.commits = commits;
        this.commitsError = null;
      }
    } catch (error) {
      if (token === this.commitsToken) this.commitsError = asAppError(error);
    }
  }

  async switchTo(name: string): Promise<void> {
    const repoId = this.repoId;
    if (!repoId || this.switching || name === this.current?.name) return;
    this.switching = name;
    try {
      const list = await api.switchBranch(repoId, name);
      if (repoId === this.repoId) this.setList(list);
    } catch (error) {
      notices.error(error);
    } finally {
      this.switching = null;
    }
  }

  async fetch(): Promise<void> {
    const repoId = this.repoId;
    if (!repoId || this.fetching.has(repoId)) return;
    try {
      const list = await api.fetchRepo(repoId);
      if (repoId === this.repoId) this.setList(list);
    } catch (error) {
      notices.error(error);
    }
  }

  private onFetch(update: FetchUpdate): void {
    const { repoId, state } = update;
    if (state.status === "running") {
      this.fetching.add(repoId);
      return;
    }
    this.fetching.delete(repoId);
    if (state.status === "failed") {
      this.fetchErrors.set(repoId, {
        kind: "gitFailed",
        message: state.message,
        detail: state.detail,
      });
    } else {
      this.fetchErrors.delete(repoId);
    }
    if (repoId === this.repoId) void this.reload();
  }

  private setList(list: BranchList): void {
    this.list = list;
    this.error = null;
    if (!list.branches.some((branch) => branch.name === this.cursor)) {
      this.cursor = this.current?.name ?? this.branches[0]?.name ?? null;
    }
    // Counts may have changed (fetch, switch, new commits).
    void this.loadCommits();
  }
}

function asAppError(error: unknown): AppError {
  return error instanceof CommandError ? error.error : toAppError(error);
}

export const branches = new BranchesStore();
