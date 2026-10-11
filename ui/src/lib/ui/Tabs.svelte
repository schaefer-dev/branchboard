<script lang="ts" generics="T extends string">
  interface Tab {
    id: T;
    label: string;
  }

  interface Props {
    tabs: readonly Tab[];
    selected: T;
    label: string;
  }

  let { tabs, selected = $bindable(), label }: Props = $props();

  const buttons: HTMLButtonElement[] = [];

  function onKeydown(event: KeyboardEvent, index: number) {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    const next = (index + step + tabs.length) % tabs.length;
    const tab = tabs[next];
    if (tab) {
      selected = tab.id;
      buttons[next]?.focus();
    }
  }
</script>

<div class="tabs" role="tablist" aria-label={label}>
  {#each tabs as tab, index (tab.id)}
    <button
      bind:this={buttons[index]}
      role="tab"
      type="button"
      aria-selected={tab.id === selected}
      tabindex={tab.id === selected ? 0 : -1}
      onclick={() => (selected = tab.id)}
      onkeydown={(event) => {
        onKeydown(event, index);
      }}
    >
      {tab.label}
    </button>
  {/each}
</div>

<style>
  .tabs {
    display: flex;
    align-self: stretch;
    gap: var(--space-4);
  }

  button {
    position: relative;
    padding: 0 var(--space-1);
    border: 0;
    background: none;
    color: var(--color-text-muted);
    font-weight: 500;
    cursor: default;
  }

  button[aria-selected="true"] {
    color: var(--color-text);
  }

  button[aria-selected="true"]::after {
    content: "";
    position: absolute;
    inset: auto 0 -1px;
    height: 2px;
    background: var(--color-accent);
  }
</style>
