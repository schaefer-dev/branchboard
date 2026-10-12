<script lang="ts">
  import type { ChangeItem } from "../../lib/state/changes-model";
  import { splitPath } from "../../lib/state/changes-model";
  import { changeKindLabel, changeKindLetter } from "./change-kind";

  interface Props {
    item: ChangeItem;
    selected: boolean;
    cursor: boolean;
  }

  let { item, selected, cursor }: Props = $props();

  const parts = $derived(splitPath(item.change.path));
  const title = $derived(
    item.change.origPath
      ? `${changeKindLabel[item.change.kind]} from ${item.change.origPath}`
      : `${changeKindLabel[item.change.kind]}: ${item.change.path}`,
  );
</script>

<div
  id="change-{item.key}"
  class="file"
  class:selected
  class:cursor
  role="option"
  aria-selected={selected}
  data-key={item.key}
  {title}
>
  <span class="badge kind-{item.change.kind}">{changeKindLetter[item.change.kind]}</span>
  <span class="name">{parts.name}</span>
  {#if parts.dir}
    <span class="dir">{parts.dir}</span>
  {/if}
</div>

<style>
  .file {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    height: 100%;
    padding: 0 var(--space-4);
    line-height: var(--row-height);
    white-space: nowrap;
  }

  .file:hover {
    background: var(--color-row-hover);
  }

  .file.selected {
    background: var(--color-row-selected);
  }

  :global(.changes:focus-visible) .file.cursor {
    box-shadow: inset var(--focus-ring);
  }

  .badge {
    flex: none;
    width: 1.1em;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    font-weight: 600;
    text-align: center;
  }

  .kind-added,
  .kind-copied {
    color: var(--color-added);
  }

  .kind-modified,
  .kind-renamed,
  .kind-typeChanged {
    color: var(--color-modified);
  }

  .kind-deleted,
  .kind-conflicted {
    color: var(--color-deleted);
  }

  .kind-untracked {
    color: var(--color-text-faint);
  }

  .name {
    flex: none;
    max-width: 70%;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dir {
    min-width: 0;
    overflow: hidden;
    color: var(--color-text-faint);
    font-size: var(--text-sm);
    text-overflow: ellipsis;
  }
</style>
