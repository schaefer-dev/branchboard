<script lang="ts">
  import { commands, type Command } from "../../lib/commands";
  import Picker from "../../lib/ui/Picker.svelte";

  interface Props {
    open: boolean;
  }

  let { open = $bindable() }: Props = $props();

  // Recomputed each time the palette opens: availability depends on state.
  const available = $derived(open ? commands.available() : []);
</script>

<Picker
  bind:open
  label="Command palette"
  placeholder="Type a command…"
  items={available}
  key={(command: Command) => command.id}
  fields={(command: Command) => [command.title]}
  choose={(command: Command) => void command.run()}
  emptyText="No matching commands"
>
  {#snippet row(command: Command)}
    <div class="command">
      <span class="name">{command.title}</span>
      {#if command.shortcut}
        <kbd>{commands.shortcutLabel(command.id)}</kbd>
      {/if}
    </div>
  {/snippet}
</Picker>

<style>
  .command {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-4);
  }

  kbd {
    color: var(--color-text-muted);
    font-family: var(--font-ui);
    font-size: var(--text-sm);
  }
</style>
