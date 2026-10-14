// Jumping from a repository to its herdr workspace: in the embedded terminal
// when herdr is installed, else in whatever terminal runs herdr.

import { api, type RepoView } from "../../lib/api";
import { agents } from "../../lib/state/agents.svelte";
import { notices } from "../../lib/state/notices.svelte";
import { terminal } from "../../lib/state/terminal.svelte";
import { view } from "../../lib/state/view.svelte";
import { workspace } from "../../lib/state/workspace.svelte";

/**
 * Shows the terminal (if embedded) and the repository's herdr workspace,
 * creating one in the repository if none works there yet.
 */
export async function openRepoInHerdr(repo: RepoView | null): Promise<void> {
  if (terminal.available) {
    if (repo) await workspace.activate(repo.id);
    view.showTerminal();
  }
  if (!repo || repo.missing || !agents.available) return;
  try {
    await api.openInHerdr(repo.id);
  } catch (error) {
    notices.error(error);
  }
}
