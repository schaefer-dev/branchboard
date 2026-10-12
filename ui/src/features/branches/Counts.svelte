<script lang="ts">
  import type { AheadBehind } from "../../lib/api";

  interface Props {
    counts: AheadBehind;
    /** Accessible description, e.g. "than origin/development". */
    relation: string;
  }

  let { counts, relation }: Props = $props();

  const label = $derived(
    `${String(counts.ahead)} ahead, ${String(counts.behind)} behind ${relation}`,
  );
</script>

<span class="counts" title={label} aria-label={label}>
  <span class:zero={counts.ahead === 0}>↑{counts.ahead}</span>
  <span class:zero={counts.behind === 0}>↓{counts.behind}</span>
</span>

<style>
  .counts {
    display: inline-grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-3);
    font-variant-numeric: tabular-nums;
  }

  span span {
    min-width: 3ch;
    text-align: right;
  }

  .zero {
    color: var(--color-text-faint);
  }
</style>
