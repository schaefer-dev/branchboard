<script lang="ts">
  import type { SidebarSection } from "../../lib/state/sections";
  import { workspace } from "../../lib/state/workspace.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { showGroupMenu } from "./repo-actions";

  interface Props {
    section: SidebarSection;
    tabindex: number;
  }

  let { section, tabindex }: Props = $props();
</script>

{#if section.group}
  {@const group = section.group}
  <button
    type="button"
    class="header"
    data-nav={`group:${group.id}`}
    data-group-id={group.id}
    {tabindex}
    aria-expanded={!section.collapsed}
    onclick={() => void workspace.setGroupCollapsed(group.id, !section.collapsed)}
    oncontextmenu={(event) => {
      event.preventDefault();
      void showGroupMenu(group);
    }}
  >
    <span class="chevron" class:open={!section.collapsed}><Icon name="chevron" size={12} /></span>
    <span class="title">{section.title}</span>
    <span class="count">{section.repos.length}</span>
  </button>
{:else}
  <h3 class="header static">
    <span class="title">{section.title}</span>
  </h3>
{/if}

<style>
  .header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    margin: var(--space-4) 0 var(--space-1);
    padding: 2px var(--space-4) 2px var(--space-3);
    border: 0;
    background: none;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
    font-weight: 600;
    text-align: left;
  }

  .header.static {
    padding-left: calc(var(--space-4) + var(--space-1));
  }

  button.header:hover {
    color: var(--color-text);
  }

  .header:focus-visible {
    box-shadow: inset var(--focus-ring);
  }

  .chevron {
    display: inline-grid;
    transition: transform 120ms ease;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    color: var(--color-text-faint);
    font-weight: 400;
    font-variant-numeric: tabular-nums;
  }
</style>
