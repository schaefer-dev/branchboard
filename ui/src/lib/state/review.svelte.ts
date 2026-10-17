import {
  api,
  CommandError,
  toAppError,
  type AppError,
  type BranchCommits,
  type RepoId,
  type ReviewFiles,
  type ReviewTarget,
} from "../api";
import type { DiffState } from "./changes.svelte";
import { chooseTarget, reviewTargets, targetKey } from "./review-model";

/** The commits, files and diff of the branch under the cursor in the branch list. */
export class ReviewStore {
  repoId = $state<RepoId | null>(null);
  /** Commits of the branch, as the branch list loaded them. */
  commits = $state<BranchCommits | null>(null);
  target = $state<ReviewTarget | null>(null);
  files = $state<ReviewFiles | null>(null);
  filesError = $state<AppError | null>(null);
  /** Path of the file whose diff is shown. */
  fileCursor = $state<string | null>(null);
  diff = $state<DiffState>({ status: "idle" });

  // Getters deliberately have no effect-owner lifetime: independent instances
  // may be created from a component effect and retained in the History cache.
  get targets() {
    return this.commits ? reviewTargets(this.commits) : [];
  }
  get targetKey() {
    return this.target ? targetKey(this.target) : null;
  }
  get cursorFile() {
    return this.files?.files.find((file) => file.path === this.fileCursor) ?? null;
  }

  private filesToken = 0;
  private diffToken = 0;
  /** A comment requested a target/file, possibly while the branch reloads. */
  private requested: { target: ReviewTarget; path: string } | null = null;

  /** Follows the branch list: the repository and the commits of its cursor branch. */
  follow(repoId: RepoId | null, commits: BranchCommits | null): void {
    if (repoId !== this.repoId || commits?.branch !== this.commits?.branch) {
      this.repoId = repoId;
      this.target = null;
      this.files = null;
      this.fileCursor = null;
      this.diff = { status: "idle" };
    }
    this.commits = commits;
    // The commits reload after fetches and new commits; the files may have changed too.
    this.target = commits ? chooseTarget(commits, this.requested?.target ?? this.target) : null;
    void this.loadFiles();
  }

  select(target: ReviewTarget): void {
    this.requested = null;
    if (targetKey(target) === this.targetKey) return;
    this.target = target;
    void this.loadFiles();
  }

  reveal(target: ReviewTarget, path: string, branch?: string): void {
    this.requested = { target, path };
    // Selecting a different branch loads its commits asynchronously. Let
    // `follow` apply the request after that load, rather than consuming it
    // now and then resetting the file cursor when the branch arrives.
    if (branch && this.commits?.branch !== branch) return;
    this.target = target;
    void this.loadFiles();
  }

  moveTarget(delta: number): void {
    const index = this.targets.findIndex((target) => targetKey(target) === this.targetKey);
    const next = this.targets[Math.max(0, Math.min(this.targets.length - 1, index + delta))];
    if (next) this.select(next);
  }

  selectFile(path: string): void {
    if (path === this.fileCursor) return;
    this.fileCursor = path;
    void this.loadDiff();
  }

  moveFile(delta: number): void {
    const files = this.files?.files ?? [];
    const index = files.findIndex((file) => file.path === this.fileCursor);
    const next = files[Math.max(0, Math.min(files.length - 1, index + delta))];
    if (next) this.selectFile(next.path);
  }

  async loadFiles(): Promise<void> {
    const repoId = this.repoId;
    const target = this.target;
    const token = ++this.filesToken;
    ++this.diffToken;
    this.filesError = null;
    this.files = null;
    this.diff = { status: "idle" };
    if (!repoId || !target) {
      this.files = null;
      this.diff = { status: "idle" };
      return;
    }
    try {
      const files = await api.getReviewFiles(repoId, target);
      if (token !== this.filesToken) return;
      this.files = files;
      this.filesError = null;
      if (this.requested && targetKey(this.requested.target) === targetKey(target)) {
        this.fileCursor = this.requested.path;
        this.requested = null;
      }
      if (!files.files.some((file) => file.path === this.fileCursor)) {
        this.fileCursor = files.files[0]?.path ?? null;
      }
      void this.loadDiff();
    } catch (error) {
      if (token !== this.filesToken) return;
      this.files = null;
      this.filesError = asAppError(error);
      this.diff = { status: "idle" };
    }
  }

  /** Loads the diff of the file under the cursor; `full` ignores the size limits. */
  async loadDiff(full = false): Promise<void> {
    const repoId = this.repoId;
    const range = this.files?.range;
    const file = this.cursorFile;
    const token = ++this.diffToken;
    if (!repoId || !range || !file) {
      this.diff = { status: "idle" };
      return;
    }
    const key = `${range.from}..${range.to}:${file.path}`;
    if (this.diff.status === "idle" || this.diff.key !== key) {
      this.diff = { status: "loading", key };
    }
    try {
      const diff = await api.getReviewDiff(repoId, {
        range,
        path: file.path,
        origPath: file.origPath,
        full,
      });
      if (token === this.diffToken) this.diff = { status: "ready", key, diff };
    } catch (error) {
      if (token === this.diffToken) this.diff = { status: "error", key, error: asAppError(error) };
    }
  }
}

function asAppError(error: unknown): AppError {
  return error instanceof CommandError ? error.error : toAppError(error);
}

export const review = new ReviewStore();
