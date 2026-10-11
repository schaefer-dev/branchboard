<script lang="ts">
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
</script>

<nav class="sidebar" aria-label="Repositories">
  <PaneHeader title="Repositories" />
  <EmptyState
    title="No repositories yet"
    hint="Add a repository to see its changes and branches here."
  />
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--color-chrome);
    border-right: 1px solid var(--color-border);
  }
</style>
