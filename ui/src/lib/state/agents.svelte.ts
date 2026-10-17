import { SvelteMap } from "svelte/reactivity";

import {
  api,
  onAlertOpened,
  onRepoAgents,
  type AgentRow,
  type AgentsSnapshot,
  type AgentSummary,
  type RepoAgents,
  type RepoId,
} from "../api";
import { branches } from "./branches.svelte";
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
  /** The agent of each repository the user used last: opened or sent a review to. */
  recent = new SvelteMap<RepoId, string>();
  /** How many agents wait for the user. */
  readonly blocked = $derived(this.list.filter((agent) => agent.state === "blocked").length);
  /** Agents that need the user: waiting for input, or finished and not seen yet. */
  readonly needYou = $derived(
    this.list.filter((agent) => agent.state === "blocked" || agent.state === "done"),
  );

  /** The agent "Next" went to last. */
  private lastNext: string | null = null;

  async init(): Promise<void> {
    await onRepoAgents((snapshot) => {
      this.apply(snapshot);
    });
    await onAlertOpened((paneId) => {
      const agent = this.list.find((candidate) => candidate.paneId === paneId);
      if (agent) void this.open(agent);
    });
    // Connects to herdr; later changes arrive through the listener above.
    this.apply(await api.getAgents());
  }

  of(id: RepoId): RepoAgents | undefined {
    return this.byRepo.get(id);
  }

  /** The repository's agents, the one used last first, then the most urgent. */
  candidates(id: RepoId): AgentSummary[] {
    const agents = this.of(id)?.agents ?? [];
    const recent = this.recent.get(id);
    return [
      ...agents.filter((agent) => agent.paneId === recent),
      ...agents.filter((agent) => agent.paneId !== recent),
    ];
  }

  remember(repoId: RepoId, paneId: string): void {
    this.recent.set(repoId, paneId);
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
  async focus(agent: { paneId: string; repoId: RepoId | null }): Promise<void> {
    try {
      await api.focusAgent(agent.paneId);
    } catch (error) {
      notices.error(error);
      return;
    }
    if (agent.repoId) {
      this.remember(agent.repoId, agent.paneId);
      await workspace.activate(agent.repoId);
    }
    if (terminal.available) view.showTerminal();
  }

  /** Goes to the next agent that needs the user, most urgent first. */
  async next(): Promise<void> {
    const waiting = this.needYou;
    if (waiting.length === 0) return;
    const index = waiting.findIndex((agent) => agent.paneId === this.lastNext);
    const agent = waiting[(index + 1) % waiting.length];
    if (!agent) return;
    this.lastNext = agent.paneId;
    await this.open(agent);
  }

  /**
   * Shows what an agent needs: a blocked one its terminal, a finished one
   * the work to review (its changes, else its branch). Looking at it counts
   * as having seen it in herdr.
   */
  async open(agent: AgentRow): Promise<void> {
    const repoId = agent.repoId;
    if (agent.state !== "done" || !repoId) {
      await this.focus(agent);
      return;
    }
    try {
      await api.focusAgent(agent.paneId);
    } catch (error) {
      notices.error(error);
      return;
    }
    this.remember(repoId, agent.paneId);
    await workspace.activate(repoId);
    const summary = workspace.summaries.get(repoId);
    if (summary?.status === "ready" && summary.summary.dirty) {
      view.focus("changes");
      return;
    }
    // The branch the agent worked on; another repository starts there anyway.
    if (branches.repoId === repoId && branches.current) branches.select(branches.current.name);
    view.focus("branches");
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
