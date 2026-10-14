import { SvelteMap } from "svelte/reactivity";

import { api, onRepoAgents, type AgentsSnapshot, type RepoAgents, type RepoId } from "../api";

/** The coding agents herdr runs, per repository. */
export class AgentsStore {
  /** A herdr session is running and reachable. */
  available = $state(false);
  byRepo = new SvelteMap<RepoId, RepoAgents>();

  async init(): Promise<void> {
    await onRepoAgents((snapshot) => {
      this.apply(snapshot);
    });
    // Connects to herdr; later changes arrive through the listener above.
    this.apply(await api.getAgents());
  }

  of(id: RepoId): RepoAgents | undefined {
    return this.byRepo.get(id);
  }

  private apply(snapshot: AgentsSnapshot): void {
    this.available = snapshot.available;
    const current = snapshot.repos.map((repo) => repo.repoId);
    for (const id of this.byRepo.keys()) {
      if (!current.includes(id)) this.byRepo.delete(id);
    }
    for (const repo of snapshot.repos) this.byRepo.set(repo.repoId, repo);
  }
}

export const agents = new AgentsStore();
