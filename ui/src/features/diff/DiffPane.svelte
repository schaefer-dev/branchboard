<script lang="ts">
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
</script>

<section class="diff" aria-label="Diff">
  <PaneHeader />
  <EmptyState title="Select a file to see its changes." />
</section>

<style>
  .diff {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--color-surface);
  }
</style>
