// Every user command with its shortcut. Shortcuts, the command palette and
// menus all run these.

import { commands } from "../../lib/commands";
import { isMac } from "../../lib/platform/os";
import { agents } from "../../lib/state/agents.svelte";
import { branches } from "../../lib/state/branches.svelte";
import { terminal } from "../../lib/state/terminal.svelte";
import { changes } from "../../lib/state/changes.svelte";
import { commit } from "../../lib/state/commit.svelte";
import { view } from "../../lib/state/view.svelte";
import { TERMINAL_FONT_SIZE, workspace } from "../../lib/state/workspace.svelte";
import { openRepoInHerdr } from "../agents/open-in-herdr";
import { chooseMainline } from "../branches/branch-actions";
import { openCurrentInEditor } from "../editor/open-in-editor";
import { addRepositories, newGroup, removeRepo, renameRepo } from "../repos/repo-actions";

interface Dialogs {
  openSwitcher: () => void;
  openPalette: () => void;
}

const terminalFontSize = () => workspace.snapshot.settings.terminalFontSize;
const settings = () => workspace.snapshot.settings;

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
      id: "history.show",
      title: "Show history",
      shortcut: { key: "4", mod: true },
      enabled: () => usableRepo() !== null,
      run: () => {
        view.showHistory();
      },
    }),
    commands.register({
      id: "editor.open",
      title: "Open in VS Code",
      shortcut: { key: "e", mod: true, shift: true },
      enabled: () => usableRepo() !== null,
      run: openCurrentInEditor,
    }),
    commands.register({
      id: "terminal.show",
      get title() {
        return terminal.available ? "Show terminal" : "Open in herdr";
      },
      shortcut: { key: "t", mod: true, shift: true },
      enabled: () => terminal.available || (agents.available && usableRepo() !== null),
      run: () => openRepoInHerdr(usableRepo()),
    }),
    commands.register({
      id: "agents.next",
      title: "Go to next agent that needs you",
      // Ctrl+J belongs to the terminal outside macOS (see `keyOwner`).
      shortcut: isMac ? { key: "j", mod: true } : { key: "j", mod: true, shift: true },
      enabled: () => agents.needYou.length > 0,
      run: () => agents.next(),
    }),
    commands.register({
      id: "sound.mute",
      get title() {
        return settings().soundMuted ? "Unmute agent sounds" : "Mute agent sounds";
      },
      run: () => workspace.setSoundMuted(!settings().soundMuted),
    }),
    commands.register({
      id: "sound.muteWhenWatching",
      get title() {
        return settings().muteWhenWatching
          ? "Play sounds for the agent on screen"
          : "Mute sounds for the agent on screen";
      },
      run: () => workspace.setMuteWhenWatching(!settings().muteWhenWatching),
    }),
    commands.register({
      id: "terminal.fontLarger",
      title: "Larger terminal font",
      shortcut: { key: "+", alternateKeys: ["="], mod: true },
      enabled: () => terminal.available,
      run: () => workspace.setTerminalFontSize(terminalFontSize() + 1),
    }),
    commands.register({
      id: "terminal.fontSmaller",
      title: "Smaller terminal font",
      shortcut: { key: "-", mod: true },
      enabled: () => terminal.available,
      run: () => workspace.setTerminalFontSize(terminalFontSize() - 1),
    }),
    commands.register({
      id: "terminal.fontReset",
      title: "Default terminal font size",
      shortcut: { key: "0", mod: true },
      enabled: () => terminal.available,
      run: () => workspace.setTerminalFontSize(TERMINAL_FONT_SIZE),
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
      id: "commit.focus",
      title: "Write commit message",
      shortcut: { key: "3", mod: true },
      enabled: () => usableRepo() !== null && changes.status !== null,
      run: () => {
        commit.focus();
      },
    }),
    commands.register({
      id: "commit.run",
      title: "Commit staged changes",
      enabled: () => usableRepo() !== null && changes.status !== null,
      run: () => {
        // Without a summary there is nothing to commit yet; go write one.
        if (commit.canCommit) void commit.commit();
        else commit.focus();
      },
    }),
    commands.register({
      id: "commit.amend",
      title: "Amend last commit",
      enabled: () => usableRepo() !== null && commit.head !== null && !commit.busy,
      run: () => {
        commit.setAmend(true);
        commit.focus();
      },
    }),
    commands.register({
      id: "commit.undo",
      title: "Undo last commit",
      enabled: () => usableRepo() !== null && commit.canUndo,
      run: () => commit.undo(),
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
