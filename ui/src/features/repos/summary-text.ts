import type { SummaryState } from "../../lib/api";

/** Secondary line of a repository row. */
export function summaryText(state: SummaryState | undefined): string {
  if (!state) return "";
  switch (state.status) {
    case "missing":
      return "Folder not found";
    case "failed":
      return "Status unavailable";
    case "ready": {
      const { head } = state.summary;
      switch (head.kind) {
        case "branch":
          return head.name;
        case "unborn":
          return `${head.name} (no commits)`;
        case "detached":
          return `Detached at ${head.oid}`;
      }
    }
  }
}
