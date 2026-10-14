<script lang="ts">
  interface Props {
    /** Current size in px of the element above the splitter. */
    value: number;
    min: number;
    max: number;
    /** Keyboard step in px. */
    step: number;
    label: string;
    onchange: (value: number) => void;
    /** Double-click: back to the automatic size. */
    onreset: () => void;
  }

  let { value, min, max, step, label, onchange, onreset }: Props = $props();

  let drag: { y: number; value: number } | null = null;

  function set(next: number) {
    onchange(Math.round(Math.max(min, Math.min(max, next))));
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    drag = { y: event.clientY, value };
  }

  function onPointerMove(event: PointerEvent) {
    if (drag) set(drag.value + event.clientY - drag.y);
  }

  function onKeydown(event: KeyboardEvent) {
    switch (event.key) {
      case "ArrowUp":
        set(value - step);
        break;
      case "ArrowDown":
        set(value + step);
        break;
      case "Home":
        set(min);
        break;
      case "End":
        set(max);
        break;
      default:
        return;
    }
    event.preventDefault();
  }
</script>

<!-- A separator with a value is a focusable widget (WAI-ARIA window splitter). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  role="separator"
  aria-orientation="horizontal"
  aria-label={label}
  aria-valuenow={Math.round(value)}
  aria-valuemin={Math.round(min)}
  aria-valuemax={Math.round(max)}
  tabindex={0}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={() => (drag = null)}
  onpointercancel={() => (drag = null)}
  ondblclick={onreset}
  onkeydown={onKeydown}
></div>

<style>
  .splitter {
    position: relative;
    flex: none;
    height: 1px;
    background: var(--color-border);
    cursor: row-resize;
    outline: none;
  }

  /* A larger hit area than the visible line. */
  .splitter::before {
    content: "";
    position: absolute;
    inset: -3px 0;
    z-index: 1;
  }

  .splitter:hover,
  .splitter:focus-visible,
  .splitter:active {
    background: var(--color-text-faint);
  }
</style>
