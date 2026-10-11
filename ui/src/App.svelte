<script lang="ts">
  import { onMount } from "svelte";

  import DiffPane from "./features/diff/DiffPane.svelte";
  import RepoSidebar from "./features/repos/RepoSidebar.svelte";
  import StatusBar from "./features/shell/StatusBar.svelte";
  import WorkPane from "./features/shell/WorkPane.svelte";
  import { appInfo } from "./lib/state/app-info.svelte";

  onMount(() => {
    void appInfo.load();
  });
</script>

<div class="app">
  <main class="panes">
    <RepoSidebar />
    <WorkPane />
    <DiffPane />
  </main>
  <StatusBar />
</div>

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
