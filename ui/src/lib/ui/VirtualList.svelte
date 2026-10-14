<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  interface Props extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
    items: readonly T[];
    itemHeight: number;
    key: (item: T) => string;
    row: Snippet<[T, number]>;
    /** Width of the scrollable content, e.g. for long diff lines. */
    contentWidth?: string;
    overscan?: number;
  }

  let {
    items,
    itemHeight,
    key,
    row,
    contentWidth,
    overscan = 10,
    class: className = "",
    ...rest
  }: Props = $props();

  let viewport = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let height = $state(0);

  const start = $derived(Math.max(0, Math.floor(scrollTop / itemHeight) - overscan));
  const end = $derived(
    Math.min(items.length, Math.ceil((scrollTop + height) / itemHeight) + overscan),
  );
  const visible = $derived(items.slice(start, end));

  /** Scrolls just enough to make row `index` fully visible. */
  export function scrollToIndex(index: number): void {
    if (!viewport) return;
    const top = index * itemHeight;
    if (top < viewport.scrollTop) {
      viewport.scrollTop = top;
    } else if (top + itemHeight > viewport.scrollTop + viewport.clientHeight) {
      viewport.scrollTop = top + itemHeight - viewport.clientHeight;
    }
  }

  export function scrollToTop(): void {
    if (viewport) viewport.scrollTop = 0;
  }

  export function focus(): void {
    viewport?.focus();
  }
</script>

<div
  bind:this={viewport}
  bind:clientHeight={height}
  class="viewport {className}"
  onscroll={() => {
    scrollTop = viewport?.scrollTop ?? 0;
  }}
  {...rest}
>
  <div class="content" style:height="{items.length * itemHeight}px" style:width={contentWidth}>
    {#each visible as item, offset (key(item))}
      <div
        class="slot"
        style:height="{itemHeight}px"
        style:transform="translateY({(start + offset) * itemHeight}px)"
      >
        {@render row(item, start + offset)}
      </div>
    {/each}
  </div>
</div>

<style>
  .viewport {
    position: relative;
    overflow: auto;
    contain: strict;
  }

  .content {
    position: relative;
    min-width: 100%;
  }

  .slot {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    min-width: 100%;
  }
</style>
