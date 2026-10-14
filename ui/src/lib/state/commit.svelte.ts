import {
  api,
  CommandError,
  onRepoStatus,
  toAppError,
  type AppError,
  type HeadCommit,
  type RepoId,
} from "../api";
import { changes } from "./changes.svelte";
import {
  composeMessage,
  EMPTY_DRAFT,
  isEmptyDraft,
  parseDraft,
  splitMessage,
  type Draft,
} from "./commit-model";
import { notices } from "./notices.svelte";
import { readText, removePref, writePref } from "./prefs";

const draftKey = (repoId: RepoId) => `commitDraft.${repoId}`;

/** The commit message being written for the repository on screen. */
export class CommitStore {
  repoId = $state<RepoId | null>(null);
  summary = $state("");
  description = $state("");
  /** Replace the last commit instead of adding one. */
  amend = $state(false);
  /** The commit `HEAD` points to; `null` before the first commit. */
  head = $state<HeadCommit | null>(null);
  busy = $state(false);
  /** Why the last commit failed, with git's (or a hook's) output. */
  error = $state<AppError | null>(null);
  /** Asks the summary field to take keyboard focus; counts up per request. */
  focusRequest = $state(0);

  readonly stagedCount = $derived(changes.status?.staged.length ?? 0);
  readonly canCommit = $derived(
    !this.busy &&
      this.summary.trim() !== "" &&
      (this.amend ? this.head !== null : this.stagedCount > 0),
  );
  /** The last commit can be taken back: it has a parent and is not on a remote. */
  readonly canUndo = $derived(
    !this.busy && this.head !== null && this.head.hasParent && !this.head.pushed,
  );

  /** The draft set aside while the form shows the message being amended. */
  private saved: Draft | null = null;
  private headToken = 0;

  async init(): Promise<void> {
    await onRepoStatus((update) => {
      // Commits made elsewhere (e.g. by an agent) move HEAD as well.
      if (update.repoId === this.repoId && update.status.headOid !== (this.head?.oid ?? null)) {
        void this.loadHead();
      }
    });
  }

  async show(repoId: RepoId | null): Promise<void> {
    if (repoId === this.repoId) return;
    this.repoId = repoId;
    this.amend = false;
    this.saved = null;
    this.error = null;
    this.head = null;
    this.setFields(repoId ? parseDraft(readText(draftKey(repoId))) : EMPTY_DRAFT);
    await this.loadHead();
  }

  setSummary(value: string): void {
    this.summary = value;
    this.edited();
  }

  setDescription(value: string): void {
    this.description = value;
    this.edited();
  }

  /** Switches to amending, showing the last commit's message, and back. */
  setAmend(on: boolean): void {
    if (on === this.amend || (on && !this.head)) return;
    this.error = null;
    if (on && this.head) {
      this.saved = { summary: this.summary, description: this.description };
      this.setFields(splitMessage(this.head.message));
    } else {
      this.setFields(this.saved ?? EMPTY_DRAFT);
      this.saved = null;
    }
    this.amend = on;
  }

  focus(): void {
    this.focusRequest += 1;
  }

  async commit(): Promise<void> {
    const repoId = this.repoId;
    if (!repoId || !this.canCommit) return;
    const amend = this.amend;
    this.busy = true;
    this.error = null;
    try {
      const result = await api.commit(repoId, composeMessage(this), amend);
      if (repoId !== this.repoId) return;
      this.head = result.head;
      changes.receive(repoId, result.status);
      this.amend = false;
      this.saved = null;
      this.setFields(EMPTY_DRAFT);
      removePref(draftKey(repoId));
      notices.show("info", `${amend ? "Amended" : "Committed"} ${result.head.shortOid}`);
    } catch (error) {
      if (repoId === this.repoId) this.error = asAppError(error);
    } finally {
      this.busy = false;
    }
  }

  /** Takes back the last commit; its changes stay staged and its message
   * returns to the form unless a draft is already there. */
  async undo(): Promise<void> {
    const repoId = this.repoId;
    const undone = this.head;
    if (!repoId || !undone || !this.canUndo) return;
    this.busy = true;
    try {
      const result = await api.undoLastCommit(repoId);
      if (repoId !== this.repoId) return;
      this.head = result.head;
      changes.receive(repoId, result.status);
      if (this.amend) this.setAmend(false);
      if (isEmptyDraft(this)) {
        this.setFields(splitMessage(result.message));
        this.edited();
      }
      notices.show("info", `Undid ${undone.shortOid}. Its changes are staged.`);
    } catch (error) {
      notices.error(error);
    } finally {
      this.busy = false;
    }
  }

  private async loadHead(): Promise<void> {
    const repoId = this.repoId;
    const token = ++this.headToken;
    if (!repoId) return;
    try {
      const head = await api.getHeadCommit(repoId);
      if (token !== this.headToken) return;
      this.head = head;
      if (!head && this.amend) this.setAmend(false);
    } catch {
      // Without it the form only lacks the last commit line and amend.
      if (token === this.headToken) this.head = null;
    }
  }

  private setFields(draft: Draft): void {
    this.summary = draft.summary;
    this.description = draft.description;
  }

  /** Keeps the draft across repository switches and restarts. */
  private edited(): void {
    this.error = null;
    const repoId = this.repoId;
    if (!repoId || this.amend) return;
    if (isEmptyDraft(this)) removePref(draftKey(repoId));
    else
      writePref(
        draftKey(repoId),
        JSON.stringify({ summary: this.summary, description: this.description }),
      );
  }
}

function asAppError(error: unknown): AppError {
  return error instanceof CommandError ? error.error : toAppError(error);
}

export const commit = new CommitStore();
