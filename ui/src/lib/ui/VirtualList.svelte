<script lang="ts" generics="T">
  import { onDestroy, type Snippet } from "svelte";
  import type { Attachment } from "svelte/attachments";
  import type { HTMLAttributes } from "svelte/elements";
  import { SvelteMap } from "svelte/reactivity";

  interface Props extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
    items: readonly T[];
    /** Height of every item; with `measure`, the height an item starts with. */
    itemHeight: number | ((item: T) => number);
    /** Items whose height is their rendered content's, e.g. wrapped text. */
    measure?: (item: T) => boolean;
    key: (item: T) => string;
    row: Snippet<[T, number]>;
    /** Width of the scrollable content, e.g. for long diff lines. */
    contentWidth?: string;
    overscan?: number;
    scrollTop?: number;
  }

  let {
    items,
    itemHeight,
    measure,
    key,
    row,
    contentWidth,
    overscan = 10,
    scrollTop = $bindable(0),
    class: className = "",
    ...rest
  }: Props = $props();

  let viewport = $state<HTMLDivElement>();
  $effect(() => {
    if (viewport && viewport.scrollTop !== scrollTop) viewport.scrollTop = scrollTop;
  });
  let height = $state(0);
  let width = $state(0);
  /** Rendered heights of measured items, by key. */
  const measured = new SvelteMap<string, number>();

  const fixed = $derived(typeof itemHeight === "number" && !measure ? itemHeight : null);
  /** Top of each item and, last, the total height; only for varying heights. */
  const offsets = $derived.by(() => {
    if (fixed !== null) return null;
    const tops = new Array<number>(items.length + 1);
    tops[0] = 0;
    items.forEach((item, index) => {
      tops[index + 1] = (tops[index] ?? 0) + heightOf(item);
    });
    return tops;
  });
  const total = $derived(offsets ? (offsets.at(-1) ?? 0) : items.length * (fixed ?? 0));

  const start = $derived(Math.max(0, indexAt(scrollTop) - overscan));
  const end = $derived(Math.min(items.length, indexAt(scrollTop + height) + 1 + overscan));
  const visible = $derived(items.slice(start, end));

  function heightOf(item: T): number {
    const known = measure?.(item) ? measured.get(key(item)) : undefined;
    if (known !== undefined) return known;
    return typeof itemHeight === "number" ? itemHeight : itemHeight(item);
  }

  function topOf(index: number): number {
    return offsets ? (offsets[index] ?? 0) : index * (fixed ?? 0);
  }

  /** The item at `y`, counted from the top of the content. */
  function indexAt(y: number): number {
    if (!offsets) return fixed ? Math.floor(y / fixed) : 0;
    let low = 0;
    let high = items.length - 1;
    while (low < high) {
      const middle = Math.ceil((low + high) / 2);
      if ((offsets[middle] ?? 0) <= y) low = middle;
      else high = middle - 1;
    }
    return Math.max(0, low);
  }

  const observer =
    typeof ResizeObserver === "undefined"
      ? null
      : new ResizeObserver((entries) => {
          for (const entry of entries) {
            const element = entry.target as HTMLElement;
            const itemKey = element.dataset.key;
            const size = Math.ceil(element.getBoundingClientRect().height);
            if (itemKey !== undefined && size > 0 && measured.get(itemKey) !== size) {
              measured.set(itemKey, size);
            }
          }
        });

  const observe: Attachment<HTMLElement> = (element) => {
    observer?.observe(element);
    return () => observer?.unobserve(element);
  };

  onDestroy(() => observer?.disconnect());

  /** Scrolls just enough to make row `index` fully visible. */
  export function scrollToIndex(index: number): void {
    const item = items[index];
    if (!viewport || item === undefined) return;
    const top = topOf(index);
    const bottom = top + heightOf(item);
    if (top < viewport.scrollTop) {
      viewport.scrollTop = top;
    } else if (bottom > viewport.scrollTop + viewport.clientHeight) {
      viewport.scrollTop = Math.min(top, bottom - viewport.clientHeight);
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
  onscroll={() => {
    scrollTop = viewport?.scrollTop ?? 0;
  }}
  bind:clientHeight={height}
  bind:clientWidth={width}
  class="viewport {className}"
  {...rest}
>
  <div
    class="content"
    style:height="{total}px"
    style:width={contentWidth}
    style:--viewport-width="{width}px"
  >
    {#each visible as item, offset (key(item))}
      {#if measure?.(item)}
        <div
          class="slot"
          data-key={key(item)}
          style:transform="translateY({topOf(start + offset)}px)"
          {@attach observe}
        >
          {@render row(item, start + offset)}
        </div>
      {:else}
        <div
          class="slot"
          style:height="{heightOf(item)}px"
          style:transform="translateY({topOf(start + offset)}px)"
        >
          {@render row(item, start + offset)}
        </div>
      {/if}
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
