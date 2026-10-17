<script lang="ts">
  import { commands } from "../../lib/commands";
  import { agents } from "../../lib/state/agents.svelte";
  import { agentName, agentPlace, describeAgentRow } from "./agent-text";
  import AgentStateIcon from "./AgentStateIcon.svelte";

  let list = $state<HTMLElement>();
  /** Roving tab stop: the last focused agent, else the first one. */
  let focusKey = $state<string | null>(null);

  const keys = $derived(agents.list.map((agent) => agent.paneId));
  const waiting = $derived(agents.needYou.length);
  const nextShortcut = commands.shortcutLabel("agents.next");
  const tabStop = $derived(focusKey && keys.includes(focusKey) ? focusKey : (keys[0] ?? null));

  function focusRow(key: string | undefined) {
    if (!key || !list) return;
    list.querySelector<HTMLElement>(`[data-pane-id="${CSS.escape(key)}"]`)?.focus();
  }

  function onKeydown(event: KeyboardEvent) {
    const key = event.target instanceof HTMLElement ? event.target.dataset.paneId : undefined;
    if (!key) return;
    const index = keys.indexOf(key);
    switch (event.key) {
      case "ArrowDown":
        focusRow(keys[index + 1]);
        break;
      case "ArrowUp":
        focusRow(keys[index - 1]);
        break;
      case "Home":
        focusRow(keys[0]);
        break;
      case "End":
        focusRow(keys.at(-1));
        break;
      default:
        return;
    }
    event.preventDefault();
  }
</script>

<section class="agents" aria-label="Agents">
  <div class="header">
    <h2 class="heading">
      <span class="title">Agents</span>
      {#if waiting > 0}
        <span class="attention" class:blocked={agents.blocked > 0}
          >{waiting} need{waiting === 1 ? "s" : ""} you</span
        >
      {:else}
        <span class="count">{agents.list.length}</span>
      {/if}
    </h2>
    {#if waiting > 0}
      <button
        type="button"
        class="next"
        title="Go to the next agent that needs you{nextShortcut ? ` (${nextShortcut})` : ''}"
        onclick={() => void agents.next()}>Next</button
      >
    {/if}
  </div>
  <!-- Keyboard handling is delegated from the focusable rows. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="list"
    bind:this={list}
    onkeydown={onKeydown}
    onfocusin={(event) => {
      if (event.target instanceof HTMLElement && event.target.dataset.paneId) {
        focusKey = event.target.dataset.paneId;
      }
    }}
  >
    {#each agents.list as agent (agent.paneId)}
      <button
        type="button"
        class="row"
        data-pane-id={agent.paneId}
        data-state={agent.state}
        tabindex={tabStop === agent.paneId ? 0 : -1}
        aria-label={describeAgentRow(agent)}
        title={describeAgentRow(agent)}
        onclick={() => void agents.focus(agent)}
      >
        <span class="icon"><AgentStateIcon state={agent.state} /></span>
        <span class="place">{agentPlace(agent)}</span>
        <span class="what">
          {agentName(agent.agent)}{#if agent.title}<span class="topic">{` · ${agent.title}`}</span
            >{/if}
        </span>
      </button>
    {:else}
      <p class="empty">No agents running</p>
    {/each}
  </div>
</section>

<style>
  .agents {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }

  .header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex: none;
    padding: var(--space-3) var(--space-4) var(--space-2);
  }

  .heading {
    display: flex;
    flex: 1;
    align-items: center;
    gap: var(--space-3);
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .title {
    flex: 1;
  }

  .count {
    font-weight: 400;
    font-variant-numeric: tabular-nums;
  }

  .attention {
    color: var(--color-text);
  }

  .attention.blocked {
    color: var(--color-agent-blocked);
  }

  .next {
    flex: none;
    padding: 0 var(--space-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-size: var(--text-xs);
  }

  .next:hover {
    color: var(--color-text);
  }

  .list {
    flex: 1;
    min-height: 0;
    padding-bottom: var(--space-3);
    overflow-y: auto;
  }

  .row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    grid-template-areas:
      "icon place"
      ". what";
    column-gap: var(--space-2);
    align-items: center;
    width: 100%;
    padding: 4px var(--space-4) 4px calc(var(--space-4) + var(--space-1));
    border: 0;
    background: none;
    text-align: left;
    line-height: var(--leading-tight);
  }

  .row:hover {
    background: var(--color-row-hover);
  }

  .row:focus-visible {
    box-shadow: inset var(--focus-ring);
  }

  .icon {
    display: flex;
    grid-area: icon;
  }

  .place {
    grid-area: place;
    overflow: hidden;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row[data-state="blocked"] .place {
    color: var(--color-agent-blocked);
  }

  .what {
    grid-area: what;
    overflow: hidden;
    color: var(--color-text-muted);
    font-size: var(--text-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .topic {
    color: var(--color-text-faint);
  }

  .empty {
    margin: 0;
    padding: var(--space-1) var(--space-4) var(--space-2) calc(var(--space-4) + var(--space-1));
    color: var(--color-text-faint);
    font-size: var(--text-sm);
  }
</style>
