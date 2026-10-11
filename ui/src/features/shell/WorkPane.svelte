<script lang="ts">
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
  import Tabs from "../../lib/ui/Tabs.svelte";

  type View = "changes" | "branches";

  const views = [
    { id: "changes", label: "Changes" },
    { id: "branches", label: "Branches" },
  ] as const;

  let view = $state<View>("changes");
</script>

<section class="work" aria-label="Working copy">
  <PaneHeader>
    <Tabs tabs={views} bind:selected={view} label="View" />
  </PaneHeader>
  <div role="tabpanel">
    <EmptyState title="Select a repository to see its {view}." />
  </div>
</section>

<style>
  .work {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--color-surface);
    border-right: 1px solid var(--color-border);
  }
</style>
