import { SvelteSet } from "svelte/reactivity";

import {
  api,
  CommandError,
  onRepoStatus,
  toAppError,
  type AppError,
  type FileDiff,
  type RepoId,
  type StatusSnapshot,
} from "../api";
import { nextCursor, rangeKeys, toItems, type ChangeItem } from "./changes-model";
import { notices } from "./notices.svelte";

export type DiffState =
  | { status: "idle" }
  | { status: "loading"; key: string }
  | { status: "ready"; key: string; diff: FileDiff }
  | { status: "error"; key: string; error: AppError };

export type SelectMode = "replace" | "toggle" | "range";

/** Status, selection and diff of the repository on screen. */
export class ChangesStore {
  repoId = $state<RepoId | null>(null);
  status = $state<StatusSnapshot | null>(null);
  error = $state<AppError | null>(null);
  cursor = $state<string | null>(null);
  selected = new SvelteSet<string>();
  diff = $state<DiffState>({ status: "idle" });
  /** Number of write operations in flight. */
  busy = $state(0);

  readonly items = $derived(toItems(this.status));
  readonly cursorItem = $derived(this.items.find((item) => item.key === this.cursor) ?? null);
  readonly selectedItems = $derived(this.items.filter((item) => this.selected.has(item.key)));

  private anchor: string | null = null;
  private lastApplied: string | null = null;
  private loadToken = 0;
  private diffToken = 0;

  async init(): Promise<void> {
    await onRepoStatus((update) => {
      if (update.repoId === this.repoId) this.apply(update.status);
    });
  }

  /** Shows the given repository (or nothing). */
  async show(repoId: RepoId | null): Promise<void> {
    if (repoId === this.repoId) return;
    this.repoId = repoId;
    this.status = null;
    this.error = null;
    this.cursor = null;
    this.anchor = null;
    this.lastApplied = null;
    this.selected.clear();
    this.diff = { status: "idle" };
    if (!repoId) return;

    const token = ++this.loadToken;
    try {
      const status = await api.getStatus(repoId);
      if (token === this.loadToken) this.apply(status);
    } catch (error) {
      if (token === this.loadToken) this.error = asAppError(error);
    }
  }

  select(key: string, mode: SelectMode = "replace"): void {
    const item = this.items.find((candidate) => candidate.key === key);
    if (!item) return;
    const sameArea = this.selectedItems.every((selected) => selected.area === item.area);

    if (mode === "toggle" && sameArea) {
      if (this.selected.has(key) && this.selected.size > 1) this.selected.delete(key);
      else this.selected.add(key);
      this.anchor = key;
    } else if (mode === "range" && sameArea && this.anchor) {
      this.selected.clear();
      for (const rangeKey of rangeKeys(this.items, this.anchor, key)) this.selected.add(rangeKey);
    } else {
      this.selected.clear();
      this.selected.add(key);
      this.anchor = key;
    }
    this.setCursor(key);
  }

  /** Moves the cursor by `delta` rows; with `extend` the selection grows. */
  move(delta: number, extend = false): void {
    if (this.items.length === 0) return;
    const index = this.items.findIndex((item) => item.key === this.cursor);
    const next = this.items[Math.max(0, Math.min(this.items.length - 1, index + delta))];
    if (next) this.select(next.key, extend ? "range" : "replace");
  }

  moveTo(position: "first" | "last", extend = false): void {
    const item = position === "first" ? this.items[0] : this.items.at(-1);
    if (item) this.select(item.key, extend ? "range" : "replace");
  }

  selectAllInArea(): void {
    const area = this.cursorItem?.area;
    if (!area) return;
    this.selected.clear();
    for (const item of this.items) if (item.area === area) this.selected.add(item.key);
  }

  /** Stages unstaged selections and unstages staged ones. */
  toggleSelection(): Promise<void> {
    const items = this.actionItems();
    const area = items[0]?.area;
    if (!area) return Promise.resolve();
    const paths = items.map((item) => item.change.path);
    return area === "staged" ? this.unstage(paths) : this.stage(paths);
  }

  stage(paths: string[]): Promise<void> {
    return this.write((repoId) => api.stagePaths(repoId, paths));
  }

  unstage(paths: string[]): Promise<void> {
    return this.write((repoId) => api.unstagePaths(repoId, paths));
  }

  discard(paths: string[]): Promise<void> {
    return this.write((repoId) => api.discardPaths(repoId, paths));
  }

  stageAll(): Promise<void> {
    return this.stage(this.status?.unstaged.map((change) => change.path) ?? []);
  }

  unstageAll(): Promise<void> {
    return this.unstage(this.status?.staged.map((change) => change.path) ?? []);
  }

  /** Shows the status a write command elsewhere (e.g. a commit) returned. */
  receive(repoId: RepoId, status: StatusSnapshot): void {
    if (repoId === this.repoId) this.apply(status);
  }

  /** The selection, or the cursor item if nothing is selected. */
  actionItems(): ChangeItem[] {
    if (this.selectedItems.length > 0) return this.selectedItems;
    return this.cursorItem ? [this.cursorItem] : [];
  }

  /** Loads the diff of the cursor item; `full` ignores the size limits. */
  async loadDiff(full = false): Promise<void> {
    const item = this.cursorItem;
    const repoId = this.repoId;
    if (!item || !repoId) {
      this.diff = { status: "idle" };
      return;
    }
    const token = ++this.diffToken;
    // Keep showing the current diff of the same file while it reloads.
    if (this.diff.status === "idle" || this.diff.key !== item.key) {
      this.diff = { status: "loading", key: item.key };
    }
    try {
      const diff = await api.getDiff(repoId, {
        path: item.change.path,
        origPath: item.change.origPath,
        area: item.area,
        kind: item.change.kind,
        full,
      });
      if (token === this.diffToken) this.diff = { status: "ready", key: item.key, diff };
    } catch (error) {
      if (token === this.diffToken) {
        this.diff = { status: "error", key: item.key, error: asAppError(error) };
      }
    }
  }

  private apply(status: StatusSnapshot): void {
    // The initial load and the watcher can report the same status twice.
    const serialized = JSON.stringify(status);
    if (serialized === this.lastApplied) return;
    this.lastApplied = serialized;
    const before = this.items;
    this.status = status;
    this.error = null;
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a local lookup, not state
    const keys = new Set(this.items.map((item) => item.key));
    for (const key of [...this.selected]) if (!keys.has(key)) this.selected.delete(key);

    const cursor = nextCursor(before, this.items, this.cursor);
    if (cursor !== this.cursor) {
      this.selected.clear();
      if (cursor) this.selected.add(cursor);
      this.anchor = cursor;
    }
    this.cursor = cursor;
    // The file itself may have changed even if the cursor stayed.
    void this.loadDiff();
  }

  private setCursor(key: string): void {
    if (key === this.cursor) return;
    this.cursor = key;
    void this.loadDiff();
  }

  private async write(action: (repoId: RepoId) => Promise<StatusSnapshot>): Promise<void> {
    const repoId = this.repoId;
    if (!repoId) return;
    this.busy++;
    try {
      const status = await action(repoId);
      if (repoId === this.repoId) this.apply(status);
    } catch (error) {
      notices.error(error);
    } finally {
      this.busy--;
    }
  }
}

function asAppError(error: unknown): AppError {
  return error instanceof CommandError ? error.error : toAppError(error);
}

export const changes = new ChangesStore();
