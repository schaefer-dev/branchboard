<script lang="ts">
  interface Props {
    visible: boolean;
  }

  let { visible }: Props = $props();
</script>

{#if visible}
  <div class="overlay" aria-hidden="true">
    <p>Drop folders to add them as repositories</p>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: var(--space-3);
    display: grid;
    place-items: center;
    border: 2px dashed var(--color-accent);
    border-radius: 8px;
    background: var(--color-accent-soft);
    pointer-events: none;
    z-index: 20;
  }

  p {
    margin: 0;
    padding: var(--space-3) var(--space-5);
    border-radius: var(--radius-sm);
    background: var(--color-surface);
    font-weight: 500;
  }
</style>
