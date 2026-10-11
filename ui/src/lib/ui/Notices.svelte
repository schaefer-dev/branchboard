<script lang="ts">
  import { notices } from "../state/notices.svelte";
</script>

<div class="notices" aria-live="polite">
  {#each notices.items as notice (notice.id)}
    <div class="notice" class:error={notice.tone === "error"} role="status">
      <p class="selectable" title={notice.detail ?? undefined}>{notice.message}</p>
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

  p {
    flex: 1;
    margin: 2px 0 0;
  }
</style>
