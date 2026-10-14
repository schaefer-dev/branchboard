<script lang="ts">
  import type { AgentState } from "../../lib/api";

  interface Props {
    state: AgentState;
  }

  let { state }: Props = $props();

  /** Each state has its own shape, so it reads without color too. */
  const shapes = {
    blocked: "ringed-dot",
    working: "half",
    done: "check",
    idle: "ring",
    unknown: "small-dot",
  } as const satisfies Record<AgentState, string>;
</script>

<span class="state" data-state={state} data-shape={shapes[state]}>
  <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
    {#if state === "blocked"}
      <circle cx="7" cy="7" r="6" fill="currentColor" opacity="0.3" />
      <circle cx="7" cy="7" r="3.5" fill="currentColor" />
    {:else if state === "working"}
      <circle cx="7" cy="7" r="4.75" fill="none" stroke="currentColor" stroke-width="1.5" />
      <path d="M7 2.25a4.75 4.75 0 0 0 0 9.5z" fill="currentColor" />
    {:else if state === "done"}
      <path
        d="M3 7.25l2.6 2.6L11 4.5"
        fill="none"
        stroke="currentColor"
        stroke-width="1.75"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    {:else if state === "idle"}
      <circle cx="7" cy="7" r="4" fill="none" stroke="currentColor" stroke-width="1.5" />
    {:else}
      <circle cx="7" cy="7" r="1.75" fill="currentColor" />
    {/if}
  </svg>
</span>

<style>
  .state {
    display: inline-flex;
    flex: none;
    color: var(--color-agent-unknown);
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

  .state[data-state="idle"] {
    color: var(--color-agent-idle);
  }
</style>
