<script lang="ts">
  import type { RepoView } from "../../lib/api";
  import { workspace } from "../../lib/state/workspace.svelte";
  import Picker from "../../lib/ui/Picker.svelte";
  import { summaryText } from "./summary-text";

  interface Props {
    open: boolean;
  }

  let { open = $bindable() }: Props = $props();
</script>

<Picker
  bind:open
  label="Switch repository"
  placeholder="Switch to repository…"
  items={workspace.snapshot.repos}
  key={(repo: RepoView) => repo.id}
  fields={(repo: RepoView) => [repo.name, repo.path]}
  choose={(repo: RepoView) => void workspace.activate(repo.id)}
  emptyText="No matching repositories"
>
  {#snippet row(repo: RepoView)}
    <div class="repo">
      <span class="name">{repo.name}</span>
      <span class="branch">{summaryText(workspace.summaries.get(repo.id))}</span>
      <span class="path">{repo.path}</span>
    </div>
  {/snippet}
</Picker>

<style>
  .repo {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas:
      "name branch"
      "path path";
    column-gap: var(--space-4);
  }

  .name {
    grid-area: name;
    font-weight: 500;
  }

  .branch {
    grid-area: branch;
    color: var(--color-text-muted);
    font-size: var(--text-sm);
  }

  .path {
    grid-area: path;
    overflow: hidden;
    color: var(--color-text-faint);
    font-size: var(--text-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
