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
import { terminal } from "./terminal.svelte";
import { view } from "./view.svelte";
import { workspace } from "./workspace.svelte";

/** The coding agents herdr runs, per repository. */
export class AgentsStore {
  /** A herdr session is running and reachable. */
  available = $state(false);
  byRepo = new SvelteMap<RepoId, RepoAgents>();
  /** Every agent in every herdr workspace, most urgent first. */
  list = $state<AgentRow[]>([]);
  /** The repository of the workspace herdr shows. */
  focusedRepoId = $state<RepoId | null>(null);
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

  /**
   * Shows the repository's herdr workspace if it has one. Returns whether it
   * has one (or `true` when that cannot be told, so nothing is offered).
   */
  async focusRepo(id: RepoId): Promise<boolean> {
    try {
      return await api.focusRepoInHerdr(id);
    } catch {
      return true;
    }
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
    if (terminal.available) view.showTerminal();
  }

  private apply(snapshot: AgentsSnapshot): void {
    this.available = snapshot.available;
    this.list = snapshot.agents;
    this.focusedRepoId = snapshot.focusedRepoId;
    const current = snapshot.repos.map((repo) => repo.repoId);
    for (const id of this.byRepo.keys()) {
      if (!current.includes(id)) this.byRepo.delete(id);
    }
    for (const repo of snapshot.repos) this.byRepo.set(repo.repoId, repo);
  }
}

export const agents = new AgentsStore();
