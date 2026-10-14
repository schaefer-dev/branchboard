<script lang="ts">
  import { isMac } from "../../lib/platform/os";
  import { commit } from "../../lib/state/commit.svelte";
  import { commitLabel, SUMMARY_HINT_LENGTH } from "../../lib/state/commit-model";

  /** The description grows with its text up to this many lines, then scrolls. */
  const MAX_DESCRIPTION_LINES = 6;

  let summaryField = $state<HTMLInputElement>();
  let descriptionField = $state<HTMLTextAreaElement>();

  const head = $derived(commit.head);
  const label = $derived(
    commit.busy
      ? commit.amend
        ? "Amending…"
        : "Committing…"
      : commitLabel(commit.stagedCount, commit.amend),
  );
  const shortcut = isMac ? "⌘↩" : "Ctrl+Enter";

  $effect(() => {
    if (commit.focusRequest > 0) summaryField?.focus();
  });

  // Grow the description with its text, including drafts the store sets.
  $effect(() => {
    if (descriptionField) fitToText(descriptionField, commit.description);
  });

  /** `text` is the field's value; passing it makes the effect re-run on edits. */
  function fitToText(field: HTMLTextAreaElement, text: string) {
    const lineHeight = parseFloat(getComputedStyle(field).lineHeight) || 19;
    field.style.height = "auto";
    const max = lineHeight * MAX_DESCRIPTION_LINES + 10;
    const height = text ? Math.min(field.scrollHeight + 2, max) : 0;
    field.style.height = height ? `${String(height)}px` : "";
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter") return;
    if (isMac ? event.metaKey : event.ctrlKey) {
      event.preventDefault();
      event.stopPropagation();
      void commit.commit();
    } else if (event.target === summaryField) {
      // A summary is one line; Enter continues with the description.
      event.preventDefault();
      descriptionField?.focus();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<form
  class="commit"
  aria-label="Commit"
  onkeydown={onKeydown}
  onsubmit={(event) => {
    event.preventDefault();
    void commit.commit();
  }}
>
  <div class="last">
    {#if head}
      <span class="last-label">Last commit</span>
      <span class="oid" title={head.oid}>{head.shortOid}</span>
      <span class="subject" title={head.subject}>{head.subject || "(no message)"}</span>
      {#if commit.canUndo}
        <button
          type="button"
          class="link"
          title="Take back this commit; its changes stay staged"
          onclick={() => void commit.undo()}>Undo</button
        >
      {/if}
    {:else}
      <span class="last-label">No commits yet</span>
    {/if}
  </div>

  <div class="summary">
    <input
      bind:this={summaryField}
      class="field"
      type="text"
      placeholder="Summary"
      aria-label="Commit summary"
      spellcheck="true"
      value={commit.summary}
      oninput={(event) => {
        commit.setSummary(event.currentTarget.value);
      }}
    />
    {#if commit.summary.length > SUMMARY_HINT_LENGTH}
      <span class="count" title="Characters in the summary">{commit.summary.length}</span>
    {/if}
  </div>
  <textarea
    bind:this={descriptionField}
    class="field"
    rows="2"
    placeholder="Description"
    aria-label="Commit description"
    spellcheck="true"
    value={commit.description}
    oninput={(event) => {
      commit.setDescription(event.currentTarget.value);
    }}></textarea>

  {#if commit.amend && head?.pushed}
    <p class="warning">This commit is already on a remote. Amending rewrites it.</p>
  {/if}

  <div class="actions">
    <label class="amend" class:disabled={!head}>
      <input
        type="checkbox"
        checked={commit.amend}
        disabled={!head || commit.busy}
        onchange={(event) => {
          commit.setAmend(event.currentTarget.checked);
        }}
      />
      Amend last commit
    </label>
    <button
      type="submit"
      class="btn btn-primary"
      disabled={!commit.canCommit}
      title="{label} ({shortcut})"
    >
      {label}
    </button>
  </div>

  {#if commit.error}
    <div class="error selectable" role="alert">
      <p class="error-message">{commit.error.message}</p>
      {#if commit.error.detail}
        <pre>{commit.error.detail}</pre>
      {/if}
    </div>
  {/if}
</form>

<style>
  .commit {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4) var(--space-4);
    border-top: 1px solid var(--color-border);
    background: var(--color-chrome);
  }

  .last {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    min-width: 0;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .last-label {
    flex: none;
    color: var(--color-text-faint);
  }

  .oid {
    flex: none;
    font-family: var(--font-mono);
  }

  .subject {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .link {
    flex: none;
    margin-left: auto;
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    font-weight: 500;
  }

  .link:hover {
    color: var(--color-text);
    text-decoration: underline;
  }

  .summary {
    position: relative;
  }

  .summary .field {
    padding-right: 4ch;
  }

  .count {
    position: absolute;
    top: 50%;
    right: var(--space-3);
    transform: translateY(-50%);
    color: var(--color-text-faint);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
  }

  textarea {
    min-height: 48px;
  }

  .warning {
    margin: 0;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .amend {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .amend.disabled {
    color: var(--color-text-faint);
  }

  .amend input {
    margin: 0;
    accent-color: var(--color-text);
  }

  .error {
    max-height: 160px;
    overflow-y: auto;
    padding: var(--space-3);
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-sm);
    background: var(--color-deleted-bg);
  }

  .error-message {
    margin: 0;
    color: var(--color-danger);
    font-size: var(--text-sm);
    font-weight: 600;
  }

  pre {
    margin: var(--space-2) 0 0;
    color: var(--color-text);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
