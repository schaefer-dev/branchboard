<script lang="ts">
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { onMount } from "svelte";

  import DiffPane from "./features/diff/DiffPane.svelte";
  import DropOverlay from "./features/repos/DropOverlay.svelte";
  import QuickSwitcher from "./features/repos/QuickSwitcher.svelte";
  import RepoSidebar from "./features/repos/RepoSidebar.svelte";
  import { addRepositories } from "./features/repos/repo-actions";
  import StatusBar from "./features/shell/StatusBar.svelte";
  import WorkPane from "./features/shell/WorkPane.svelte";
  import { commands } from "./lib/commands";
  import { appInfo } from "./lib/state/app-info.svelte";
  import { notices } from "./lib/state/notices.svelte";
  import { workspace } from "./lib/state/workspace.svelte";
  import Notices from "./lib/ui/Notices.svelte";
  import PromptDialog from "./lib/ui/PromptDialog.svelte";

  let switcherOpen = $state(false);
  let dragging = $state(false);

  onMount(() => {
    void appInfo.load();
    workspace.init().catch((error: unknown) => {
      notices.error(error);
    });

    const unregister = [
      commands.register({
        id: "repo.add",
        title: "Add repositories…",
        shortcut: { key: "o", mod: true },
        run: addRepositories,
      }),
      commands.register({
        id: "repo.switch",
        title: "Switch repository…",
        shortcut: { key: "p", mod: true },
        run: () => (switcherOpen = true),
      }),
    ];

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
      for (const fn of unregister) fn();
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

<svelte:window onkeydown={onKeydown} oncontextmenu={onContextMenu} />

<div class="app">
  <main class="panes">
    <RepoSidebar />
    <WorkPane />
    <DiffPane />
  </main>
  <StatusBar />
</div>

<QuickSwitcher bind:open={switcherOpen} />
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
