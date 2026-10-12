<script lang="ts">
  import { branches } from "../../lib/state/branches.svelte";
  import { changes } from "../../lib/state/changes.svelte";
  import { view } from "../../lib/state/view.svelte";
  import { workspace } from "../../lib/state/workspace.svelte";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
  import Tabs from "../../lib/ui/Tabs.svelte";
  import BranchesView from "../branches/BranchesView.svelte";
  import ChangesView from "../changes/ChangesView.svelte";
  import { locateRepo, removeRepo } from "../repos/repo-actions";

  const views = [
    { id: "changes", label: "Changes" },
    { id: "branches", label: "Branches" },
  ] as const;

  const repo = $derived(workspace.activeRepo);
  const summary = $derived(repo ? workspace.summaries.get(repo.id) : undefined);
  const missing = $derived(repo !== null && (repo.missing || summary?.status === "missing"));

  $effect(() => {
    const shown = repo && !missing ? repo.id : null;
    void changes.show(shown);
    void branches.show(shown);
  });
</script>

<section class="work" aria-label="Working copy">
  <PaneHeader>
    <Tabs tabs={views} bind:selected={view.work} label="View" />
  </PaneHeader>
  <div class="body" role="tabpanel">
    {#if !repo}
      <EmptyState title="Select a repository to see its {view.work}." />
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
    {:else if view.work === "changes"}
      <ChangesView />
    {:else}
      <BranchesView {repo} />
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

  .body:has(:global(.changes)),
  .body:has(:global(.branches)) {
    overflow: hidden;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
    padding: 0 var(--space-4);
  }
</style>
