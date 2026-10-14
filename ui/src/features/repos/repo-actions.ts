// User-facing repository and group actions, shared by the sidebar, context
// menus, keyboard shortcuts and the work pane.

import type { Group, RepoView } from "../../lib/api";
import { confirmDestructive, pickFolders } from "../../lib/platform/dialogs";
import { showContextMenu, type MenuEntry } from "../../lib/platform/menu";
import { agents } from "../../lib/state/agents.svelte";
import { terminal } from "../../lib/state/terminal.svelte";
import { workspace } from "../../lib/state/workspace.svelte";
import { prompts } from "../../lib/ui/prompt.svelte";
import { openRepoInHerdr } from "../agents/open-in-herdr";
import { chooseMainline } from "../branches/branch-actions";
import { openRepoInEditor } from "../editor/open-in-editor";

function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

export async function addRepositories(): Promise<void> {
  const paths = await pickFolders("Add repositories");
  await workspace.addFolders(paths);
}

export async function renameRepo(repo: RepoView): Promise<void> {
  const name = await prompts.ask({
    title: "Rename repository",
    label: "Display name",
    initial: repo.customName ?? "",
    placeholder: folderName(repo.path),
    hint: "Leave empty to use the folder name.",
    confirmLabel: "Rename",
    allowEmpty: true,
  });
  if (name === null) return;
  await workspace.rename(repo.id, name.trim() || null);
}

export async function removeRepo(repo: RepoView): Promise<void> {
  const confirmed = await confirmDestructive(
    `Remove ${repo.name}?`,
    "It disappears from Branchboard. The folder on disk stays untouched.",
    "Remove",
  );
  if (confirmed) await workspace.remove(repo.id);
}

export async function locateRepo(repo: RepoView): Promise<void> {
  const [path] = await pickFolders(`Locate ${repo.name}`, false);
  if (path) await workspace.relocate(repo.id, path);
}

export async function newGroup(repo: RepoView | null = null): Promise<void> {
  const name = await prompts.ask({
    title: "New group",
    label: "Group name",
    confirmLabel: "Create group",
  });
  if (name !== null) await workspace.createGroup(name, repo?.id ?? null);
}

export async function renameGroup(group: Group): Promise<void> {
  const name = await prompts.ask({
    title: "Rename group",
    label: "Group name",
    initial: group.name,
    confirmLabel: "Rename",
  });
  if (name !== null) await workspace.renameGroup(group.id, name);
}

export async function deleteGroup(group: Group): Promise<void> {
  const confirmed = await confirmDestructive(
    `Delete group ${group.name}?`,
    "Its repositories move to Ungrouped.",
    "Delete group",
  );
  if (confirmed) await workspace.deleteGroup(group.id);
}

export function showRepoMenu(repo: RepoView): Promise<void> {
  const groups: MenuEntry[] = [...workspace.snapshot.groups]
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((group) => ({
      kind: "check",
      text: group.name,
      checked: repo.groupId === group.id,
      run: () => void workspace.moveToGroup(repo.id, group.id),
    }));

  return showContextMenu([
    {
      kind: "item",
      text: "Open in VS Code",
      enabled: !repo.missing,
      run: () => void openRepoInEditor(repo),
    },
    ...(agents.available || terminal.available
      ? [
          {
            kind: "item",
            text: terminal.available ? "Show in terminal" : "Open in herdr",
            enabled: !repo.missing,
            run: () => void openRepoInHerdr(repo),
          } as const,
        ]
      : []),
    { kind: "separator" },
    { kind: "item", text: "Rename…", run: () => void renameRepo(repo) },
    {
      kind: "item",
      text: repo.favorite ? "Remove from favorites" : "Add to favorites",
      run: () => void workspace.setFavorite(repo.id, !repo.favorite),
    },
    {
      kind: "submenu",
      text: "Move to group",
      items: [
        {
          kind: "check",
          text: "No group",
          checked: repo.groupId === null,
          run: () => void workspace.moveToGroup(repo.id, null),
        },
        ...(groups.length > 0 ? [{ kind: "separator" } as const, ...groups] : []),
        { kind: "separator" },
        { kind: "item", text: "New group…", run: () => void newGroup(repo) },
      ],
    },
    { kind: "item", text: "Compare branches with…", run: () => void chooseMainline(repo) },
    { kind: "separator" },
    ...(repo.missing
      ? [{ kind: "item", text: "Locate…", run: () => void locateRepo(repo) } as const]
      : []),
    { kind: "item", text: "Remove from Branchboard…", run: () => void removeRepo(repo) },
  ]);
}

export function showGroupMenu(group: Group): Promise<void> {
  return showContextMenu([
    { kind: "item", text: "Rename group…", run: () => void renameGroup(group) },
    { kind: "item", text: "Delete group…", run: () => void deleteGroup(group) },
  ]);
}

export function showSidebarMenu(): Promise<void> {
  return showContextMenu([
    { kind: "item", text: "Add repositories…", run: () => void addRepositories() },
    { kind: "item", text: "New group…", run: () => void newGroup() },
  ]);
}
