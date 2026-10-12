// Actions on changed files, shared by keyboard handling and context menus.

import { confirmDestructive } from "../../lib/platform/dialogs";
import { showContextMenu } from "../../lib/platform/menu";
import { changes } from "../../lib/state/changes.svelte";
import type { ChangeItem } from "../../lib/state/changes-model";

const count = (n: number) => (n === 1 ? "1 file" : `${String(n)} files`);

export async function discardItems(items: ChangeItem[]): Promise<void> {
  const unstaged = items.filter((item) => item.area === "unstaged");
  const discardable = unstaged.filter((item) => item.change.kind !== "conflicted");
  if (discardable.length === 0) return;

  const untracked = discardable.filter((item) => item.change.kind === "untracked").length;
  const title =
    discardable.length === 1 && discardable[0]
      ? `Discard changes to ${discardable[0].change.path}?`
      : `Discard changes to ${count(discardable.length)}?`;
  const details = [
    untracked < discardable.length &&
      "Tracked files are reset to their staged or committed state. This can't be undone.",
    untracked > 0 && "Untracked files are moved to the trash.",
  ].filter(Boolean);
  if (await confirmDestructive(title, details.join(" "), "Discard")) {
    await changes.discard(discardable.map((item) => item.change.path));
  }
}

export function showChangeMenu(items: ChangeItem[]): Promise<void> {
  const area = items[0]?.area;
  if (!area) return Promise.resolve();
  const paths = items.map((item) => item.change.path);
  const label = items.length === 1 ? "" : ` ${count(items.length)}`;
  return showContextMenu([
    area === "staged"
      ? { kind: "item", text: `Unstage${label}`, run: () => void changes.unstage(paths) }
      : { kind: "item", text: `Stage${label}`, run: () => void changes.stage(paths) },
    ...(area === "unstaged"
      ? [
          { kind: "separator" } as const,
          {
            kind: "item",
            text: `Discard changes${label}…`,
            enabled: items.some((item) => item.change.kind !== "conflicted"),
            run: () => void discardItems(items),
          } as const,
        ]
      : []),
  ]);
}
