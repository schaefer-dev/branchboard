<script lang="ts">
  import { notices } from "../state/notices.svelte";
</script>

<div class="notices" aria-live="polite">
  {#each notices.items as notice (notice.id)}
    <div class="notice" class:error={notice.tone === "error"} role="status">
      <div class="text selectable">
        <p>{notice.message}</p>
        {#if notice.detail}
          <pre>{notice.detail}</pre>
        {/if}
      </div>
      <button
        class="icon-btn"
        aria-label="Dismiss"
        onclick={() => {
          notices.dismiss(notice.id);
        }}>×</button
      >
    </div>
  {/each}
</div>

<style>
  .notices {
    position: fixed;
    right: var(--space-5);
    bottom: calc(var(--statusbar-height) + var(--space-4));
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-width: 420px;
    z-index: 10;
  }

  .notice {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-3) var(--space-3) var(--space-4);
    border: 1px solid var(--color-border);
    border-left: 3px solid var(--color-text-muted);
    border-radius: var(--radius-sm);
    background: var(--color-surface);
    box-shadow: 0 6px 20px rgb(0 0 0 / 18%);
  }

  .notice.error {
    border-left-color: var(--color-danger);
  }

  .text {
    flex: 1;
    min-width: 0;
    margin-top: 2px;
  }

  p {
    margin: 0;
  }

  pre {
    max-height: 7.5em;
    margin: var(--space-2) 0 0;
    overflow: auto;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    white-space: pre-wrap;
  }
</style>
