import { beforeEach, describe, expect, it } from "vitest";

import { invoke, resetTauriMock, respond } from "../../test/tauri-mock";
import type { HeadCommit, StatusSnapshot } from "../api";
import { changes } from "./changes.svelte";
import { commit } from "./commit.svelte";

const head = (extra: Partial<HeadCommit> = {}): HeadCommit => ({
  oid: "a".repeat(40),
  shortOid: "aaaaaaa",
  subject: "Fix login",
  message: "Fix login\n\nThe session expired too early.",
  hasParent: true,
  pushed: false,
  ...extra,
});

const status = (staged: string[]): StatusSnapshot => ({
  head: { kind: "branch", name: "main" },
  headOid: "a".repeat(40),
  staged: staged.map((path) => ({ path, origPath: null, kind: "modified" })),
  unstaged: [],
});

async function show(repoId: string) {
  await changes.show(repoId);
  await commit.show(repoId);
}

function calls(command: string) {
  return invoke.mock.calls.filter(([name]) => name === command).map(([, args]) => args);
}

describe("CommitStore", () => {
  beforeEach(async () => {
    localStorage.clear();
    resetTauriMock();
    respond("get_status", () => status(["a.txt"]));
    respond("get_diff", () => ({
      notes: [],
      binary: false,
      hunks: [],
      additions: 0,
      deletions: 0,
      tooLarge: false,
      lineCount: 0,
    }));
    respond("get_head_commit", () => head());
    await changes.show(null);
    await commit.show(null);
  });

  it("keeps a draft per repository", async () => {
    await show("api");
    commit.setSummary("Half-written");
    commit.setDescription("Details");
    await show("web");
    expect(commit.summary).toBe("");
    await show("api");
    expect(commit.summary).toBe("Half-written");
    expect(commit.description).toBe("Details");
  });

  it("commits the composed message and clears the draft", async () => {
    respond("commit", () => ({ status: status([]), head: head({ oid: "b".repeat(40) }) }));
    await show("api");
    expect(commit.canCommit).toBe(false);
    commit.setSummary("Add rate limit");
    commit.setDescription("Per user.");
    expect(commit.canCommit).toBe(true);

    await commit.commit();
    expect(calls("commit")).toEqual([
      { repoId: "api", message: "Add rate limit\n\nPer user.", amend: false },
    ]);
    expect(commit.summary).toBe("");
    expect(changes.status?.staged).toEqual([]);
    await show("web");
    await show("api");
    expect(commit.summary).toBe("");
  });

  it("shows the last message while amending and restores the draft after", async () => {
    await show("api");
    commit.setSummary("Draft");
    commit.setAmend(true);
    expect(commit.summary).toBe("Fix login");
    expect(commit.description).toBe("The session expired too early.");
    commit.setAmend(false);
    expect(commit.summary).toBe("Draft");
  });

  it("keeps the error with git's output until the message changes", async () => {
    respond("commit", () =>
      // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors -- Tauri rejects with the plain AppError
      Promise.reject({ kind: "gitFailed", message: "Commit failed.", detail: "lint: 2 problems" }),
    );
    await show("api");
    commit.setSummary("Add a");
    await commit.commit();
    expect(commit.error?.detail).toBe("lint: 2 problems");
    expect(commit.summary).toBe("Add a");
    commit.setSummary("Add a!");
    expect(commit.error).toBeNull();
  });

  it("puts the undone message back into an empty form", async () => {
    respond("undo_last_commit", () => ({
      status: status(["a.txt", "b.txt"]),
      head: head({ oid: "c".repeat(40), shortOid: "ccccccc", subject: "Earlier" }),
      message: "Fix login\n\nThe session expired too early.",
    }));
    await show("api");
    expect(commit.canUndo).toBe(true);
    await commit.undo();
    expect(commit.summary).toBe("Fix login");
    expect(commit.head?.subject).toBe("Earlier");
    expect(changes.status?.staged).toHaveLength(2);
  });

  it("does not offer undo for a pushed commit", async () => {
    respond("get_head_commit", () => head({ pushed: true }));
    await show("api");
    expect(commit.canUndo).toBe(false);
  });
});
