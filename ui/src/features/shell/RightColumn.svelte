<script lang="ts">
  import { untrack } from "svelte";

  import type { RepoId, RepoView } from "../../lib/api";
  import { agents } from "../../lib/state/agents.svelte";
  import { terminal } from "../../lib/state/terminal.svelte";
  import { view } from "../../lib/state/view.svelte";
  import { workspace } from "../../lib/state/workspace.svelte";
  import ReviewPane from "../branches/ReviewPane.svelte";
  import ReviewBar from "../comments/ReviewBar.svelte";
  import DiffPane from "../diff/DiffPane.svelte";

  const showTerminal = $derived(terminal.available && view.right === "terminal");
  /** The terminal starts when first shown and then stays, hidden or not. */
  let started = $state(false);
  $effect(() => {
    if (showTerminal) started = true;
    terminal.setVisible(showTerminal);
  });

  /** The selected repository has no herdr workspace yet. */
  let withoutWorkspace = $state<RepoView | null>(null);

  // Selecting a repository while the terminal is shown shows its workspace.
  $effect(() => {
    const repo = workspace.activeRepo;
    if (!showTerminal || !agents.available || !repo || repo.missing) {
      withoutWorkspace = null;
      return;
    }
    void agents.focusRepo(repo.id).then((found) => {
      if (workspace.activeRepo?.id === repo.id) withoutWorkspace = found ? null : repo;
    });
  });

  // Switching workspaces inside herdr selects the matching repository. Only
  // changes count, so a repository without a workspace stays selected.
  let lastFocused: RepoId | null = null;
  $effect(() => {
    const focused = agents.focusedRepoId;
    if (focused === lastFocused) return;
    lastFocused = focused;
    untrack(() => {
      if (showTerminal && focused && focused !== workspace.snapshot.activeRepoId) {
        void workspace.activate(focused);
      }
    });
  });
</script>

<div class="right">
  <div class="pane detail" class:hidden={showTerminal}>
    {#if view.detail === "branches"}
      <ReviewPane />
    {:else}
      <DiffPane />
    {/if}
    <ReviewBar />
  </div>
  {#if started}
    <!-- xterm.js loads with the first use of the terminal, not at startup. -->
    {#await import("../terminal/TerminalPane.svelte") then { default: TerminalPane }}
      <div class="pane" class:hidden={!showTerminal}>
        <TerminalPane visible={showTerminal} {withoutWorkspace} />
      </div>
    {/await}
  {/if}
</div>

<style>
  .right {
    display: grid;
    /* Both panes share one cell and never grow it beyond the window. */
    grid-template-rows: minmax(0, 1fr);
    grid-template-columns: minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .pane {
    display: grid;
    grid-area: 1 / 1;
    min-width: 0;
    min-height: 0;
  }

  .pane.detail {
    grid-template-rows: minmax(0, 1fr) auto;
  }

  .pane.hidden {
    visibility: hidden;
  }
</style>
