<script lang="ts">
  import { untrack } from "svelte";

  import type { CommitList, CommitSummary, ReviewTarget } from "../../lib/api";
  import { branches } from "../../lib/state/branches.svelte";
  import { timeAgo } from "../../lib/state/branches-model";
  import { splitPath } from "../../lib/state/changes-model";
  import { comments } from "../../lib/state/comments.svelte";
  import type { CommentTarget } from "../../lib/state/comments-model";
  import { review } from "../../lib/state/review.svelte";
  import { targetKey } from "../../lib/state/review-model";
  import { view } from "../../lib/state/view.svelte";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
  import Splitter from "../../lib/ui/Splitter.svelte";
  import FileRow from "../changes/FileRow.svelte";
  import DiffBody from "../diff/DiffBody.svelte";
  import RightSwitch from "../shell/RightSwitch.svelte";

  const ROW_HEIGHT = 26;
  /** The diff keeps at least this much height when the lists are resized. */
  const MIN_DIFF_HEIGHT = 120;

  const data = $derived(branches.commits);
  const isMainline = $derived(
    data !== null &&
      data.behind !== null &&
      data.ahead.commits.length === 0 &&
      data.behind.commits.length === 0,
  );
  const file = $derived(review.cursorFile);
  const parts = $derived(file ? splitPath(file.path) : null);
  const reviewDiff = $derived(review.diff.status === "ready" ? review.diff.diff : null);
  const commentTarget = $derived.by((): CommentTarget | null => {
    const repoId = review.repoId;
    const key = review.targetKey;
    return file && repoId && key ? { repoId, path: file.path, source: `review:${key}` } : null;
  });
  const selectedCommit = $derived.by((): CommitSummary | null => {
    const target = review.target;
    if (target?.kind !== "commit" || !data) return null;
    const all = [...data.ahead.commits, ...(data.behind?.commits ?? [])];
    return all.find((commit) => commit.oid === target.oid) ?? null;
  });
  const formatter = new Intl.NumberFormat();

  let commitsHeight = $state(0);
  let filesHeight = $state(0);
  let diffHeight = $state(0);

  // The review follows the branch under the cursor of the branch list; only
  // these two inputs are tracked, not the review state `follow` updates.
  $effect(() => {
    const repoId = branches.repoId;
    const commits = branches.commits;
    untrack(() => {
      review.follow(repoId, commits);
    });
  });

  const optionId = (target: ReviewTarget) => `review-${targetKey(target)}`;
  const commitTarget = (commit: CommitSummary): ReviewTarget => ({
    kind: "commit",
    oid: commit.oid,
  });

  function targetFromEvent(event: Event): ReviewTarget | undefined {
    const element = event.target instanceof Element ? event.target.closest("[data-target]") : null;
    const key = element instanceof HTMLElement ? element.dataset.target : undefined;
    return review.targets.find((target) => targetKey(target) === key);
  }

  function onCommitsKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown") review.moveTarget(1);
    else if (event.key === "ArrowUp") review.moveTarget(-1);
    else return;
    event.preventDefault();
  }

  function onFilesKeydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown") review.moveFile(1);
    else if (event.key === "ArrowUp") review.moveFile(-1);
    else return;
    event.preventDefault();
  }

  function onFilesClick(event: MouseEvent) {
    const element = event.target instanceof Element ? event.target.closest("[data-key]") : null;
    if (element instanceof HTMLElement && element.dataset.key)
      review.selectFile(element.dataset.key);
  }
</script>

