<script lang="ts">
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { onMount } from "svelte";

  import CommitsPane from "./features/branches/CommitsPane.svelte";
  import DiffPane from "./features/diff/DiffPane.svelte";
  import DropOverlay from "./features/repos/DropOverlay.svelte";
  import QuickSwitcher from "./features/repos/QuickSwitcher.svelte";
  import RepoSidebar from "./features/repos/RepoSidebar.svelte";
  import { registerAppCommands } from "./features/shell/app-commands";
  import CommandPalette from "./features/shell/CommandPalette.svelte";
  import StatusBar from "./features/shell/StatusBar.svelte";
  import WorkPane from "./features/shell/WorkPane.svelte";
  import { commands } from "./lib/commands";
  import { appInfo } from "./lib/state/app-info.svelte";
  import { branches } from "./lib/state/branches.svelte";
  import { changes } from "./lib/state/changes.svelte";
  import { notices } from "./lib/state/notices.svelte";
  import { view } from "./lib/state/view.svelte";
  import { workspace } from "./lib/state/workspace.svelte";
  import Notices from "./lib/ui/Notices.svelte";
  import PromptDialog from "./lib/ui/PromptDialog.svelte";

  let switcherOpen = $state(false);
  let paletteOpen = $state(false);
  let dragging = $state(false);

  onMount(() => {
    void appInfo.load();
    Promise.all([changes.init(), branches.init(), workspace.init()]).catch((error: unknown) => {
      notices.error(error);
    });

    const unregister = registerAppCommands({
      openSwitcher: () => (switcherOpen = true),
      openPalette: () => (paletteOpen = true),
    });

    const unlistenDrop = getCurrentWebview().onDragDropEvent((event) => {
      const { payload } = event;
      if (payload.type === "enter" || payload.type === "over") {
        dragging = true;
      } else {
        dragging = false;
        if (payload.type === "drop") void workspace.addFolders(payload.paths);
      }
    });

    return () => {
      unregister();
      void unlistenDrop.then((fn) => {
        fn();
      });
    };
  });

  function onKeydown(event: KeyboardEvent) {
    commands.handleKeydown(event);
  }

  function onContextMenu(event: MouseEvent) {
    // Keep the WebView's own menu out of the app; text fields keep theirs
    // (copy/paste). In development it stays available for "Inspect".
    const editable =
      event.target instanceof HTMLElement &&
      (event.target.isContentEditable || ["INPUT", "TEXTAREA"].includes(event.target.tagName));
    if (!import.meta.env.DEV && !editable) event.preventDefault();
  }
</script>

<svelte:window
  onkeydown={onKeydown}
  oncontextmenu={onContextMenu}
  onfocus={() => void branches.reload()}
/>

<div class="app">
  <main class="panes">
    <RepoSidebar />
    <WorkPane />
    {#if view.work === "branches"}
      <CommitsPane />
    {:else}
      <DiffPane />
    {/if}
  </main>
  <StatusBar />
</div>

<QuickSwitcher bind:open={switcherOpen} />
<CommandPalette bind:open={paletteOpen} />
<PromptDialog />
<Notices />
<DropOverlay visible={dragging} />

<style>
  .app {
    display: grid;
    grid-template-rows: 1fr auto;
    height: 100%;
  }

  .panes {
    display: grid;
    grid-template-columns: var(--sidebar-width) var(--list-width) minmax(0, 1fr);
    min-height: 0;
  }
</style>
