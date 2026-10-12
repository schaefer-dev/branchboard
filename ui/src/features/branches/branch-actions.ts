// Branch-related actions shared by the branch view, menus and shortcuts.

import type { RepoView } from "../../lib/api";
import { api } from "../../lib/api";
import { showContextMenu } from "../../lib/platform/menu";
import { branches } from "../../lib/state/branches.svelte";
import { notices } from "../../lib/state/notices.svelte";
import { workspace } from "../../lib/state/workspace.svelte";
import { prompts } from "../../lib/ui/prompt.svelte";

const DEFAULT_MAINLINE = "origin/development";
const FETCH_INTERVALS = [5, 10, 30, 60] as const;

export async function chooseMainline(repo: RepoView): Promise<void> {
  const mainline = await prompts.ask({
    title: "Compare branches with",
    label: "Branch",
    initial: repo.mainline,
    placeholder: DEFAULT_MAINLINE,
    hint: `A local or remote branch, e.g. origin/main. Leave empty for ${DEFAULT_MAINLINE}.`,
    confirmLabel: "Compare",
    allowEmpty: true,
  });
  if (mainline === null) return;
  try {
    workspace.snapshot = await api.setMainline(repo.id, mainline.trim() || null);
    await branches.reload();
  } catch (error) {
    notices.error(error);
  }
}

async function setFetchInterval(minutes: number): Promise<void> {
  try {
    workspace.snapshot = await api.setFetchInterval(minutes);
  } catch (error) {
    notices.error(error);
  }
}

export function showBranchOptions(repo: RepoView): Promise<void> {
  const current = workspace.snapshot.settings.fetchIntervalMinutes;
  return showContextMenu([
    { kind: "item", text: "Compare branches with…", run: () => void chooseMainline(repo) },
    { kind: "separator" },
    {
      kind: "submenu",
      text: "Fetch in the background",
      items: [
        ...FETCH_INTERVALS.map((minutes) => ({
          kind: "check" as const,
          text: `Every ${String(minutes)} minutes`,
          checked: current === minutes,
          run: () => void setFetchInterval(minutes),
        })),
        { kind: "separator" },
        { kind: "check", text: "Off", checked: current === 0, run: () => void setFetchInterval(0) },
      ],
    },
  ]);
}

export function showBranchMenu(name: string): Promise<void> {
  const isCurrent = name === branches.current?.name;
  return showContextMenu([
    {
      kind: "item",
      text: isCurrent ? "Current branch" : `Switch to ${name}`,
      enabled: !isCurrent,
      run: () => void branches.switchTo(name),
    },
  ]);
}
