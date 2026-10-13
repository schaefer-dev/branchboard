<script lang="ts">
  import type { FileDiff } from "../../lib/api";
  import VirtualList from "../../lib/ui/VirtualList.svelte";
  import { maxLineWidth, unifiedRows, type DiffRow } from "./diff-rows";
  import { segments } from "./segments";

  interface Props {
    diff: FileDiff;
  }

  let { diff }: Props = $props();

  const ROW_HEIGHT = 20;
  /** Two line number columns, the sign column and padding, in `ch`. */
  const GUTTER_CH = 16;

  const rows = $derived(unifiedRows(diff));
  const width = $derived(`calc(${String(maxLineWidth(diff) + GUTTER_CH)}ch + 32px)`);
  const sign = { context: " ", added: "+", removed: "-" } as const;
</script>

<VirtualList
  class="diff-view selectable"
  items={rows}
  itemHeight={ROW_HEIGHT}
  key={(row: DiffRow) => row.key}
  contentWidth={width}
  tabindex={0}
  aria-label="Diff"
>
  {#snippet row(row: DiffRow)}
    {#if row.kind === "hunk"}
      <div class="line hunk">
        <span class="num"></span><span class="num"></span>
        <span class="text">{row.header}</span>
      </div>
    {:else}
      <div class="line {row.line.kind}">
        <span class="num">{row.line.oldLine ?? ""}</span>
        <span class="num">{row.line.newLine ?? ""}</span>
        <span class="sign">{sign[row.line.kind]}</span>
        <span class="text"
          >{#each segments(row.line.text, row.line.tokens) as segment, index (index)}{#if segment.kind}<span
                class="tok tok-{segment.kind}">{segment.text}</span
              >{:else}{segment.text}{/if}{/each}</span
        >
        {#if row.line.noNewline}
          <span class="eof" title="No newline at end of file">⏎̸</span>
        {/if}
      </div>
    {/if}
  {/snippet}
</VirtualList>

<style>
  :global(.diff-view) {
    height: 100%;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    outline: none;
  }

  .line {
    display: flex;
    height: 100%;
    line-height: 20px;
    white-space: pre;
    tab-size: 4;
  }

  .num {
    flex: none;
    width: 6ch;
    padding-right: 1ch;
    color: var(--color-text-faint);
    text-align: right;
    user-select: none;
  }

  .sign {
    flex: none;
    width: 2ch;
    padding-left: 0.5ch;
    color: var(--color-text-faint);
    user-select: none;
  }

  .text {
    flex: 1;
  }

  .added {
    background: var(--color-added-bg);
  }

  .added .sign {
    color: var(--color-added);
  }

  .removed {
    background: var(--color-deleted-bg);
  }

  .removed .sign {
    color: var(--color-deleted);
  }

  .hunk {
    background: var(--color-sunken);
    color: var(--color-text-muted);
  }

  .hunk .text {
    padding-left: 2.5ch;
  }

  .tok-comment {
    color: var(--syntax-comment);
    font-style: italic;
  }

  .tok-keyword {
    color: var(--syntax-keyword);
  }

  .tok-string {
    color: var(--syntax-string);
  }

  .tok-number {
    color: var(--syntax-number);
  }

  .tok-function {
    color: var(--syntax-function);
  }

  .tok-type {
    color: var(--syntax-type);
  }

  .tok-tag {
    color: var(--syntax-tag);
  }

  .tok-attribute {
    color: var(--syntax-attribute);
  }

  .eof {
    padding-left: 1ch;
    color: var(--color-text-faint);
  }
</style>
