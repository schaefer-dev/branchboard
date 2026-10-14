<script lang="ts">
  import type { DiffState } from "../../lib/state/changes.svelte";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import DiffView from "./DiffView.svelte";

  interface Props {
    state: DiffState;
    /** Shown when no file is selected. */
    empty: string;
    /** Loads the diff without the size limits. */
    onShowFull: () => void;
  }

  let { state, empty, onShowFull }: Props = $props();

  const diff = $derived(state.status === "ready" ? state.diff : null);
  const formatter = new Intl.NumberFormat();
</script>

<div class="body">
  {#if state.status === "idle"}
    <EmptyState title={empty} />
  {:else if state.status === "error"}
    <EmptyState title="The diff could not be loaded." hint={state.error.message} />
  {:else if diff}
    {#if diff.notes.length > 0}
      <ul class="notes">
        {#each diff.notes as note (note)}
          <li>{note}</li>
        {/each}
      </ul>
    {/if}
    {#if diff.binary}
      <EmptyState title="Binary file" hint="There is no text preview for this file." />
    {:else if diff.tooLarge}
      <div class="large">
        <EmptyState
          title="This diff is large"
          hint="{formatter.format(diff.lineCount)} changed lines. Showing it may take a moment."
        />
        <button type="button" class="btn" onclick={onShowFull}>Show diff</button>
      </div>
    {:else if diff.hunks.length === 0}
      <EmptyState title="No content changes" />
    {:else}
      <div class="view">
        <DiffView {diff} />
      </div>
    {/if}
  {/if}
</div>

<style>
  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }

  .view {
    flex: 1;
    min-height: 0;
  }

  .notes {
    margin: 0;
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--color-border);
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    list-style: none;
  }

  .large {
    padding-bottom: var(--space-4);
  }

  .large .btn {
    margin-left: var(--space-4);
  }
</style>
