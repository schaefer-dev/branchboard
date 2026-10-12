<script lang="ts">
  import type { Branch } from "../../lib/api";
  import Counts from "./Counts.svelte";

  interface Props {
    branch: Branch;
    mainline: string;
    cursor: boolean;
    switching: boolean;
  }

  let { branch, mainline, cursor, switching }: Props = $props();
</script>

<div
  id="branch-{branch.name}"
  class="branch"
  class:current={branch.current}
  class:cursor
  role="option"
  aria-selected={cursor}
  aria-current={branch.current ? "true" : undefined}
  data-branch={branch.name}
  title={branch.current ? `${branch.name} (current branch)` : branch.name}
>
  <span class="name">{branch.name}</span>
  <span class="col">
    {#if switching}
      <span class="note">Switching…</span>
    {:else if branch.mainline}
      <Counts counts={branch.mainline} relation="than {mainline}" />
    {:else}
      <span class="note">–</span>
    {/if}
  </span>
  <span class="col">
    {#if branch.upstream.kind === "tracking"}
      <Counts counts={branch.upstream.counts} relation="than {branch.upstream.name}" />
    {:else if branch.upstream.kind === "gone"}
      <span class="note" title="{branch.upstream.name} was deleted on the remote">gone</span>
    {:else}
      <span class="note" title="Not pushed to a remote">local</span>
    {/if}
  </span>
</div>

<style>
  .branch {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--count-column) var(--count-column);
    align-items: center;
    gap: var(--space-3);
    height: 100%;
    padding: 0 var(--space-4);
    white-space: nowrap;
  }

  .branch:hover {
    background: var(--color-row-hover);
  }

  .branch.cursor {
    background: var(--color-row-selected);
  }

  :global(.branches:focus-visible) .branch.cursor {
    box-shadow: inset var(--focus-ring);
  }

  .branch.current::before {
    content: "";
    position: absolute;
    inset: 4px auto 4px 0;
    width: 3px;
    border-radius: 0 2px 2px 0;
    background: var(--color-accent);
  }

  .current .name {
    font-weight: 600;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .col {
    display: flex;
    justify-content: flex-end;
    font-size: var(--text-sm);
  }

  .note {
    color: var(--color-text-faint);
  }
</style>
