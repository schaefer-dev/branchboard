<script lang="ts">
  import type { CommitList } from "../../lib/api";
  import { branches } from "../../lib/state/branches.svelte";
  import { timeAgo } from "../../lib/state/branches-model";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";

  const data = $derived(branches.commits);
  const isMainline = $derived(
    data !== null &&
      data.behind !== null &&
      data.ahead.commits.length === 0 &&
      data.behind.commits.length === 0,
  );
</script>

{#snippet list(title: string, commits: CommitList, empty: string)}
  <section class="group" aria-label={title}>
    <h3>
      {title}
      <span class="count">{commits.commits.length}{commits.truncated ? "+" : ""}</span>
    </h3>
    {#if commits.commits.length === 0}
      <p class="empty">{empty}</p>
    {:else}
      <ol>
        {#each commits.commits as commit (commit.oid)}
          <li title={commit.oid}>
            <span class="oid">{commit.shortOid}</span>
            <span class="subject">{commit.subject || "(no message)"}</span>
            <span class="author">{commit.author}</span>
            <span class="when">{timeAgo(commit.authoredAt)}</span>
          </li>
        {/each}
      </ol>
      {#if commits.truncated}
        <p class="empty">Only the latest {commits.commits.length} commits are shown.</p>
      {/if}
    {/if}
  </section>
{/snippet}

<section class="pane" aria-label="Branch commits">
  <PaneHeader>
    {#if data}
      <span class="branch">{data.branch}</span>
      <span class="relation">compared with {data.mainline}</span>
    {/if}
  </PaneHeader>
  <div class="body selectable">
    {#if branches.commitsError}
      <EmptyState title="The commits could not be loaded." hint={branches.commitsError.message} />
    {:else if !data}
      <EmptyState title="Select a branch to see its commits." />
    {:else if data.behind === null}
      {@render list("Latest commits", data.ahead, "This branch has no commits.")}
    {:else if isMainline}
      <EmptyState
        title="Up to date with {data.mainline}"
        hint="This branch has the same commits."
      />
    {:else}
      {@render list(`Ahead of ${data.mainline}`, data.ahead, "Nothing that isn't on the mainline.")}
      {@render list(`Behind ${data.mainline}`, data.behind, "Has everything from the mainline.")}
    {/if}
  </div>
</section>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--color-surface);
  }

  .branch {
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .relation {
    flex: none;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .group + .group {
    border-top: 1px solid var(--color-border);
  }

  h3 {
    display: flex;
    gap: var(--space-3);
    margin: 0;
    padding: var(--space-4) var(--space-4) var(--space-2);
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .count {
    color: var(--color-text-faint);
    font-weight: 400;
    font-variant-numeric: tabular-nums;
  }

  ol {
    margin: 0 0 var(--space-3);
    padding: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: baseline;
    gap: var(--space-4);
    padding: var(--space-2) var(--space-4);
  }

  li:hover {
    background: var(--color-row-hover);
  }

  .oid {
    color: var(--color-text-faint);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  .subject {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .author,
  .when {
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    white-space: nowrap;
  }

  .when {
    min-width: 7ch;
    text-align: right;
  }

  .empty {
    margin: 0 0 var(--space-4);
    padding: 0 var(--space-4);
    color: var(--color-text-faint);
    font-size: var(--text-sm);
  }
</style>
