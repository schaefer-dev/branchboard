import { SvelteMap } from "svelte/reactivity";
import {
  api,
  CommandError,
  onRepoRefs,
  onRepoFetch,
  onRepoStatus,
  toAppError,
  type AppError,
  type CommitSummary,
  type HistoryCursor,
  type RepoId,
} from "../api";
import { ReviewStore } from "./review.svelte";

/** Explicit pages bound retained summaries to ~10k rows per repository. Search
 * still scans the complete history; narrow the query to browse beyond the cap. */
export const HISTORY_ROW_LIMIT = 10_000;

export class HistorySession {
  reference = $state("HEAD");
  query = $state("");
  commits = $state<CommitSummary[]>([]);
  tip = $state<string | null>(null);
  cursor = $state<HistoryCursor | null>(null);
  loading = $state(false);
  error = $state<AppError | null>(null);
  stale = $state(false);
  initialized = $state(false);
  scrollTop = $state(0);
  commitsHeight = $state(220);
  filesHeight = $state(130);
  localBranches = $state<string[]>([]);
  private branchesToken = 0;
  readonly review = new ReviewStore();
  private token = 0;
  private freshnessToken = 0;
  private freshnessPending = false;
  private failedRequest: { more: boolean; refresh: boolean } | null = null;
  private timer: ReturnType<typeof setTimeout> | undefined;

  readonly repoId: RepoId;
  constructor(repoId: RepoId) {
    this.repoId = repoId;
    this.review.repoId = repoId;
  }

  async load(more = false, refresh = false): Promise<void> {
    clearTimeout(this.timer);
    if (more && (!this.cursor || this.loading || this.commits.length >= HISTORY_ROW_LIMIT)) return;
    const token = ++this.token;
    ++this.freshnessToken;
    const selected = this.review.target;
    this.loading = true;
    this.error = null;
    this.failedRequest = null;
    try {
      const page = await api.getHistory(this.repoId, {
        reference: this.reference,
        query: this.query,
        tip: refresh ? null : this.tip,
        cursor: more ? this.cursor : null,
        limit: 100,
      });
      if (token !== this.token) return;
      this.commits = more ? [...this.commits, ...page.commits] : page.commits;
      this.tip = page.tip;
      this.cursor = page.cursor;
      this.initialized = true;
      if (!more) {
        this.scrollTop = 0;
        if (refresh) this.stale = false;
        const retained =
          selected?.kind === "commit" && page.commits.some((c) => c.oid === selected.oid);
        const oid = retained ? selected.oid : page.commits[0]?.oid;
        // A comment may have revealed an outside-list commit during this load.
        if (this.review.target === selected) {
          if (oid) this.select(oid);
          else {
            this.review.target = null;
            await this.review.loadFiles();
          }
        }
      }
    } catch (error) {
      if (token === this.token) {
        this.error = error instanceof CommandError ? error.error : toAppError(error);
        this.failedRequest = { more, refresh };
      }
    } finally {
      if (token === this.token) {
        this.loading = false;
        if (this.freshnessPending) {
          this.freshnessPending = false;
          void this.checkFreshness();
        }
      }
    }
  }

  retry(): Promise<void> {
    return this.load(this.failedRequest?.more ?? false, this.failedRequest?.refresh ?? false);
  }

  setReference(reference: string): void {
    if (reference === this.reference) return;
    this.reference = reference;
    this.reset(true);
    void this.load(false, true);
  }

  setQuery(query: string): void {
    this.query = query;
    this.reset(false);
    this.loading = true;
    this.timer = setTimeout(() => void this.load(), 250);
  }

  private reset(ref: boolean): void {
    ++this.token;
    ++this.freshnessToken;
    clearTimeout(this.timer);
    this.commits = [];
    this.cursor = null;
    this.error = null;
    this.loading = false;
    this.scrollTop = 0;
    if (ref) {
      this.tip = null;
      this.stale = false;
    }
    this.review.target = null;
    void this.review.loadFiles();
  }

  select(oid: string): void {
    this.review.select({ kind: "commit", oid });
  }
  move(delta: number): number {
    const target = this.review.target;
    const index = this.commits.findIndex((c) => target?.kind === "commit" && c.oid === target.oid);
    const next = Math.max(0, Math.min(this.commits.length - 1, index + delta));
    const commit = this.commits[next];
    if (commit) this.select(commit.oid);
    return next;
  }

  /** An explicit outside-list selection: comments can reveal a commit excluded
   * by the current query/ref/page without altering the browsing snapshot. */
  reveal(oid: string, path: string): void {
    this.review.reveal({ kind: "commit", oid }, path);
  }

  reloadBranches(): void {
    const branchesToken = ++this.branchesToken;
    void api
      .getBranches(this.repoId)
      .then((list) => {
        if (branchesToken === this.branchesToken)
          this.localBranches = list.branches.map((branch) => branch.name).sort();
      })
      .catch(() => {
        /* The history request reports repository errors. */
      });
  }

  async checkFreshness(): Promise<void> {
    if (this.loading) {
      this.freshnessPending = true;
      return;
    }
    if (!this.initialized) return;
    const token = ++this.freshnessToken;
    try {
      const tip = await api.getHistoryTip(this.repoId, this.reference);
      if (token === this.freshnessToken) this.stale = tip !== this.tip;
    } catch {
      if (token === this.freshnessToken) this.stale = true;
    }
  }
}

export class HistoryStore {
  current = $state.raw<HistorySession | null>(null);
  private sessions = new SvelteMap<RepoId, HistorySession>();
  private visible = false;

  async init(): Promise<void> {
    await onRepoRefs((id) => {
      if (this.visible && id === this.current?.repoId) {
        this.current.reloadBranches();
        void this.current.checkFreshness();
      }
    });
    await onRepoStatus((update) => {
      if (this.visible && update.repoId === this.current?.repoId)
        void this.current.checkFreshness();
    });
    await onRepoFetch((update) => {
      if (this.visible && update.repoId === this.current?.repoId)
        void this.current.checkFreshness();
    });
  }

  show(repoId: RepoId | null, visible: boolean): void {
    this.visible = visible;
    if (!repoId) {
      this.current = null;
      return;
    }
    let session = this.sessions.get(repoId);
    if (!session) {
      session = new HistorySession(repoId);
      this.sessions.set(repoId, session);
    }
    this.current = session;
    if (visible) {
      session.reloadBranches();
      void session.checkFreshness();
      if (!session.initialized && !session.loading) void session.load(false, true);
    }
  }

  checkFreshness(): void {
    if (this.visible) void this.current?.checkFreshness();
  }
}

export const history = new HistoryStore();
