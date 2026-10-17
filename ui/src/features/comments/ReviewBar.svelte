<script lang="ts">
  import type { RepoId } from "../../lib/api";
  import { showContextMenu } from "../../lib/platform/menu";
  import { agents } from "../../lib/state/agents.svelte";
  import { comments } from "../../lib/state/comments.svelte";
  import { commentPlace } from "../../lib/state/comments-model";
  import { notices } from "../../lib/state/notices.svelte";
  import { workspace } from "../../lib/state/workspace.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { agentName } from "../agents/agent-text";
  import { agentChoice, countsLabel } from "./comment-text";

  const repoId = $derived(workspace.activeRepo?.id ?? null);
  const all = $derived(comments.of(repoId));
  const pending = $derived(all.filter((comment) => comment.status === "pending").length);
  const sent = $derived(all.length - pending);
  const candidates = $derived(repoId && agents.available ? agents.candidates(repoId) : []);

  let open = $state(false);

  async function send(repoId: RepoId) {
    const [only, ...others] = candidates;
    if (!only) return;
    if (others.length === 0) {
      if (only.state === "blocked" || only.state === "unknown") {
        notices.show(
          "info",
          `${agentName(only.agent)} is not ready for a review. Check its terminal first.`,
        );
      } else {
        await comments.pasteTo(repoId, only.paneId);
      }
      return;
    }
    await showContextMenu(
      candidates.map((agent) => ({
        kind: "item",
        text: agentChoice(agent),
        // Pasted text could answer the question the agent is asking.
        enabled: agent.state !== "blocked" && agent.state !== "unknown",
        run: () => void comments.pasteTo(repoId, agent.paneId),
      })),
    );
  }
</script>

{#if repoId && all.length > 0}
  <section class="review" aria-label="Review comments">
    {#if open}
      <ul class="list selectable" aria-label="Comments">
        {#each all as comment (comment.id)}
          <li class="item" class:sent={comment.status === "sent"}>
            <button
              type="button"
              class="place"
              title="Show this comment"
              onclick={() => {
                comments.reveal(comment);
              }}
            >
              {commentPlace(comment)}
            </button>
            {#if comment.outdated}<span class="tag">Outdated</span>{/if}
            <span class="tag">{comment.status === "sent" ? "Sent" : "Pending"}</span>
            <span class="body">{comment.body}</span>
            <button type="button" class="link" onclick={() => comments.remove(comment.id)}>
              {comment.status === "sent" ? "Resolve" : "Delete"}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
    <div class="bar">
      <button
        type="button"
        class="toggle"
        aria-expanded={open}
        title={open ? "Hide the comments" : "Show all comments"}
        onclick={() => (open = !open)}
      >
        <Icon name="comment" />
        <span>{countsLabel(pending, sent)}</span>
      </button>
      <span class="spacer"></span>
      {#if candidates.length > 0}
        <button
          type="button"
          class="btn"
          disabled={pending === 0 || comments.busy}
          onclick={() => void comments.copy(repoId)}>Copy review</button
        >
        <button
          type="button"
          class="btn btn-primary"
          disabled={pending === 0 || comments.busy}
          title="Paste the pending comments into the agent's input"
          onclick={() => void send(repoId)}>Send review</button
        >
      {:else}
        <button
          type="button"
          class="btn btn-primary"
          disabled={pending === 0 || comments.busy}
          title="Copy the pending comments as one prompt"
          onclick={() => void comments.copy(repoId)}>Copy review</button
        >
      {/if}
    </div>
  </section>
{/if}

<style>
  .review {
    border-top: 1px solid var(--color-border);
    background: var(--color-chrome);
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
  }

  .toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .toggle:hover {
    color: var(--color-text);
  }

  .spacer {
    flex: 1;
  }

  .list {
    max-height: 180px;
    margin: 0;
    padding: var(--space-2) 0;
    overflow-y: auto;
    border-bottom: 1px solid var(--color-border);
    list-style: none;
  }

  .item {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    padding: var(--space-1) var(--space-4);
    font-size: var(--text-sm);
  }

  .item.sent {
    color: var(--color-text-muted);
  }

  .place {
    flex: none;
    max-width: 50%;
    overflow: hidden;
    padding: 0;
    border: 0;
    background: none;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .place:hover {
    text-decoration: underline;
  }

  .tag {
    flex: none;
    color: var(--color-text-faint);
    font-size: var(--text-xs);
  }

  .body {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .link {
    flex: none;
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .link:hover {
    color: var(--color-text);
    text-decoration: underline;
  }
</style>
