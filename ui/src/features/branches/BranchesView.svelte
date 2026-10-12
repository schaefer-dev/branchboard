<script lang="ts">
  import type { Branch, RepoView } from "../../lib/api";
  import { branches } from "../../lib/state/branches.svelte";
  import { timeAgo } from "../../lib/state/branches-model";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import VirtualList from "../../lib/ui/VirtualList.svelte";
  import BranchRow from "./BranchRow.svelte";
  import { chooseMainline, showBranchMenu, showBranchOptions } from "./branch-actions";

  interface Props {
    repo: RepoView;
  }

  let { repo }: Props = $props();

  const ROW_HEIGHT = 28;

  let list = $state<{ scrollToIndex: (index: number) => void }>();
  /** Re-rendered every minute so "5 min ago" stays true. */
  let now = $state(Date.now() / 1000);

  const data = $derived(branches.list);
  const fetching = $derived(branches.fetching.has(repo.id));
  const fetchError = $derived(branches.fetchErrors.get(repo.id) ?? null);
  const fetchedText = $derived.by(() => {
    if (fetching) return "Fetching…";
    if (fetchError) return "Last fetch failed";
    if (!data) return "";
    return data.lastFetched === null
      ? "Never fetched"
      : `Fetched ${timeAgo(data.lastFetched, now)}`;
  });

  $effect(() => {
    const timer = setInterval(() => {
      now = Date.now() / 1000;
    }, 60_000);
    return () => {
      clearInterval(timer);
    };
  });

  $effect(() => {
    const index = branches.branches.findIndex((branch) => branch.name === branches.cursor);
    if (index !== -1) list?.scrollToIndex(index);
  });

  function branchAt(event: Event): string | undefined {
    const target = event.target instanceof Element ? event.target.closest("[data-branch]") : null;
    return target instanceof HTMLElement ? target.dataset.branch : undefined;
  }

  function onKeydown(event: KeyboardEvent) {
    switch (event.key) {
      case "ArrowDown":
        branches.move(1);
        break;
      case "ArrowUp":
        branches.move(-1);
        break;
      case "Home":
        branches.moveTo("first");
        break;
      case "End":
        branches.moveTo("last");
        break;
      case "Enter":
        if (branches.cursor) void branches.switchTo(branches.cursor);
        break;
      case "ContextMenu":
        if (branches.cursor) void showBranchMenu(branches.cursor);
        break;
      default:
        return;
    }
    event.preventDefault();
  }
</script>

<div class="board">
  <div class="toolbar">
    <button
      type="button"
      class="mainline"
      title="Branches are compared with this branch. Click to change."
      onclick={() => void chooseMainline(repo)}
    >
      <span class="label">Compared with</span>
      <span class="ref">{repo.mainline}</span>
    </button>
    <button
      type="button"
      class="icon-btn"
      class:spinning={fetching}
      title="Fetch all remotes"
      aria-label="Fetch"
      disabled={fetching}
      onclick={() => void branches.fetch()}
    >
      <Icon name="fetch" />
    </button>
    <button
      type="button"
      class="icon-btn"
      title="Branch options"
      aria-label="Branch options"
      onclick={() => void showBranchOptions(repo)}
    >
      <Icon name="more" />
    </button>
  </div>
  <div class="meta">
    <span
      class="fetched"
      class:failed={fetchError !== null}
      title={fetchError?.detail ?? undefined}
    >
      {fetchedText}
    </span>
    <div class="sort" role="group" aria-label="Sort branches">
      <button
        type="button"
        aria-pressed={branches.sort === "recent"}
        onclick={() => {
          branches.setSort("recent");
        }}>Recent</button
      >
      <button
        type="button"
        aria-pressed={branches.sort === "name"}
        onclick={() => {
          branches.setSort("name");
        }}>A–Z</button
      >
    </div>
  </div>

  {#if branches.error}
    <EmptyState title={branches.error.message} hint={branches.error.detail ?? undefined} />
  {:else if data}
    {#if !data.mainlineExists}
      <div class="missing" role="status">
        <p>
          <strong>{data.mainline}</strong> doesn't exist. Fetch, or compare with another branch.
        </p>
      </div>
    {/if}
    <div class="columns" aria-hidden="true">
      <span>Branch</span>
      <span title="Commits ahead/behind {data.mainline}">Mainline</span>
      <span title="Commits ahead/behind the branch's upstream">Upstream</span>
    </div>
    <VirtualList
      bind:this={list}
      class="branches"
      items={branches.branches}
      itemHeight={ROW_HEIGHT}
      key={(branch: Branch) => branch.name}
      role="listbox"
      aria-label="Branches"
      aria-activedescendant={branches.cursor ? `branch-${branches.cursor}` : undefined}
      tabindex={0}
      onclick={(event: MouseEvent) => {
        const name = branchAt(event);
        if (name) branches.cursor = name;
      }}
      ondblclick={(event: MouseEvent) => {
        const name = branchAt(event);
        if (name) void branches.switchTo(name);
      }}
      oncontextmenu={(event: MouseEvent) => {
        const name = branchAt(event);
        if (!name) return;
        event.preventDefault();
        branches.cursor = name;
        void showBranchMenu(name);
      }}
      onkeydown={onKeydown}
    >
      {#snippet row(branch: Branch)}
        <BranchRow
          {branch}
          mainline={data.mainline}
          cursor={branch.name === branches.cursor}
          switching={branch.name === branches.switching}
        />
      {/snippet}
    </VirtualList>
  {/if}
</div>

<style>
  .board {
    --count-column: 76px;

    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-3) 0 var(--space-4);
  }

  .mainline {
    display: flex;
    flex: 1;
    align-items: baseline;
    gap: var(--space-2);
    min-width: 0;
    padding: 2px var(--space-2);
    margin-left: calc(var(--space-2) * -1);
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    text-align: left;
  }

  .mainline:hover {
    background: var(--color-row-hover);
  }

  .label {
    flex: none;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .ref {
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .icon-btn:disabled {
    opacity: 0.6;
  }

  .spinning :global(svg) {
    animation: bob 0.9s ease-in-out infinite alternate;
  }

  @keyframes bob {
    to {
      transform: translateY(2px);
    }
  }

  .meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-4) var(--space-3);
    border-bottom: 1px solid var(--color-border);
    font-size: var(--text-sm);
  }

  .fetched {
    color: var(--color-text-faint);
  }

  .fetched.failed {
    color: var(--color-danger);
  }

  .sort {
    display: flex;
    padding: 1px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
  }

  .sort button {
    padding: 0 var(--space-3);
    border: 0;
    border-radius: 3px;
    background: none;
    color: var(--color-text-muted);
    font-size: var(--text-xs);
    line-height: 18px;
  }

  .sort button[aria-pressed="true"] {
    background: var(--color-row-selected);
    color: var(--color-text);
  }

  .missing {
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-accent-soft);
    font-size: var(--text-sm);
  }

  .missing p {
    margin: 0;
  }

  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--count-column) var(--count-column);
    gap: var(--space-3);
    padding: var(--space-2) var(--space-4);
    color: var(--color-text-faint);
    font-size: var(--text-xs);
  }

  .columns span:not(:first-child) {
    text-align: right;
  }

  :global(.branches) {
    flex: 1;
    min-height: 0;
    outline: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .spinning :global(svg) {
      animation: none;
    }
  }
</style>
