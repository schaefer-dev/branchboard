// Commit messages as the form edits them: a one-line summary and an
// optional description.

export interface Draft {
  summary: string;
  description: string;
}

export const EMPTY_DRAFT: Draft = { summary: "", description: "" };

/** Summaries longer than this get a character count. */
export const SUMMARY_HINT_LENGTH = 50;

/** The message git receives: summary, blank line, description. */
export function composeMessage(draft: Draft): string {
  const summary = draft.summary.trim();
  const description = draft.description.trim();
  return description ? `${summary}\n\n${description}` : summary;
}

/** Splits a commit message into what the form shows. */
export function splitMessage(message: string): Draft {
  const [summary = "", ...rest] = message.split("\n");
  return { summary: summary.trim(), description: rest.join("\n").trim() };
}

export function isEmptyDraft(draft: Draft): boolean {
  return draft.summary.trim() === "" && draft.description.trim() === "";
}

export function parseDraft(stored: string | null): Draft {
  if (!stored) return EMPTY_DRAFT;
  try {
    const value = JSON.parse(stored) as Partial<Draft> | null;
    return {
      summary: typeof value?.summary === "string" ? value.summary : "",
      description: typeof value?.description === "string" ? value.description : "",
    };
  } catch {
    return EMPTY_DRAFT;
  }
}

/** "Commit 2 files", "Commit 1 file" or "Amend commit". */
export function commitLabel(stagedCount: number, amend: boolean): string {
  if (amend) return "Amend commit";
  if (stagedCount === 0) return "Commit";
  return `Commit ${String(stagedCount)} ${stagedCount === 1 ? "file" : "files"}`;
}
