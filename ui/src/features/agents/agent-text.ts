// Wording for the agents herdr reports.

import type { AgentState, AgentSummary, RepoAgents } from "../../lib/api";

const doing: Record<AgentState, string> = {
  blocked: "needs input",
  done: "is done",
  working: "is working",
  idle: "is idle",
  unknown: "is running",
};

/** "claude" → "Claude". */
export function agentName(agent: string): string {
  return agent.charAt(0).toUpperCase() + agent.slice(1);
}

/** e.g. "Claude needs input · Fix the login form". */
export function describeAgent(agent: AgentSummary): string {
  const sentence = `${agentName(agent.agent)} ${doing[agent.state]}`;
  return agent.title ? `${sentence} · ${agent.title}` : sentence;
}

/** One line per agent, most urgent first. */
export function describeAgents(repo: RepoAgents): string {
  return repo.agents.map(describeAgent).join("\n");
}
