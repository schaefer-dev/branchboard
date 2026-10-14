<script lang="ts">
  import type { RepoAgents } from "../../lib/api";
  import { describeAgents } from "./agent-text";
  import AgentStateIcon from "./AgentStateIcon.svelte";

  interface Props {
    agents: RepoAgents;
  }

  let { agents }: Props = $props();

  const description = $derived(describeAgents(agents));
</script>

<span
  class="badge"
  data-state={agents.state}
  role="img"
  aria-label={description.replaceAll("\n", "; ")}
  title={description}
>
  <AgentStateIcon state={agents.state} />
  {#if agents.agents.length > 1}
    <span class="count">{agents.agents.length}</span>
  {/if}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 1px;
    color: var(--color-agent-idle);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
  }

  .badge[data-state="blocked"] {
    color: var(--color-agent-blocked);
    font-weight: 600;
  }
</style>
