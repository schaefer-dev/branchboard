<script lang="ts">
  import { untrack } from "svelte";

  import type { FileDiff } from "../../lib/api";
  import { comments, sameTarget } from "../../lib/state/comments.svelte";
  import {
    byPlace,
    locate,
    selectLines,
    type CommentTarget,
    type LineRange,
  } from "../../lib/state/comments-model";
  import VirtualList from "../../lib/ui/VirtualList.svelte";
  import CommentCard from "../comments/CommentCard.svelte";
  import CommentEditor from "../comments/CommentEditor.svelte";
  import {
    diffLines,
    FILE_ANNOTATIONS,
    maxLineWidth,
    unifiedRows,
    type Annotation,
    type DiffRow,
  } from "./diff-rows";
  import { segments } from "./segments";

  interface Props {
    diff: FileDiff;
    /** Where comments on this diff belong; `null` shows none and offers none. */
    target?: CommentTarget | null;
  }

  let { diff, target = null }: Props = $props();

  const ROW_HEIGHT = 20;
  /** Height of a comment or the editor until it is measured. */
  const ANNOTATION_HEIGHT = 80;
  /** Two line number columns, the sign column and padding, in `ch`. */
  const GUTTER_CH = 16;

  let list = $state<{ scrollToIndex: (index: number) => void }>();

  const lines = $derived(diffLines(diff));
  /** The hunk of each line. */
  const hunks = $derived(diff.hunks.flatMap((hunk, h) => hunk.lines.map(() => h)));
  const editingId = $derived(comments.draft?.commentId ?? null);
  const newDraft = $derived(
    target &&
      comments.draft &&
      !comments.draft.commentId &&
      sameTarget(comments.draft.target, target)
      ? comments.draft
      : null,
  );

  /** Comments (from any diff of the file) whose lines are in this diff, and the editor. */
  const annotations = $derived.by(() => {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- rebuilt on every change, not state
    const result = new Map<number, Annotation[]>();
    if (!target) return result;
    const add = (index: number, annotation: Annotation) => {
      const existing = result.get(index);
      if (existing) existing.push(annotation);
      else result.set(index, [annotation]);
    };
    const indexOf = (range: LineRange | null) =>
      range ? (locate(lines, range)?.last ?? null) : FILE_ANNOTATIONS;
    for (const comment of comments.forFile(target.repoId, target.path).sort(byPlace)) {
      const index = indexOf(comment.lines);
      if (index === null) continue;
      add(index, comment.id === editingId ? { kind: "editor" } : { kind: "comment", comment });
    }
    if (newDraft) {
      const index = indexOf(newDraft.lines);
      if (index !== null) add(index, { kind: "editor" });
    }
    return result;
  });

  const rows = $derived(unifiedRows(diff, annotations));
  const width = $derived(`calc(${String(maxLineWidth(diff) + GUTTER_CH)}ch + 32px)`);
  const sign = { context: " ", added: "+", removed: "-" } as const;

  const isAnnotation = (row: DiffRow) => row.kind === "comment" || row.kind === "editor";

  // The diff's current lines tell whether comments written in it are outdated.
  $effect(() => {
    const current = target;
    const all = lines;
    if (current) {
      untrack(() => {
        comments.check(current, all);
      });
    }
  });

  // A comment asked to be shown, once its diff is here.
  $effect(() => {
    const request = comments.revealRequest;
    if (!request) return;
    const index = rows.findIndex((row) => row.key === `c:${request.id}`);
    if (index === -1) return;
    list?.scrollToIndex(index);
    untrack(() => {
      comments.revealRequest = null;
    });
  });

  // A new editor scrolls into view once.
  let shownDraft: object | null = null;
  $effect(() => {
    const draft = comments.draft;
    if (!draft || draft === shownDraft) return;
    const index = rows.findIndex((row) => row.kind === "editor");
    if (index === -1) return;
    shownDraft = draft;
    list?.scrollToIndex(index);
  });

  /** Lines picked with the gutter button: from where the mouse went down to where it is. */
  let picking = $state<{ anchor: number; current: number } | null>(null);
  /** Where the last pick started; Shift-click extends from it. */
  let lastAnchor: number | null = null;
  const picked = $derived(
    picking
      ? {
          from: Math.min(picking.anchor, picking.current),
          to: Math.max(picking.anchor, picking.current),
        }
      : null,
  );

  /** `index`, moved back into the hunk of `anchor`: a comment covers one hunk. */
  function withinHunk(anchor: number, index: number): number {
    let clamped = index;
    while (hunks[clamped] !== hunks[anchor]) clamped += clamped > anchor ? -1 : 1;
    return clamped;
  }

  function startPick(event: MouseEvent, index: number) {
    if (event.button !== 0) return;
    event.preventDefault();
    const anchor =
      event.shiftKey && lastAnchor !== null && hunks[lastAnchor] === hunks[index]
        ? lastAnchor
        : index;
    picking = { anchor, current: index };
  }

  function movePick(event: MouseEvent) {
    if (!picking) return;
    const element = event.target instanceof Element ? event.target.closest("[data-index]") : null;
    const index = element instanceof HTMLElement ? Number(element.dataset.index) : NaN;
    if (!Number.isNaN(index)) picking.current = withinHunk(picking.anchor, index);
  }

  function endPick() {
    if (!picking || !picked) return;
    const { from, to } = picked;
    lastAnchor = picking.anchor;
    picking = null;
    const range = selectLines(lines.slice(from, to + 1));
    if (target && range) comments.start(target, range);
  }
