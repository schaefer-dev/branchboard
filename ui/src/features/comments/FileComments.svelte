<script lang="ts">
  import { comments, sameTarget } from "../../lib/state/comments.svelte";
  import type { CommentTarget } from "../../lib/state/comments-model";
  import CommentCard from "./CommentCard.svelte";
  import CommentEditor from "./CommentEditor.svelte";

  let { target }: { target: CommentTarget } = $props();
  const items = $derived(comments.forFile(target.repoId, target.path).filter((c) => !c.lines));
  const draft = $derived(comments.draft);
  const editing = $derived(draft && !draft.lines && sameTarget(draft.target, target));
</script>

<div class="comments">
  {#each items as comment (comment.id)}
    {#if comment.id !== draft?.commentId}
      <CommentCard {comment} />
    {/if}
  {/each}
  {#if editing && draft}<CommentEditor {draft} />{/if}
</div>

<style>
  .comments {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    overflow-y: auto;
  }
</style>
