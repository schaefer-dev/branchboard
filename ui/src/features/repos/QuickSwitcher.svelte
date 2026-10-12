<script lang="ts">
  import { fuzzyFilter } from "../../lib/fuzzy";
  import { workspace } from "../../lib/state/workspace.svelte";
  import { summaryText } from "./summary-text";

  interface Props {
    open: boolean;
  }

  let { open = $bindable() }: Props = $props();

  const MAX_RESULTS = 50;

  let dialog = $state<HTMLDialogElement>();
  let query = $state("");
  let selected = $state(0);

  const activeId = $derived.by(() => {
    const repo = results[selected];
    return repo ? `switcher-${repo.id}` : undefined;
  });

  const results = $derived(
    fuzzyFilter(workspace.snapshot.repos, query, (repo) => [repo.name, repo.path]).slice(
      0,
      MAX_RESULTS,
    ),
  );

  $effect(() => {
    if (open && dialog && !dialog.open) {
      query = "";
      selected = 0;
      dialog.showModal();
    } else if (!open && dialog?.open) {
      dialog.close();
    }
  });

  function choose(index: number) {
    const repo = results[index];
    if (!repo) return;
    open = false;
    void workspace.activate(repo.id);
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown") {
      selected = Math.min(selected + 1, results.length - 1);
    } else if (event.key === "ArrowUp") {
      selected = Math.max(selected - 1, 0);
    } else if (event.key === "Enter") {
      choose(selected);
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

<dialog
  bind:this={dialog}
  class="modal switcher"
  aria-label="Switch repository"
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
      placeholder="Switch to repository…"
      aria-controls="switcher-results"
      aria-activedescendant={activeId}
      autofocus
      spellcheck="false"
      autocomplete="off"
    />
    <ul id="switcher-results" role="listbox">
      {#each results as repo, index (repo.id)}
        <!-- Keyboard selection happens in the search field. -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          id={`switcher-${repo.id}`}
          role="option"
          aria-selected={index === selected}
          class:selected={index === selected}
          onmousemove={() => (selected = index)}
          onclick={() => {
            choose(index);
          }}
        >
          <span class="name">{repo.name}</span>
          <span class="branch">{summaryText(workspace.summaries.get(repo.id))}</span>
          <span class="path">{repo.path}</span>
        </li>
      {:else}
        <li class="none">No matching repositories</li>
      {/each}
    </ul>
  {/if}
</dialog>

<style>
  .switcher {
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
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas:
      "name branch"
      "path path";
    column-gap: var(--space-4);
    padding: var(--space-2) var(--space-5);
    line-height: var(--leading-tight);
  }

  li.selected {
    background: var(--color-row-selected);
  }

  .name {
    grid-area: name;
    font-weight: 500;
  }

  .branch {
    grid-area: branch;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .path {
    grid-area: path;
    overflow: hidden;
    color: var(--color-text-faint);
    font-size: var(--text-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .none {
    display: block;
    color: var(--color-text-muted);
  }
</style>
