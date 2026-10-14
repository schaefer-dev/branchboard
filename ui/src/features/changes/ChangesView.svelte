<script lang="ts">
  import type { DiffArea } from "../../lib/api";
  import { isMac } from "../../lib/platform/os";
  import { changes } from "../../lib/state/changes.svelte";
  import type { ChangeItem } from "../../lib/state/changes-model";
  import { view } from "../../lib/state/view.svelte";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import VirtualList from "../../lib/ui/VirtualList.svelte";
  import FileRow from "./FileRow.svelte";
  import { discardItems, showChangeMenu } from "./change-actions";

  type Row =
    | { kind: "header"; key: string; area: DiffArea; count: number }
    | { kind: "file"; key: string; item: ChangeItem };

  const ROW_HEIGHT = 26;

  let list = $state<{ scrollToIndex: (index: number) => void; focus: () => void }>();

  const status = $derived(changes.status);
  const rows = $derived.by((): Row[] => {
    if (!status) return [];
    const section = (area: DiffArea, count: number): Row[] => [
      { kind: "header", key: `header:${area}`, area, count },
      ...changes.items
        .filter((item) => item.area === area)
        .map((item): Row => ({ kind: "file", key: item.key, item })),
    ];
    return [
      ...section("staged", status.staged.length),
      ...section("unstaged", status.unstaged.length),
    ];
  });
  const clean = $derived(status !== null && changes.items.length === 0);

  $effect(() => {
    const index = rows.findIndex((row) => row.key === changes.cursor);
    if (index !== -1) list?.scrollToIndex(index);
  });

  $effect(() => {
    if (view.focusRequest?.list === "changes") list?.focus();
  });

  function modeFor(event: MouseEvent | KeyboardEvent) {
    if (event.shiftKey) return "range" as const;
    if (isMac ? event.metaKey : event.ctrlKey) return "toggle" as const;
    return "replace" as const;
  }

  function rowKey(event: Event): string | undefined {
    const target = event.target instanceof Element ? event.target.closest("[data-key]") : null;
    return target instanceof HTMLElement ? target.dataset.key : undefined;
  }

  function onClick(event: MouseEvent) {
    const key = rowKey(event);
    if (key) changes.select(key, modeFor(event));
  }

  function onDoubleClick(event: MouseEvent) {
    const key = rowKey(event);
    if (!key) return;
    changes.select(key);
    void changes.toggleSelection();
  }

  function onContextMenu(event: MouseEvent) {
    const key = rowKey(event);
    if (!key) return;
    event.preventDefault();
    if (!changes.selected.has(key)) changes.select(key);
    void showChangeMenu(changes.actionItems());
  }

  function onKeydown(event: KeyboardEvent) {
    const mod = isMac ? event.metaKey : event.ctrlKey;
    switch (event.key) {
      case "ArrowDown":
        changes.move(1, event.shiftKey);
        break;
      case "ArrowUp":
        changes.move(-1, event.shiftKey);
        break;
      case "Home":
        changes.moveTo("first", event.shiftKey);
        break;
      case "End":
        changes.moveTo("last", event.shiftKey);
        break;
      case " ":
      case "Enter":
        void changes.toggleSelection();
        break;
      case "Delete":
      case "Backspace":
        void discardItems(changes.actionItems());
        break;
      case "a":
        if (!mod) return;
        changes.selectAllInArea();
        break;
      case "ContextMenu":
        void showChangeMenu(changes.actionItems());
        break;
      default:
        return;
    }
    event.preventDefault();
  }
</script>

{#if changes.error}
  <EmptyState title={changes.error.message} hint={changes.error.detail ?? undefined} />
{:else if clean}
  <EmptyState title="No changes" hint="The working tree matches the last commit." />
{:else if status}
  <VirtualList
    bind:this={list}
    class="changes"
    items={rows}
    itemHeight={ROW_HEIGHT}
    key={(row: Row) => row.key}
    role="listbox"
    aria-label="Changed files"
    aria-multiselectable="true"
    aria-activedescendant={changes.cursor ? `change-${changes.cursor}` : undefined}
    tabindex={0}
    onclick={onClick}
    ondblclick={onDoubleClick}
    oncontextmenu={onContextMenu}
    onkeydown={onKeydown}
  >
    {#snippet row(row: Row)}
      {#if row.kind === "header"}
        <div class="header" class:first={row.area === "staged"} role="presentation">
          <span class="title">{row.area === "staged" ? "Staged" : "Changes"}</span>
          <span class="count">{row.count}</span>
          <span class="spacer"></span>
          {#if row.count > 0}
            <button
              type="button"
              class="link"
              tabindex={-1}
              onclick={(event) => {
                event.stopPropagation();
                void (row.area === "staged" ? changes.unstageAll() : changes.stageAll());
              }}
            >
              {row.area === "staged" ? "Unstage all" : "Stage all"}
            </button>
          {/if}
        </div>
      {:else}
        <FileRow
          item={row.item}
          selected={changes.selected.has(row.key)}
          cursor={changes.cursor === row.key}
        />
      {/if}
    {/snippet}
  </VirtualList>
{/if}

<style>
  :global(.changes) {
    flex: 1;
    min-height: 0;
    outline: none;
  }

  .header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: 100%;
    padding: 0 var(--space-4);
    border-top: 1px solid var(--color-border);
    background: var(--color-chrome);
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .header.first {
    border-top: 0;
  }

  .count {
    color: var(--color-text-faint);
    font-weight: 400;
    font-variant-numeric: tabular-nums;
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
    font-weight: 500;
  }

  .link:hover {
    color: var(--color-text);
    text-decoration: underline;
  }
</style>
