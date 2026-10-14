<script lang="ts">
  import type { RepoView, SummaryState } from "../../lib/api";
  import { agents } from "../../lib/state/agents.svelte";
  import { workspace } from "../../lib/state/workspace.svelte";
  import AgentBadge from "../agents/AgentBadge.svelte";
  import { showRepoMenu } from "./repo-actions";
  import { summaryText } from "./summary-text";

  interface Props {
    repo: RepoView;
    summary: SummaryState | undefined;
    active: boolean;
    tabindex: number;
  }

  let { repo, summary, active, tabindex }: Props = $props();

  const dirty = $derived(summary?.status === "ready" && summary.summary.dirty);
  const missing = $derived(repo.missing || summary?.status === "missing");
  const detail = $derived(summary?.status === "failed" ? summary.message : repo.path);
  const repoAgents = $derived(agents.of(repo.id));
</script>

<button
  type="button"
  class="row"
  class:active
  class:missing
  data-nav={`repo:${repo.id}`}
  data-repo-id={repo.id}
  {tabindex}
  aria-current={active ? "true" : undefined}
  title={detail}
  onclick={() => void workspace.activate(repo.id)}
  oncontextmenu={(event) => {
    event.preventDefault();
    void showRepoMenu(repo);
  }}
>
  <span class="name">{repo.name}</span>
  {#if repoAgents}
    <span class="agents"><AgentBadge agents={repoAgents} /></span>
  {/if}
  {#if dirty}
    <span class="dirty" title="Uncommitted changes"></span>
  {/if}
  <span class="meta">{summaryText(missing ? { status: "missing" } : summary)}</span>
</button>

<style>
  .row {
    position: relative;
    display: grid;
    /* The dot column keeps its width, so agent badges line up. */
    grid-template-columns: minmax(0, 1fr) auto 7px;
    grid-template-areas:
      "name agents dirty"
      "meta meta meta";
    align-items: center;
    column-gap: var(--space-3);
    width: 100%;
    padding: 5px var(--space-4) 5px calc(var(--space-4) + var(--space-1));
    border: 0;
    background: none;
    text-align: left;
    line-height: var(--leading-tight);
  }

  .row:hover {
    background: var(--color-row-hover);
  }

  .row.active {
    background: var(--color-row-selected);
  }

  .row.active::before {
    content: "";
    position: absolute;
    inset: 4px auto 4px 0;
    width: 3px;
    border-radius: 0 2px 2px 0;
    background: var(--color-accent);
  }

  .row:focus-visible {
    box-shadow: inset var(--focus-ring);
  }

  .name {
    grid-area: name;
    overflow: hidden;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    grid-area: meta;
    min-height: 1.25em;
    overflow: hidden;
    color: var(--color-text-muted);
    font-size: var(--text-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .agents {
    display: flex;
    grid-area: agents;
  }

  .dirty {
    grid-area: dirty;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--color-modified);
  }

  .missing .name,
  .missing .meta {
    color: var(--color-text-faint);
  }
</style>
