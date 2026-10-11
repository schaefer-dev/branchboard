<script lang="ts">
  import { workspace } from "../../lib/state/workspace.svelte";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
  import Tabs from "../../lib/ui/Tabs.svelte";
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
    {:else}
      <EmptyState title={repo.name} hint="The {view} view arrives with the next milestone." />
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

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
    padding: 0 var(--space-4);
  }
</style>
