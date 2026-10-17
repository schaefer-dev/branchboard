import { api, type DiffLine, type RepoId } from "../api";
import { agents } from "./agents.svelte";
import { itemKey } from "./changes-model";
import { changes } from "./changes.svelte";
import {
  byPlace,
  formatReview,
  locate,
  type CommentTarget,
  type LineRange,
  type ReviewComment,
} from "./comments-model";
import { notices } from "./notices.svelte";
import { review } from "./review.svelte";
import { view } from "./view.svelte";

/** A comment being written: a new one, or an existing one being edited. */
export interface CommentDraft {
  target: CommentTarget;
  /** `null` for a comment on the whole file. */
  lines: LineRange | null;
  /** The comment being edited; `null` for a new one. */
  commentId: string | null;
  body: string;
}

export function sameTarget(a: CommentTarget, b: CommentTarget): boolean {
  return a.repoId === b.repoId && a.path === b.path && a.source === b.source;
}

/**
 * Review comments on diff lines, collected per repository until they are
 * handed to an agent. Kept in memory only.
 */
export class CommentsStore {
  items = $state<ReviewComment[]>([]);
  draft = $state<CommentDraft | null>(null);
  /** Asks the diff showing a comment to scroll to it; `seq` makes repeats distinct. */
  revealRequest = $state<{ id: string; seq: number } | null>(null);

  private nextId = 1;

  /** The repository's comments, by file and line. */
  of(repoId: RepoId | null): ReviewComment[] {
    return this.items.filter((comment) => comment.repoId === repoId).sort(byPlace);
  }

  pendingOf(repoId: RepoId | null): ReviewComment[] {
    return this.of(repoId).filter((comment) => comment.status === "pending");
  }

  /** Comments on a file, whichever diff they were written in. */
  forFile(repoId: RepoId, path: string): ReviewComment[] {
    return this.items.filter((comment) => comment.repoId === repoId && comment.path === path);
  }

  start(target: CommentTarget, lines: LineRange | null): void {
    this.draft = { target, lines, commentId: null, body: "" };
  }

  edit(id: string): void {
    const comment = this.items.find((candidate) => candidate.id === id);
    if (!comment) return;
    this.draft = {
      target: { repoId: comment.repoId, path: comment.path, source: comment.source },
      lines: comment.lines,
      commentId: id,
      body: comment.body,
    };
  }

  setDraftBody(body: string): void {
    if (this.draft) this.draft.body = body;
  }

  cancel(): void {
    this.draft = null;
  }

  /** Adds or updates the draft's comment; an edited comment is sent again. */
  save(): void {
    const draft = this.draft;
    if (!draft || draft.body.trim() === "") return;
    if (draft.commentId) {
      const comment = this.items.find((candidate) => candidate.id === draft.commentId);
      if (comment) {
        comment.body = draft.body;
        comment.status = "pending";
      }
    } else {
      this.items.push({
        id: `comment-${String(this.nextId++)}`,
        ...draft.target,
        lines: draft.lines,
        body: draft.body,
        status: "pending",
        outdated: false,
      });
    }
    this.draft = null;
  }

  remove(id: string): void {
    this.items = this.items.filter((comment) => comment.id !== id);
    if (this.draft?.commentId === id) this.draft = null;
  }

  /**
   * Notes which comments written in `target`'s diff no longer find their
   * lines in its current `lines`. Diffs of other sources cannot tell.
   */
  check(target: CommentTarget, lines: readonly DiffLine[]): void {
    for (const comment of this.items) {
      if (!comment.lines || !sameTarget(comment, target)) continue;
      const outdated = locate(lines, comment.lines) === null;
      if (comment.outdated !== outdated) comment.outdated = outdated;
    }
  }

  /** Pastes the pending comments into an agent's input and shows the agent. */
  async pasteTo(repoId: RepoId, paneId: string): Promise<void> {
    const pending = this.pendingOf(repoId);
    if (pending.length === 0) return;
    try {
      await api.pasteToAgent(paneId, formatReview(pending));
    } catch (error) {
      notices.error(error);
      return;
    }
    this.markSent(pending);
    await agents.focus({ paneId, repoId });
  }

  /**
   * Copies the pending comments as one prompt. Call it straight from a
   * click or key press: the clipboard is only writable during one.
   */
  async copy(repoId: RepoId): Promise<void> {
    const pending = this.pendingOf(repoId);
    if (pending.length === 0) return;
    try {
      await navigator.clipboard.writeText(formatReview(pending));
    } catch (error) {
      notices.error(error);
      return;
    }
    this.markSent(pending);
    notices.show(
      "info",
      pending.length === 1 ? "Copied 1 comment" : `Copied ${String(pending.length)} comments`,
    );
  }

  /** Shows the file a comment is on, and the comment in its diff. */
  reveal(comment: ReviewComment): void {
    if (comment.source === "staged" || comment.source === "unstaged") {
      view.focus("changes");
      const key = itemKey(comment.source, comment.path);
      if (changes.items.some((item) => item.key === key)) changes.select(key);
    } else {
      view.focus("branches");
      if (review.files?.files.some((file) => file.path === comment.path)) {
        review.selectFile(comment.path);
      }
    }
    this.revealRequest = { id: comment.id, seq: (this.revealRequest?.seq ?? 0) + 1 };
  }

  private markSent(comments: readonly ReviewComment[]): void {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a local lookup, not state
    const ids = new Set(comments.map((comment) => comment.id));
    for (const comment of this.items) if (ids.has(comment.id)) comment.status = "sent";
  }
}

export const comments = new CommentsStore();
