<script lang="ts">
  import { branches } from "../../lib/state/branches.svelte";
  import { changes } from "../../lib/state/changes.svelte";
  import { commit } from "../../lib/state/commit.svelte";
  import { view, type WorkList } from "../../lib/state/view.svelte";
  import { workspace } from "../../lib/state/workspace.svelte";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
  import Splitter from "../../lib/ui/Splitter.svelte";
  import { AUTO_BRANCH_ROWS, BRANCH_ROW_HEIGHT } from "../branches/branch-layout";
  import BranchesView from "../branches/BranchesView.svelte";
  import ChangesView from "../changes/ChangesView.svelte";
  import CommitForm from "../changes/CommitForm.svelte";
  import { locateRepo, removeRepo } from "../repos/repo-actions";

  /** The changes keep at least this much height when the branch list is resized. */
  const MIN_CHANGES_HEIGHT = 120;

  const repo = $derived(workspace.activeRepo);
  const summary = $derived(repo ? workspace.summaries.get(repo.id) : undefined);
  const missing = $derived(repo !== null && (repo.missing || summary?.status === "missing"));

  let listHeight = $state(0);
  let changesHeight = $state(0);
  let commitHeight = $state(0);
  /** The changed files keep their minimum height above the commit form. */
  const minChangesHeight = $derived(MIN_CHANGES_HEIGHT + commitHeight);
  const hasList = $derived(branches.list !== null && branches.error === null);
  const listHeightWanted = $derived(
    view.branchHeight ??
      Math.min(Math.max(branches.branches.length, 1), AUTO_BRANCH_ROWS) * BRANCH_ROW_HEIGHT,
  );
  /** The list and the changes share this space; the measured sum stays put while resizing. */
  const maxListHeight = $derived(
    Math.max(BRANCH_ROW_HEIGHT, listHeight + changesHeight - minChangesHeight),
  );

  $effect(() => {
    const shown = repo && !missing ? repo.id : null;
    void changes.show(shown);
    void branches.show(shown);
    void commit.show(shown);
  });

  function follow(list: WorkList) {
    return () => {
      view.detail = list;
      view.right = "detail";
    };
  }
</script>

<section class="work" aria-label="Working copy">
  {#if !repo}
    <PaneHeader />
    <EmptyState title="Select a repository to see its changes and branches." />
  {:else if missing}
    <PaneHeader />
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
    <div
      class="section branches-section"
      role="group"
      aria-label="Branch board"
      onfocusin={follow("branches")}
      onpointerdown={follow("branches")}
    >
      <BranchesView {repo} height={listHeightWanted} bind:listHeight />
    </div>
    {#if hasList}
      <Splitter
        label="Resize branches"
        value={Math.min(listHeightWanted, maxListHeight)}
        min={BRANCH_ROW_HEIGHT}
        max={maxListHeight}
        step={BRANCH_ROW_HEIGHT}
        onchange={(height: number) => (view.branchHeight = height)}
        onreset={() => (view.branchHeight = null)}
      />
    {/if}
    <div
      class="section changes-section"
      role="group"
      aria-label="Changes"
      style:min-height="{minChangesHeight}px"
      bind:clientHeight={changesHeight}
      onfocusin={follow("changes")}
      onpointerdown={follow("changes")}
    >
      <div class="changes-list">
        <ChangesView />
      </div>
      {#if changes.status}
        <div class="commit-slot" bind:clientHeight={commitHeight}>
          <CommitForm />
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .work {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
    background: var(--color-surface);
    border-right: 1px solid var(--color-border);
  }

  .section {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .branches-section {
    flex: 0 1 auto;
  }

  .changes-section {
    flex: 1 1 0;
  }

  .changes-list {
    display: flex;
    flex: 1 1 0;
    flex-direction: column;
    min-height: 0;
  }

  .commit-slot {
    display: flex;
    flex: none;
    flex-direction: column;
  }

  .missing {
    overflow-y: auto;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
    padding: 0 var(--space-4);
  }
</style>
