<script lang="ts" generics="T">
  import type { Snippet } from "svelte";

  import { fuzzyFilter } from "../fuzzy";

  interface Props {
    open: boolean;
    /** Accessible name of the dialog. */
    label: string;
    placeholder: string;
    items: readonly T[];
    key: (item: T) => string;
    /** Texts the query is matched against; the best match wins. */
    fields: (item: T) => string[];
    choose: (item: T) => void;
    row: Snippet<[T]>;
    emptyText: string;
  }

  let {
    open = $bindable(),
    label,
    placeholder,
    items,
    key,
    fields,
    choose,
    row,
    emptyText,
  }: Props = $props();

  const MAX_RESULTS = 50;
  const id = `picker-${Math.random().toString(36).slice(2, 8)}`;

  let dialog = $state<HTMLDialogElement>();
  let query = $state("");
  let selected = $state(0);

  const results = $derived(fuzzyFilter(items, query, fields).slice(0, MAX_RESULTS));
  const activeId = $derived.by(() => {
    const item = results[selected];
    return item === undefined ? undefined : `${id}-${key(item)}`;
  });

  $effect(() => {
    if (open && dialog && !dialog.open) {
      query = "";
      selected = 0;
      dialog.showModal();
    } else if (!open && dialog?.open) {
      dialog.close();
    }
  });

  function pick(index: number) {
    const item = results[index];
    if (item === undefined) return;
    open = false;
    choose(item);
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown") {
      selected = Math.min(selected + 1, results.length - 1);
    } else if (event.key === "ArrowUp") {
      selected = Math.max(selected - 1, 0);
    } else if (event.key === "Enter") {
      pick(selected);
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

<dialog
  bind:this={dialog}
  class="modal picker"
  aria-label={label}
  onclose={() => (open = false)}
  onclick={(event) => {
    if (event.target === event.currentTarget) open = false;
  }}
>
  {#if open}
    <input
      class="search"
      bind:value={query}
      oninput={() => (selected = 0)}
      onkeydown={onKeydown}
      {placeholder}
      aria-controls="{id}-results"
      aria-activedescendant={activeId}
      autofocus
      spellcheck="false"
      autocomplete="off"
    />
    <ul id="{id}-results" role="listbox" aria-label={label}>
      {#each results as item, index (key(item))}
        <!-- Keyboard selection happens in the search field. -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          id="{id}-{key(item)}"
          role="option"
          aria-selected={index === selected}
          class:selected={index === selected}
          onmousemove={() => (selected = index)}
          onclick={() => {
            pick(index);
          }}
        >
          {@render row(item)}
        </li>
      {:else}
        <li class="none">{emptyText}</li>
      {/each}
    </ul>
  {/if}
</dialog>

<style>
  .picker {
    width: min(560px, calc(100vw - 48px));
    margin-top: 12vh;
  }

  .search {
    width: 100%;
    height: 44px;
    padding: 0 var(--space-5);
    border: 0;
    border-bottom: 1px solid var(--color-border);
    background: none;
    color: var(--color-text);
    font: inherit;
    font-size: var(--text-lg);
    outline: none;
  }

  /* The open dialog already shows where focus is. */
  .search:focus-visible {
    box-shadow: none;
  }

  ul {
    max-height: 360px;
    margin: 0;
    padding: var(--space-2) 0;
    overflow-y: auto;
    list-style: none;
  }

  li {
    padding: var(--space-2) var(--space-5);
    line-height: var(--leading-tight);
  }

  li.selected {
    background: var(--color-row-selected);
  }

  .none {
    color: var(--color-text-muted);
  }
</style>
