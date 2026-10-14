<script lang="ts">
  import type { AgentState } from "../../lib/api";
  import Icon from "../../lib/ui/Icon.svelte";

  interface Props {
    state: AgentState;
  }

  let { state }: Props = $props();
</script>

<span class="state" data-state={state}><Icon name="agent" size={14} /></span>

<style>
  .state {
    display: inline-flex;
    color: var(--color-agent-idle);
  }

  .state[data-state="blocked"] {
    color: var(--color-agent-blocked);
  }

  .state[data-state="done"] {
    color: var(--color-agent-done);
  }

  .state[data-state="working"] {
    color: var(--color-agent-working);
  }
</style>
