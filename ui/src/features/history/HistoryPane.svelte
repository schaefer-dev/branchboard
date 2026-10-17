<script lang="ts">
  import { untrack } from "svelte";
  import type { CommitSummary } from "../../lib/api";
  import { showContextMenu } from "../../lib/platform/menu";
  import { timeAgo } from "../../lib/state/branches-model";
  import { comments } from "../../lib/state/comments.svelte";
  import type { CommentTarget } from "../../lib/state/comments-model";
  import { HISTORY_ROW_LIMIT, type HistorySession } from "../../lib/state/history.svelte";
  import { view } from "../../lib/state/view.svelte";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
  import Splitter from "../../lib/ui/Splitter.svelte";
  import VirtualList from "../../lib/ui/VirtualList.svelte";
  import FileRow from "../changes/FileRow.svelte";
  import DiffBody from "../diff/DiffBody.svelte";
  import RightSwitch from "../shell/RightSwitch.svelte";

  let { session, visible }: { session: HistorySession | null; visible: boolean } = $props();
  const review = $derived(session?.review);
  const oid = $derived(review?.target?.kind === "commit" ? review.target.oid : null);
  const outside = $derived(oid && !session?.commits.some((c) => c.oid === oid));
  const target = $derived.by((): CommentTarget | null =>
    session && oid && review?.cursorFile
      ? { repoId: session.repoId, path: review.cursorFile.path, source: `history:${oid}` }
      : null,
  );
  let list = $state<{ scrollToIndex: (index: number) => void }>();
  let paneHeight = $state(700);
  let search = $state<HTMLInputElement>();

  $effect(() => {
    const request = view.historyFocusRequest;
    if (visible && request) untrack(() => search?.focus());
  });

  function fileClick(event: MouseEvent, context = false) {
    const element = event.target instanceof Element ? event.target.closest("[data-key]") : null;
    const path = element instanceof HTMLElement ? element.dataset.key : undefined;
    if (!path || !review || !session || !oid) return;
    review.selectFile(path);
    if (context) {
      event.preventDefault();
      const target: CommentTarget = { repoId: session.repoId, path, source: `history:${oid}` };
      void showContextMenu([
        { kind: "item", text: "Comment on file", run: () => comments.start(target, null) },
      ]);
    }
  }

  function commitClick(event: MouseEvent) {
    const element = event.target instanceof Element ? event.target.closest("[data-oid]") : null;
    if (element instanceof HTMLElement && element.dataset.oid) session?.select(element.dataset.oid);
  }

  function commitKeydown(event: KeyboardEvent) {
    if (!session || !["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const delta =
      event.key === "ArrowDown"
        ? 1
        : event.key === "ArrowUp"
          ? -1
          : event.key === "Home"
            ? -session.commits.length
            : session.commits.length;
    list?.scrollToIndex(session.move(delta));
  }
</script>

{#snippet commitRow(commit: CommitSummary)}
  <div
    class="commit"
    class:selected={oid === commit.oid}
    role="option"
    id="history-{commit.oid}"
    aria-selected={oid === commit.oid}
    data-oid={commit.oid}
    title={commit.oid}
  >
    <span class="subject">{commit.subject || "(no message)"}</span>
    <span class="author">{commit.author}</span>
    <span class="when">{timeAgo(commit.authoredAt)}</span>
    <span class="hash">{commit.shortOid}</span>
  </div>
{/snippet}

<section class="history" aria-label="History" bind:clientHeight={paneHeight}>
  <PaneHeader><span>History</span><RightSwitch /></PaneHeader>
  {#if !session}
    <EmptyState title="Select an available repository to browse its history." />
  {:else}
    <div class="toolbar">
      <select
        aria-label="History reference"
        value={session.reference}
        onchange={(event) => session.setReference(event.currentTarget.value)}
      >
        <option value="HEAD">Current checkout (HEAD)</option>
        {#if session.reference !== "HEAD" && !session.localBranches.some((name) => `refs/heads/${name}` === session.reference)}
          <option value={session.reference}>{session.reference.slice(11)} (missing)</option>
        {/if}
        {#each session.localBranches as name (name)}
          <option value="refs/heads/{name}">{name}</option>
        {/each}
      </select>
      <input
        bind:this={search}
        type="search"
        aria-label="Search history"
        placeholder="Search message, author or hash…"
        maxlength="4096"
        value={session.query}
        oninput={(event) => session.setQuery(event.currentTarget.value)}
      />
    </div>
    {#if session.stale}
      <div class="notice" role="status">
        History changed · <button
          type="button"
          class="btn"
          disabled={session.loading}
          onclick={() => void session.load(false, true)}>Refresh</button
        >
      </div>
    {/if}
    <div
      class="commits section"
      style:height="{session.commitsHeight}px"
      aria-busy={session.loading}
    >
      <VirtualList
        bind:this={list}
        items={session.commits}
        itemHeight={26}
        key={(c: CommitSummary) => c.oid}
        row={commitRow}
        bind:scrollTop={session.scrollTop}
        role="listbox"
        aria-label="History commits"
        tabindex={0}
        aria-activedescendant={oid && !outside ? `history-${oid}` : undefined}
        onclick={commitClick}
        onkeydown={commitKeydown}
      />
      {#if session.loading}<p class="message" role="status">Loading history…</p>
      {:else if !session.error && session.commits.length === 0}<p class="message">
          {session.query ? "No matching commits." : "This history has no commits."}
        </p>{/if}
      {#if session.error}
        <div class="message" role="alert">
          {session.error.message}
          {session.error.detail ?? ""}
          <button type="button" class="btn" onclick={() => void session.retry()}>Retry</button>
        </div>
      {/if}
      {#if session.cursor}
        {#if session.commits.length >= HISTORY_ROW_LIMIT}<p class="message">
            10,000 commits loaded. Narrow your search to browse further.
          </p>
        {:else}<button
            class="btn more"
            type="button"
            disabled={session.loading}
            onclick={() => void session.load(true)}>Load more</button
          >{/if}
      {/if}
    </div>
    <Splitter
      label="Resize history commits"
      value={session.commitsHeight}
      min={52}
      max={Math.max(52, paneHeight - session.filesHeight - 180)}
      step={26}
      onchange={(height: number) => {
        session.commitsHeight = height;
      }}
      onreset={() => {
        session.commitsHeight = 220;
      }}
    />
    {#if outside}<div class="notice">
        Showing linked commit {oid?.slice(0, 8)} outside the current list.
      </div>{/if}
    <div class="files section" style:height="{session.filesHeight}px">
      {#if review?.filesError}<EmptyState
          title="The files could not be loaded."
          hint={review.filesError.message}
        />
      {:else if review?.files}
        {#if review.files.files.length === 0}<EmptyState title="No file changes" />{:else}
          <div
            role="listbox"
            aria-label="Files in history commit"
            tabindex={0}
            aria-activedescendant={review.fileCursor
              ? `history-file-${review.fileCursor}`
              : undefined}
            onclick={(event) => fileClick(event)}
            oncontextmenu={(event) => fileClick(event, true)}
            onkeydown={(event) => {
              if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                event.preventDefault();
                review.moveFile(event.key === "ArrowDown" ? 1 : -1);
              }
            }}
          >
            {#each review.files.files as file (file.path)}
              <div class="file-row">
                <FileRow
                  id="history-file-{file.path}"
                  key={file.path}
                  path={file.path}
                  origPath={file.origPath}
                  kind={file.kind}
                  selected={review.fileCursor === file.path}
                  cursor={review.fileCursor === file.path}
                  additions={file.additions}
                  deletions={file.deletions}
                />
              </div>
            {/each}
          </div>
        {/if}
      {:else}<EmptyState
          title={oid ? "Loading files…" : "Select a commit to see its files."}
        />{/if}
    </div>
    <Splitter
      label="Resize history files"
      value={session.filesHeight}
      min={52}
      max={Math.max(52, paneHeight - session.commitsHeight - 180)}
      step={26}
      onchange={(height: number) => {
        session.filesHeight = height;
      }}
      onreset={() => {
        session.filesHeight = 130;
      }}
    />
    <section class="diff" aria-label="History file diff">
      {#if review?.cursorFile}
        <div class="diff-head">
          <span>{review.cursorFile.path}</span><span class="hash">{oid?.slice(0, 8)}</span>
          <button
            type="button"
            class="btn"
            disabled={review.diff.status !== "ready"}
            onclick={() => {
              if (target) comments.start(target, null);
            }}>Comment on file</button
          >
        </div>
      {/if}
      <DiffBody
        state={review?.cursorFile ? review.diff : { status: "idle" }}
        empty="Select a file to see its changes."
        onShowFull={() => void review?.loadDiff(true)}
        {target}
      />
    </section>
  {/if}
</section>

<style>
  .history {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background: var(--color-surface);
  }
  .toolbar {
    display: flex;
    flex: none;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }
  select {
    max-width: 45%;
    min-width: 0;
  }
  input {
    flex: 1;
    min-width: 0;
  }
  .section {
    flex: none;
    min-height: 0;
    overflow: auto;
  }
  .commits {
    display: flex;
    flex-direction: column;
  }
  .commits :global(.viewport) {
    flex: 1;
    min-height: 0;
  }
  input,
  select {
    padding: var(--space-2) var(--space-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-surface);
    color: var(--color-text);
    font: inherit;
  }
  .commit {
    height: 26px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 100px) 12ch 8ch;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-4);
  }
  .commit:hover {
    background: var(--color-row-hover);
  }
  .commit.selected {
    background: var(--color-row-selected);
  }
  .subject,
  .author {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .author,
  .when,
  .hash,
  .message {
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }
  .hash {
    font-family: var(--font-mono);
  }
  .when {
    text-align: right;
    white-space: nowrap;
  }
  .message,
  .notice {
    padding: var(--space-2) var(--space-4);
    margin: 0;
  }
  .notice {
    flex: none;
    background: var(--color-chrome);
    border-bottom: 1px solid var(--color-border);
  }
  .more {
    flex: none;
    align-self: center;
    margin: var(--space-2);
  }
  .file-row {
    height: 26px;
  }
  .diff {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 120px;
    overflow: hidden;
  }
  .diff-head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }
  .diff-head > span:first-child {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
