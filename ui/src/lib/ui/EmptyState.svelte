<script lang="ts">
  interface Props {
    title: string;
    hint?: string;
  }

  let { title, hint }: Props = $props();
</script>

<div class="empty">
  <p class="title">{title}</p>
  {#if hint}
    <p class="hint">{hint}</p>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-5) var(--space-4);
  }

  p {
    margin: 0;
    max-width: 36ch;
  }

  .title {
    color: var(--color-text-muted);
  }

  .hint {
    color: var(--color-text-faint);
    font-size: var(--text-sm);
  }
</style>
