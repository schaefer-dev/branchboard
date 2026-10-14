<script lang="ts">
  import { agents } from "../../lib/state/agents.svelte";
  import { view } from "../../lib/state/view.svelte";
  import Splitter from "../../lib/ui/Splitter.svelte";
  import AgentList from "../agents/AgentList.svelte";
  import RepoSidebar from "../repos/RepoSidebar.svelte";

  /** Height of one agent row (two lines). */
  const AGENT_ROW_HEIGHT = 38;
  /** The agents header and bottom padding. */
  const AGENTS_CHROME = 40;
  /** Agents shown without scrolling unless resized. */
  const AUTO_AGENT_ROWS = 5;
  /** The repositories keep at least this much height. */
  const MIN_REPOS_HEIGHT = 160;

  /** Up to five rows; half of a sixth hints that the list scrolls. */
  function autoRows(count: number): number {
    return count > AUTO_AGENT_ROWS ? AUTO_AGENT_ROWS + 0.5 : Math.max(count, 1);
  }

  let height = $state(0);

  const maxAgentsHeight = $derived(Math.max(AGENTS_CHROME, height - MIN_REPOS_HEIGHT));
  const agentsHeight = $derived(
    Math.min(
      maxAgentsHeight,
      view.agentsHeight ?? AGENTS_CHROME + autoRows(agents.list.length) * AGENT_ROW_HEIGHT,
    ),
  );
</script>

<div class="sidebar" bind:clientHeight={height}>
  <RepoSidebar />
  {#if agents.available}
    <Splitter
      label="Resize agents"
      value={height - agentsHeight}
      min={MIN_REPOS_HEIGHT}
      max={height - AGENTS_CHROME}
      step={AGENT_ROW_HEIGHT}
      onchange={(reposHeight: number) => (view.agentsHeight = height - reposHeight)}
      onreset={() => (view.agentsHeight = null)}
    />
    <div class="agents" style:height="{agentsHeight}px">
      <AgentList />
    </div>
  {/if}
</div>

<style>
  .sidebar {
    display: grid;
    grid-template-rows: minmax(0, 1fr) auto auto;
    min-height: 0;
    background: var(--color-chrome);
  }

  .agents {
    min-height: 0;
    border-right: 1px solid var(--color-border);
  }
</style>
