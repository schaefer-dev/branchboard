<script lang="ts">
  import type { RepoAgents } from "../../lib/api";
  import Icon from "../../lib/ui/Icon.svelte";
  import { describeAgents } from "./agent-text";

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
  <Icon name="agent" size={14} />
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

  .badge[data-state="done"] {
    color: var(--color-agent-done);
  }

  .badge[data-state="working"] {
    color: var(--color-agent-working);
  }
</style>
