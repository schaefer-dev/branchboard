<script lang="ts">
  import { changes } from "../../lib/state/changes.svelte";
  import { splitPath } from "../../lib/state/changes-model";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { openChangeInEditor } from "../editor/open-in-editor";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
  import DiffView from "./DiffView.svelte";

  const item = $derived(changes.cursorItem);
  const state = $derived(changes.diff);
  const diff = $derived(state.status === "ready" ? state.diff : null);
  const parts = $derived(item ? splitPath(item.change.path) : null);
  const formatter = new Intl.NumberFormat();
</script>

<section class="diff" aria-label="Diff">
  <PaneHeader>
    {#if item && parts}
      <span class="path selectable" title={item.change.path}>
        {#if parts.dir}<span class="dir">{parts.dir}/</span>{/if}<span class="name"
          >{parts.name}</span
        >
      </span>
      <span class="area">{item.area === "staged" ? "Staged" : "Unstaged"}</span>
      <span class="spacer"></span>
      {#if diff && !diff.binary}
        <span class="stats">
          <span class="additions">+{formatter.format(diff.additions)}</span>
          <span class="deletions">−{formatter.format(diff.deletions)}</span>
        </span>
      {/if}
      <button
        type="button"
        class="icon-btn"
        title="Open in VS Code"
        aria-label="Open in VS Code"
        onclick={() => void openChangeInEditor(item)}
      >
        <Icon name="external" />
      </button>
    {/if}
  </PaneHeader>

  <div class="body">
    {#if !item}
      <EmptyState title="Select a file to see its changes." />
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
          <button type="button" class="btn" onclick={() => void changes.loadDiff(true)}>
            Show diff
          </button>
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
</section>

<style>
  .diff {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--color-surface);
  }

  .path {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dir {
    color: var(--color-text-muted);
  }

  .name {
    font-weight: 600;
  }

  .area {
    flex: none;
    padding: 1px var(--space-3);
    border: 1px solid var(--color-border);
    border-radius: 999px;
    color: var(--color-text-muted);
    font-size: var(--text-xs);
  }

  .spacer {
    flex: 1;
  }

  .stats {
    display: flex;
    flex: none;
    gap: var(--space-3);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  .additions {
    color: var(--color-added);
  }

  .deletions {
    color: var(--color-deleted);
  }

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
