<script lang="ts" module>
  /** Drafts whose field took focus once; a row scrolled back into view keeps it where it is. */
  const focused = new WeakSet<object>();
</script>

<script lang="ts">
  import type { Attachment } from "svelte/attachments";

  import { isMac } from "../../lib/platform/os";
  import { comments, type CommentDraft } from "../../lib/state/comments.svelte";
  import { linesLabel } from "./comment-text";

  interface Props {
    draft: CommentDraft;
  }

  let { draft }: Props = $props();

  const shortcut = isMac ? "⌘↩" : "Ctrl+Enter";

  const focusOnce: Attachment<HTMLTextAreaElement> = (field) => {
    if (focused.has(draft)) return;
    focused.add(draft);
    field.focus();
  };

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      comments.cancel();
    } else if (event.key === "Enter" && (isMac ? event.metaKey : event.ctrlKey)) {
      comments.save();
    } else {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<form
  class="editor"
  aria-label="Write a comment"
  onkeydown={onKeydown}
  onsubmit={(event) => {
    event.preventDefault();
    comments.save();
  }}
>
  <div class="place">{linesLabel(draft.lines)}</div>
  <textarea
    class="field"
    rows="3"
    placeholder="What should the agent change?"
    aria-label="Comment"
    spellcheck="true"
    value={draft.body}
    oninput={(event) => {
      comments.setDraftBody(event.currentTarget.value);
    }}
    {@attach focusOnce}></textarea>
  <div class="actions">
    <span class="hint">{shortcut} to save</span>
    <button type="button" class="btn" onclick={() => comments.cancel()}>Cancel</button>
    <button type="submit" class="btn btn-primary" disabled={draft.body.trim() === ""}>
      {draft.commentId ? "Save" : "Add comment"}
    </button>
  </div>
</form>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-width: 720px;
    padding: var(--space-3) var(--space-4);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-chrome);
    font-family: var(--font-ui);
    font-size: var(--text-md);
    white-space: normal;
  }

  .place {
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-3);
  }

  .hint {
    margin-right: auto;
    color: var(--color-text-faint);
    font-size: var(--text-sm);
  }
</style>
