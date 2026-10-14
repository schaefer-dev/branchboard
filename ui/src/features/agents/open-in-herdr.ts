// Jumping from a repository to its herdr workspace.

import { api, type RepoView } from "../../lib/api";
import { notices } from "../../lib/state/notices.svelte";

/** Shows the repository's herdr workspace, or creates one in it. */
export async function openRepoInHerdr(repo: RepoView): Promise<void> {
  try {
    await api.openInHerdr(repo.id);
  } catch (error) {
    notices.error(error);
  }
}