{#snippet commitRow(commit: CommitSummary)}
  {@const target = commitTarget(commit)}
  <div
    id={optionId(target)}
    class="row commit"
    class:selected={review.targetKey === targetKey(target)}
    role="option"
    aria-selected={review.targetKey === targetKey(target)}
    data-target={targetKey(target)}
    title={commit.oid}
  >
    <span class="oid">{commit.shortOid}</span>
    <span class="subject">{commit.subject || "(no message)"}</span>
    <span class="author">{commit.author}</span>
    <span class="when">{timeAgo(commit.authoredAt)}</span>
  </div>
{/snippet}

{#snippet group(title: string, commits: CommitList, empty: string)}
  <div class="group" role="group" aria-label={title}>
    <div class="group-title" role="presentation">
      {title}
      <span class="count">{commits.commits.length}{commits.truncated ? "+" : ""}</span>
    </div>
    {#if commits.commits.length === 0}
      <p class="empty">{empty}</p>
    {:else}
      {#each commits.commits as commit (commit.oid)}
        {@render commitRow(commit)}
      {/each}
      {#if commits.truncated}
        <p class="empty">Only the latest {commits.commits.length} commits are shown.</p>
      {/if}
    {/if}
  </div>
{/snippet}

<section class="pane" aria-label="Branch review">
  <PaneHeader>
    {#if data}
      <span class="branch">{data.branch}</span>
      <span class="relation">compared with {data.mainline}</span>
    {/if}
    <RightSwitch />
  </PaneHeader>

  {#if branches.commitsError}
    <EmptyState title="The commits could not be loaded." hint={branches.commitsError.message} />
  {:else if !data}
    <EmptyState title="Select a branch to review its changes." />
  {:else if isMainline}
    <EmptyState title="Up to date with {data.mainline}" hint="This branch has the same commits." />
  {:else}
    <div
      class="section commits selectable"
      style:height={view.reviewCommitsHeight === null
        ? null
        : `${String(view.reviewCommitsHeight)}px`}
      class:auto={view.reviewCommitsHeight === null}
      bind:clientHeight={commitsHeight}
    >
      <div
        class="list"
        role="listbox"
        aria-label="Commits"
        tabindex={0}
        aria-activedescendant={review.target ? optionId(review.target) : undefined}
        onkeydown={onCommitsKeydown}
        onclick={(event) => {
          const target = targetFromEvent(event);
          if (target) review.select(target);
        }}
      >
        {#if data.behind === null}
          {@render group("Latest commits", data.ahead, "This branch has no commits.")}
        {:else}
          {@const whole: ReviewTarget = { kind: "branch", name: data.branch }}
          <div
            id={optionId(whole)}
            class="row whole"
            class:selected={review.targetKey === targetKey(whole)}
            role="option"
            aria-selected={review.targetKey === targetKey(whole)}
            data-target={targetKey(whole)}
          >
            <span class="whole-title">All changes</span>
            <span class="whole-hint">
              {data.ahead.commits.length}{data.ahead.truncated ? "+" : ""}
              {data.ahead.commits.length === 1 ? "commit" : "commits"} since {data.mainline}
            </span>
          </div>
          {@render group(
            `Ahead of ${data.mainline}`,
            data.ahead,
            "Nothing that isn't on the mainline.",
          )}
          {@render group(
            `Behind ${data.mainline}`,
            data.behind,
            "Has everything from the mainline.",
          )}
        {/if}
      </div>
    </div>
    <Splitter
      label="Resize commits"
      value={commitsHeight}
      min={ROW_HEIGHT * 2}
      max={Math.max(ROW_HEIGHT * 2, commitsHeight + diffHeight - MIN_DIFF_HEIGHT)}
      step={ROW_HEIGHT}
      onchange={(height: number) => (view.reviewCommitsHeight = height)}
      onreset={() => (view.reviewCommitsHeight = null)}
    />

    <div
      class="section files"
      style:height={view.reviewFilesHeight === null ? null : `${String(view.reviewFilesHeight)}px`}
      class:auto={view.reviewFilesHeight === null}
      bind:clientHeight={filesHeight}
    >
      {#if review.filesError}
        <EmptyState title="The files could not be loaded." hint={review.filesError.message} />
      {:else if review.files && review.files.files.length === 0}
        <EmptyState title="No file changes" hint="The selected commits change no files." />
      {:else if review.files}
        <div
          class="list"
          role="listbox"
          aria-label="Files in this review"
          tabindex={0}
          aria-activedescendant={review.fileCursor ? `review-file-${review.fileCursor}` : undefined}
          onkeydown={onFilesKeydown}
          onclick={onFilesClick}
        >
          {#each review.files.files as reviewFile (reviewFile.path)}
            <div class="file-row">
              <FileRow
                id="review-file-{reviewFile.path}"
                key={reviewFile.path}
                path={reviewFile.path}
                origPath={reviewFile.origPath}
                kind={reviewFile.kind}
                selected={review.fileCursor === reviewFile.path}
                cursor={review.fileCursor === reviewFile.path}
                additions={reviewFile.additions}
                deletions={reviewFile.deletions}
              />
            </div>
          {/each}
        </div>
      {/if}
    </div>
    <Splitter
      label="Resize files"
      value={filesHeight}
      min={ROW_HEIGHT * 2}
      max={Math.max(ROW_HEIGHT * 2, filesHeight + diffHeight - MIN_DIFF_HEIGHT)}
      step={ROW_HEIGHT}
      onchange={(height: number) => (view.reviewFilesHeight = height)}
      onreset={() => (view.reviewFilesHeight = null)}
    />

    <section
      class="section diff"
      aria-label="Selected file"
      style:min-height="{MIN_DIFF_HEIGHT}px"
      bind:clientHeight={diffHeight}
    >
      {#if file && parts}
        <div class="diff-head">
          <span class="path selectable" title={file.path}>
            {#if parts.dir}<span class="dir">{parts.dir}/</span>{/if}<span class="name"
              >{parts.name}</span
            >
          </span>
          <span class="scope">
            {#if selectedCommit}
              in {selectedCommit.shortOid}
            {:else}
              since {data.mainline}
            {/if}
          </span>
          <span class="spacer"></span>
          {#if reviewDiff && !reviewDiff.binary}
            <span class="stats">
              <span class="additions">+{formatter.format(reviewDiff.additions)}</span>
              <span class="deletions">−{formatter.format(reviewDiff.deletions)}</span>
            </span>
          {/if}
          <button
            type="button"
            class="icon-btn"
            title="Comment on this file"
            aria-label="Comment on this file"
            disabled={!reviewDiff || reviewDiff.hunks.length === 0}
            onclick={() => {
              if (commentTarget) comments.start(commentTarget, null);
            }}
          >
            <Icon name="comment" />
          </button>
        </div>
      {/if}
      <DiffBody
        state={file ? review.diff : { status: "idle" }}
        empty="Select a file to see its changes."
        onShowFull={() => void review.loadDiff(true)}
        target={commentTarget}
      />
    </section>
  {/if}
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

  .section {
    display: flex;
    flex: none;
    flex-direction: column;
    min-height: 0;
    overflow-y: auto;
  }

  .section.commits.auto {
    max-height: 40%;
  }

  .section.files.auto {
    max-height: 30%;
  }

  .section.diff {
    flex: 1 1 0;
    overflow: hidden;
  }

  .list {
    outline: none;
  }

  .row {
    display: grid;
    align-items: baseline;
    gap: var(--space-4);
    min-height: 26px;
    padding: var(--space-2) var(--space-4);
  }

  .row:hover {
    background: var(--color-row-hover);
  }

  .row.selected {
    background: var(--color-row-selected);
  }

  .list:focus-visible .row.selected {
    box-shadow: inset var(--focus-ring);
  }

  .commit {
    grid-template-columns: auto minmax(0, 1fr) auto auto;
  }

  .whole {
    grid-template-columns: auto minmax(0, 1fr);
    border-bottom: 1px solid var(--color-border);
  }

  .whole-title {
    font-weight: 600;
  }

  .whole-hint {
    overflow: hidden;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group + .group {
    border-top: 1px solid var(--color-border);
  }

  .group-title {
    display: flex;
    gap: var(--space-3);
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

  .file-row {
    height: 26px;
  }

  .diff-head {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-4);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-chrome);
  }

  .path {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dir {
    color: var(--color-text-muted);
  }

  .name {
    font-weight: 600;
  }

  .scope {
    flex: none;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .spacer {
    flex: 1;
  }

  .stats {
    display: flex;
    flex: none;
    gap: var(--space-3);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }

  .additions {
    color: var(--color-added);
  }

  .deletions {
    color: var(--color-deleted);
  }
</style>