</script>

<svelte:window onmouseup={endPick} />

<VirtualList
  bind:this={list}
  class="diff-view selectable{picking ? ' picking' : ''}"
  items={rows}
  itemHeight={annotations.size > 0
    ? (row: DiffRow) => (isAnnotation(row) ? ANNOTATION_HEIGHT : ROW_HEIGHT)
    : ROW_HEIGHT}
  measure={annotations.size > 0 ? isAnnotation : undefined}
  key={(row: DiffRow) => row.key}
  contentWidth={width}
  tabindex={0}
  aria-label="Diff"
  onmouseover={movePick}
>
  {#snippet row(row: DiffRow)}
    {#if row.kind === "hunk"}
      <div class="line hunk">
        <span class="num"></span><span class="num"></span>
        <span class="text">{row.header}</span>
      </div>
    {:else if row.kind === "line"}
      <div
        class="line {row.line.kind}"
        class:picked={picked !== null && row.index >= picked.from && row.index <= picked.to}
        data-index={row.index}
      >
        {#if target}
          <button
            type="button"
            class="add"
            tabindex={-1}
            aria-label="Comment on this line"
            title="Comment on this line. Drag or Shift-click for several lines."
            onmousedown={(event) => {
              startPick(event, row.index);
            }}>+</button
          >
        {/if}
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
    {:else if row.kind === "comment"}
      <div class="annotation"><CommentCard comment={row.comment} /></div>
    {:else if comments.draft}
      <div class="annotation"><CommentEditor draft={comments.draft} /></div>
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

  :global(.diff-view.picking) {
    user-select: none;
  }

  .line {
    position: relative;
    display: flex;
    height: 100%;
    line-height: 20px;
    white-space: pre;
    tab-size: 4;
  }

  .add {
    position: absolute;
    top: 2px;
    left: 2px;
    display: none;
    width: 16px;
    height: 16px;
    padding: 0;
    border: 1px solid var(--color-border);
    border-radius: 3px;
    background: var(--color-surface);
    color: var(--color-text-muted);
    font-family: var(--font-ui);
    font-size: 13px;
    line-height: 13px;
  }

  .line:hover .add {
    display: block;
  }

  .add:hover {
    border-color: var(--color-text-muted);
    color: var(--color-text);
  }

  .line.picked {
    box-shadow: inset 3px 0 0 var(--color-accent);
    background: var(--color-accent-soft);
  }

  .annotation {
    position: sticky;
    left: 0;
    width: var(--viewport-width, 100%);
    padding: var(--space-2) var(--space-4) var(--space-3) 12ch;
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
