<script lang="ts">
  import { changes } from "../../lib/state/changes.svelte";
  import { workspace } from "../../lib/state/workspace.svelte";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
  import Tabs from "../../lib/ui/Tabs.svelte";
  import ChangesView from "../changes/ChangesView.svelte";
  import { locateRepo, removeRepo } from "../repos/repo-actions";

  type View = "changes" | "branches";

  const views = [
    { id: "changes", label: "Changes" },
    { id: "branches", label: "Branches" },
  ] as const;

  let view = $state<View>("changes");

  const repo = $derived(workspace.activeRepo);
  const summary = $derived(repo ? workspace.summaries.get(repo.id) : undefined);
  const missing = $derived(repo !== null && (repo.missing || summary?.status === "missing"));

  $effect(() => {
    void changes.show(repo && !missing ? repo.id : null);
  });
</script>

<section class="work" aria-label="Working copy">
  <PaneHeader>
    <Tabs tabs={views} bind:selected={view} label="View" />
  </PaneHeader>
  <div class="body" role="tabpanel">
    {#if !repo}
      <EmptyState title="Select a repository to see its {view}." />
    {:else if missing}
      <div class="missing">
        <EmptyState
          title="{repo.name} can't be found"
          hint="The folder {repo.path} was moved, renamed or deleted. Locate it to keep using it here."
        />
        <div class="actions">
          <button type="button" class="btn btn-primary" onclick={() => void locateRepo(repo)}>
            Locate…
          </button>
          <button type="button" class="btn" onclick={() => void removeRepo(repo)}>
            Remove from Branchboard
          </button>
        </div>
      </div>
    {:else if view === "changes"}
      <ChangesView />
    {:else}
      <EmptyState title={repo.name} hint="Branches arrive with the next milestone." />
    {/if}
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

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .body:has(:global(.changes)) {
    overflow: hidden;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
    padding: 0 var(--space-4);
  }
</style>
