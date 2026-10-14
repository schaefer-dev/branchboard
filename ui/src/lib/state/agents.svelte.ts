import { SvelteMap } from "svelte/reactivity";

import {
  api,
  onRepoAgents,
  type AgentRow,
  type AgentsSnapshot,
  type RepoAgents,
  type RepoId,
} from "../api";
import { notices } from "./notices.svelte";
import { workspace } from "./workspace.svelte";

/** The coding agents herdr runs, per repository. */
export class AgentsStore {
  /** A herdr session is running and reachable. */
  available = $state(false);
  byRepo = new SvelteMap<RepoId, RepoAgents>();
  /** Every agent in every herdr workspace, most urgent first. */
  list = $state<AgentRow[]>([]);
  /** How many agents wait for the user. */
  readonly blocked = $derived(this.list.filter((agent) => agent.state === "blocked").length);

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

  /** Shows the agent in herdr and, if it works in a listed repository, that repository. */
  async focus(agent: AgentRow): Promise<void> {
    try {
      await api.focusAgent(agent.paneId);
    } catch (error) {
      notices.error(error);
      return;
    }
    if (agent.repoId) await workspace.activate(agent.repoId);
  }

  private apply(snapshot: AgentsSnapshot): void {
    this.available = snapshot.available;
    this.list = snapshot.agents;
    const current = snapshot.repos.map((repo) => repo.repoId);
    for (const id of this.byRepo.keys()) {
      if (!current.includes(id)) this.byRepo.delete(id);
    }
    for (const repo of snapshot.repos) this.byRepo.set(repo.repoId, repo);
  }
}

export const agents = new AgentsStore();
