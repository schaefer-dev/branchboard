<script lang="ts">
  import type { ChangeKind } from "../../lib/api";
  import { splitPath } from "../../lib/state/changes-model";
  import { changeKindLabel, changeKindLetter } from "./change-kind";

  interface Props {
    /** Element id, for `aria-activedescendant`. */
    id: string;
    /** Identifies the row in its list (`data-key`). */
    key: string;
    path: string;
    origPath: string | null;
    kind: ChangeKind;
    selected: boolean;
    cursor: boolean;
    /** Line counts shown at the end of the row; `null` hides them (e.g. binary files). */
    additions?: number | null;
    deletions?: number | null;
  }

  let {
    id,
    key,
    path,
    origPath,
    kind,
    selected,
    cursor,
    additions = null,
    deletions = null,
  }: Props = $props();

  const parts = $derived(splitPath(path));
  const title = $derived(
    origPath ? `${changeKindLabel[kind]} from ${origPath}` : `${changeKindLabel[kind]}: ${path}`,
  );
</script>

<div
  {id}
  class="file"
  class:selected
  class:cursor
  role="option"
  aria-selected={selected}
  data-key={key}
  {title}
>
  <span class="badge kind-{kind}">{changeKindLetter[kind]}</span>
  <span class="name">{parts.name}</span>
  {#if parts.dir}
    <span class="dir">{parts.dir}</span>
  {/if}
  {#if additions !== null && deletions !== null}
    <span class="stats">
      <span class="additions">+{additions}</span>
      <span class="deletions">−{deletions}</span>
    </span>
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

  :global([role="listbox"]:focus-visible) .file.cursor {
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

  .stats {
    display: flex;
    flex: none;
    gap: var(--space-2);
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .additions {
    color: var(--color-added);
  }

  .deletions {
    color: var(--color-deleted);
  }

  .dir {
    min-width: 0;
    overflow: hidden;
    color: var(--color-text-faint);
    font-size: var(--text-sm);
    text-overflow: ellipsis;
  }
</style>
