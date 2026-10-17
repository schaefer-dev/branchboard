// Wording for review comments.

import type { AgentSummary } from "../../lib/api";
import type { LineRange } from "../../lib/state/comments-model";
import { agentName } from "../agents/agent-text";

/** e.g. "Line 3", "Lines 3–5", "Removed line 3" or "File". */
export function linesLabel(lines: LineRange | null): string {
  if (!lines) return "File";
  const numbers =
    lines.start === lines.end
      ? `line ${String(lines.start)}`
      : `lines ${String(lines.start)}–${String(lines.end)}`;
  const label = lines.side === "old" ? `removed ${numbers}` : numbers;
  return label.charAt(0).toUpperCase() + label.slice(1);
}

/** e.g. "2 pending · 1 sent". */
export function countsLabel(pending: number, sent: number): string {
  return [pending > 0 && `${String(pending)} pending`, sent > 0 && `${String(sent)} sent`]
    .filter(Boolean)
    .join(" · ");
}

/** An agent in the menu of agents a review can go to. */
export function agentChoice(agent: AgentSummary): string {
  const name = agent.title ? `${agentName(agent.agent)} · ${agent.title}` : agentName(agent.agent);
  return agent.state === "blocked" ? `${name} (waiting for input)` : name;
}
