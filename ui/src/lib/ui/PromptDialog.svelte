<script lang="ts">
  import { prompts } from "./prompt.svelte";

  let dialog = $state<HTMLDialogElement>();
  let value = $state("");

  const current = $derived(prompts.current);
  const canConfirm = $derived(current?.allowEmpty === true || value.trim() !== "");

  $effect(() => {
    if (current && dialog && !dialog.open) {
      value = current.initial ?? "";
      dialog.showModal();
    } else if (!current && dialog?.open) {
      dialog.close();
    }
  });

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (canConfirm) prompts.settle(value);
  }
</script>

<dialog
  bind:this={dialog}
  class="modal"
  aria-labelledby="prompt-title"
  onclose={() => {
    prompts.settle(null);
  }}
>
  {#if current}
    <form onsubmit={submit}>
      <h2 id="prompt-title">{current.title}</h2>
      <label>
        <span>{current.label}</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="field"
          bind:value
          placeholder={current.placeholder}
          autofocus
          spellcheck="false"
          autocomplete="off"
        />
      </label>
      {#if current.hint}
        <p class="hint">{current.hint}</p>
      {/if}
      <div class="actions">
        <button
          type="button"
          class="btn"
          onclick={() => {
            prompts.settle(null);
          }}>Cancel</button
        >
        <button type="submit" class="btn btn-primary" disabled={!canConfirm}>
          {current.confirmLabel}
        </button>
      </div>
    </form>
  {/if}
</dialog>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    width: 360px;
    padding: var(--space-5);
  }

  h2 {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: 600;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .hint {
    margin: calc(var(--space-2) * -1) 0 0;
    color: var(--color-text-faint);
    font-size: var(--text-sm);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
  }

  button:disabled {
    opacity: 0.5;
  }
</style>
