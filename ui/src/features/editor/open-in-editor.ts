// Opening the repository or a changed file in VS Code.

import { api, type FileDiff, type RepoView } from "../../lib/api";
import { changes } from "../../lib/state/changes.svelte";
import type { ChangeItem } from "../../lib/state/changes-model";
import { notices } from "../../lib/state/notices.svelte";
import { view } from "../../lib/state/view.svelte";
import { workspace } from "../../lib/state/workspace.svelte";

/**
 * Line of the new version where the first change is: the first added line,
 * or for a removal the line that now follows it.
 */
export function firstChangedLine(diff: FileDiff | null): number | null {
  for (const hunk of diff?.hunks ?? []) {
    const index = hunk.lines.findIndex((line) => line.kind !== "context");
    if (index === -1) continue;
    const next = hunk.lines.slice(index).find((line) => line.newLine !== null);
    return next?.newLine ?? Math.max(1, hunk.newStart);
  }
  return null;
}

export async function openRepoInEditor(repo: RepoView): Promise<void> {
  try {
    await api.openInEditor(repo.id, null, null);
  } catch (error) {
    notices.error(error);
  }
}

export async function openChangeInEditor(item: ChangeItem): Promise<void> {
  const repoId = changes.repoId;
  if (!repoId) return;
  // A deleted file has nothing to open; show the repository instead.
  const path = item.change.kind === "deleted" ? null : item.change.path;
  const diff =
    changes.diff.status === "ready" && changes.diff.key === item.key ? changes.diff.diff : null;
  try {
    await api.openInEditor(repoId, path, path ? firstChangedLine(diff) : null);
  } catch (error) {
    notices.error(error);
  }
}

/** The file under the cursor in the changes view, else the repository. */
export function openCurrentInEditor(): Promise<void> {
  const item = view.work === "changes" ? changes.cursorItem : null;
  if (item) return openChangeInEditor(item);
  const repo = workspace.activeRepo;
  return repo ? openRepoInEditor(repo) : Promise.resolve();
}
