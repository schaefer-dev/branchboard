<script lang="ts">
  import { comments } from "../../lib/state/comments.svelte";
  import type { ReviewComment } from "../../lib/state/comments-model";
  import { linesLabel } from "./comment-text";

  interface Props {
    comment: ReviewComment;
  }

  let { comment }: Props = $props();

  const sent = $derived(comment.status === "sent");
</script>

<article class="card" class:sent aria-label="Comment on {linesLabel(comment.lines).toLowerCase()}">
  <header class="head">
    <span class="place">{linesLabel(comment.lines)}</span>
    <span class="status">{sent ? "Sent" : "Pending"}</span>
    <span class="spacer"></span>
    <button type="button" class="link" onclick={() => comments.edit(comment.id)}>Edit</button>
    {#if sent}
      <button
        type="button"
        class="link"
        title="The agent took care of it; removes the comment"
        onclick={() => comments.remove(comment.id)}>Resolve</button
      >
    {:else}
      <button type="button" class="link" onclick={() => comments.remove(comment.id)}>Delete</button>
    {/if}
  </header>
  <p class="body selectable">{comment.body}</p>
</article>

<style>
  .card {
    max-width: 720px;
    padding: var(--space-3) var(--space-4);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-surface);
    color: var(--color-text);
    font-family: var(--font-ui);
    font-size: var(--text-md);
    line-height: var(--leading-normal);
    white-space: normal;
  }

  .card.sent {
    color: var(--color-text-muted);
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    font-size: var(--text-sm);
  }

  .place {
    font-weight: 600;
  }

  .status {
    color: var(--color-text-faint);
  }

  .spacer {
    flex: 1;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .link:hover {
    color: var(--color-text);
    text-decoration: underline;
  }

  .body {
    margin: var(--space-2) 0 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
