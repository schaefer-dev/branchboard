// Every user command with its shortcut. Shortcuts, the command palette and
// menus all run these.

import { commands } from "../../lib/commands";
import { branches } from "../../lib/state/branches.svelte";
import { changes } from "../../lib/state/changes.svelte";
import { view } from "../../lib/state/view.svelte";
import { workspace } from "../../lib/state/workspace.svelte";
import { chooseMainline } from "../branches/branch-actions";
import { openCurrentInEditor } from "../editor/open-in-editor";
import { addRepositories, newGroup, removeRepo, renameRepo } from "../repos/repo-actions";

interface Dialogs {
  openSwitcher: () => void;
  openPalette: () => void;
}

/** The active repository, if it can be worked with. */
const usableRepo = () => {
  const repo = workspace.activeRepo;
  return repo && !repo.missing ? repo : null;
};

export function registerAppCommands(dialogs: Dialogs): () => void {
  const unregister = [
    commands.register({
      id: "palette.open",
      title: "Show all commands",
      shortcut: { key: "k", mod: true },
      run: dialogs.openPalette,
    }),
    commands.register({
      id: "repo.switch",
      title: "Switch repository…",
      shortcut: { key: "p", mod: true },
      enabled: () => workspace.snapshot.repos.length > 0,
      run: dialogs.openSwitcher,
    }),
    commands.register({
      id: "repo.add",
      title: "Add repositories…",
      shortcut: { key: "o", mod: true },
      run: addRepositories,
    }),
    commands.register({
      id: "view.changes",
      title: "Focus changes",
      shortcut: { key: "1", mod: true },
      enabled: () => usableRepo() !== null,
      run: () => {
        view.focus("changes");
      },
    }),
    commands.register({
      id: "view.branches",
      title: "Focus branches",
      shortcut: { key: "2", mod: true },
      enabled: () => usableRepo() !== null,
      run: () => {
        view.focus("branches");
      },
    }),
    commands.register({
      id: "repo.fetch",
      title: "Fetch",
      shortcut: { key: "f", mod: true, shift: true },
      enabled: () => usableRepo() !== null,
      run: () => branches.fetch(),
    }),
    commands.register({
      id: "editor.open",
      title: "Open in VS Code",
      shortcut: { key: "e", mod: true, shift: true },
      enabled: () => usableRepo() !== null,
      run: openCurrentInEditor,
    }),
    commands.register({
      id: "changes.stageAll",
      title: "Stage all changes",
      enabled: () => (changes.status?.unstaged.length ?? 0) > 0,
      run: () => changes.stageAll(),
    }),
    commands.register({
      id: "changes.unstageAll",
      title: "Unstage all changes",
      enabled: () => (changes.status?.staged.length ?? 0) > 0,
      run: () => changes.unstageAll(),
    }),
    commands.register({
      id: "branches.compare",
      title: "Compare branches with…",
      enabled: () => usableRepo() !== null,
      run: () => {
        const repo = usableRepo();
        if (repo) void chooseMainline(repo);
      },
    }),
    commands.register({
      id: "repo.rename",
      title: "Rename repository…",
      enabled: () => workspace.activeRepo !== null,
      run: () => {
        if (workspace.activeRepo) void renameRepo(workspace.activeRepo);
      },
    }),
    commands.register({
      id: "repo.remove",
      title: "Remove repository from Branchboard…",
      enabled: () => workspace.activeRepo !== null,
      run: () => {
        if (workspace.activeRepo) void removeRepo(workspace.activeRepo);
      },
    }),
    commands.register({
      id: "group.new",
      title: "New group…",
      run: () => newGroup(),
    }),
  ];
  return () => {
    for (const fn of unregister) fn();
  };
}
