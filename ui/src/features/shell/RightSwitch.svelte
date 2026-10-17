<script lang="ts">
  import { commands } from "../../lib/commands";
  import { terminal } from "../../lib/state/terminal.svelte";
  import { view } from "../../lib/state/view.svelte";

  const detailLabel = $derived(view.detail === "branches" ? "Review" : "Diff");
  const terminalTitle = $derived(
    `Terminal (${commands.shortcutLabel("terminal.show") ?? ""})`.replace(" ()", ""),
  );
</script>

<div class="switch" role="group" aria-label="Right pane">
  <button
    type="button"
    aria-pressed={view.right === "detail"}
    onclick={() => (view.right = "detail")}
  >
    {detailLabel}
  </button>
  <button
    type="button"
    aria-pressed={view.right === "history"}
    title="History (Cmd/Ctrl+4)"
    onclick={() => view.showHistory()}>History</button
  >
  {#if terminal.available}
    <button
      type="button"
      aria-pressed={view.right === "terminal"}
      title={terminalTitle}
      onclick={() => {
        view.showTerminal();
      }}
    >
      Terminal
    </button>
  {/if}
</div>

<style>
  .switch {
    display: inline-flex;
    flex: none;
    margin-left: auto;
    padding: 1px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-sunken);
  }

  button {
    padding: 1px var(--space-3);
    border: 0;
    border-radius: 3px;
    background: none;
    color: var(--color-text-muted);
    font-size: var(--text-xs);
  }

  button[aria-pressed="true"] {
    background: var(--color-surface);
    color: var(--color-text);
    box-shadow: 0 0 0 1px var(--color-border);
  }
</style>
