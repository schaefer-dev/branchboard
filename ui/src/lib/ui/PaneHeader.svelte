<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title?: string;
    children?: Snippet;
  }

  let { title, children }: Props = $props();
</script>

<header class="pane-header">
  {#if title}
    <h2>{title}</h2>
  {/if}
  {@render children?.()}
</header>

<style>
  .pane-header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex: none;
    height: var(--pane-header-height);
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }

  h2 {
    margin: 0;
    font-size: var(--text-md);
    font-weight: 600;
  }
</style>
