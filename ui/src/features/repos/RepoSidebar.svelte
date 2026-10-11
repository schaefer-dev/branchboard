<script lang="ts">
  import { workspace } from "../../lib/state/workspace.svelte";
  import EmptyState from "../../lib/ui/EmptyState.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import PaneHeader from "../../lib/ui/PaneHeader.svelte";
  import { commands } from "../../lib/commands";
  import RepoRow from "./RepoRow.svelte";
  import SectionHeader from "./SectionHeader.svelte";
  import {
    addRepositories,
    removeRepo,
    renameRepo,
    showGroupMenu,
    showRepoMenu,
    showSidebarMenu,
  } from "./repo-actions";

  let list = $state<HTMLElement>();

  /** Roving tab stop: the last focused item, else the active repository, else the first item. */
  let focusKey = $state<string | null>(null);

  const navKeys = $derived(
    workspace.sections.flatMap((section) => [
      ...(section.group ? [`group:${section.group.id}`] : []),
      ...(section.collapsed ? [] : section.repos.map((repo) => `repo:${repo.id}`)),
    ]),
  );
  const tabStop = $derived.by(() => {
    if (focusKey && navKeys.includes(focusKey)) return focusKey;
    const active = workspace.snapshot.activeRepoId;
    if (active && navKeys.includes(`repo:${active}`)) return `repo:${active}`;
    return navKeys[0] ?? null;
  });

  const addLabel = $derived(
    `Add repositories (${commands.shortcutLabel("repo.add") ?? ""})`.replace(" ()", ""),
  );

  function focusItem(key: string | undefined) {
    if (!key || !list) return;
    list.querySelector<HTMLElement>(`[data-nav="${CSS.escape(key)}"]`)?.focus();
  }

  function onFocusIn(event: FocusEvent) {
    if (event.target instanceof HTMLElement && event.target.dataset.nav) {
      focusKey = event.target.dataset.nav;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    const target = event.target instanceof HTMLElement ? event.target : null;
    const key = target?.dataset.nav;
    if (!key) return;
    const index = navKeys.indexOf(key);
    const repo = target.dataset.repoId ? workspace.repo(target.dataset.repoId) : undefined;
    const section = workspace.sections.find(
      (candidate) => candidate.group?.id === target.dataset.groupId,
    );

    switch (event.key) {
      case "ArrowDown":
        focusItem(navKeys[index + 1]);
        break;
      case "ArrowUp":
        focusItem(navKeys[index - 1]);
        break;
      case "Home":
        focusItem(navKeys[0]);
        break;
      case "End":
        focusItem(navKeys.at(-1));
        break;
      case "ArrowLeft":
      case "ArrowRight": {
        const group = section?.group;
        if (!group) return;
        const collapse = event.key === "ArrowLeft";
        if (group.collapsed !== collapse) void workspace.setGroupCollapsed(group.id, collapse);
        break;
      }
      case "F2":
        if (repo) void renameRepo(repo);
        else if (section?.group) void showGroupMenu(section.group);
        break;
      case "Delete":
      case "Backspace":
        if (!repo) return;
        void removeRepo(repo);
        break;
      case "ContextMenu":
        if (repo) void showRepoMenu(repo);
        else if (section?.group) void showGroupMenu(section.group);
        break;
      default:
        return;
    }
    event.preventDefault();
  }
</script>

<nav class="sidebar" aria-label="Repositories">
  <PaneHeader title="Repositories">
    <span class="spacer"></span>
    <button
      type="button"
      class="icon-btn"
      title={addLabel}
      aria-label="Add repositories"
      onclick={() => void addRepositories()}
    >
      <Icon name="plus" />
    </button>
  </PaneHeader>

  <!-- Keyboard handling is delegated from the focusable rows. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="list"
    bind:this={list}
    onkeydown={onKeydown}
    onfocusin={onFocusIn}
    oncontextmenu={(event) => {
      if (event.target === event.currentTarget) {
        event.preventDefault();
        void showSidebarMenu();
      }
    }}
  >
    {#if workspace.loaded && workspace.snapshot.repos.length === 0 && workspace.snapshot.groups.length === 0}
      <EmptyState
        title="No repositories yet"
        hint="Add a repository folder, or drop folders onto this window."
      />
      <div class="empty-action">
        <button type="button" class="btn" onclick={() => void addRepositories()}>
          Add repositories
        </button>
      </div>
    {/if}

    {#each workspace.sections as section (section.key)}
      {#if section.title}
        <SectionHeader
          {section}
          tabindex={section.group && tabStop === `group:${section.group.id}` ? 0 : -1}
        />
      {/if}
      {#if !section.collapsed}
        {#each section.repos as repo (repo.id)}
          <RepoRow
            {repo}
            summary={workspace.summaries.get(repo.id)}
            active={repo.id === workspace.snapshot.activeRepoId}
            tabindex={tabStop === `repo:${repo.id}` ? 0 : -1}
          />
        {:else}
          {#if section.group}
            <p class="section-empty">No repositories in this group</p>
          {/if}
        {/each}
      {/if}
    {/each}
  </div>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--color-chrome);
    border-right: 1px solid var(--color-border);
  }

  .spacer {
    flex: 1;
  }

  .list {
    flex: 1;
    min-height: 0;
    padding-bottom: var(--space-5);
    overflow-y: auto;
  }

  .list > :global(:first-child.header) {
    margin-top: var(--space-3);
  }

  .empty-action {
    padding: 0 var(--space-4);
  }

  .section-empty {
    margin: 0;
    padding: var(--space-1) var(--space-4) var(--space-2) calc(var(--space-4) + var(--space-1));
    color: var(--color-text-faint);
    font-size: var(--text-sm);
  }
</style>
