import { beforeEach, describe, expect, it } from "vitest";

import { invoke, resetTauriMock, respond } from "../../test/tauri-mock";
import type { DiffLine } from "../api";
import { comments } from "./comments.svelte";
import type { CommentTarget } from "./comments-model";

const target: CommentTarget = { repoId: "api", path: "src/a.ts", source: "unstaged" };

function line(newLine: number, text: string): DiffLine {
  return { kind: "added", oldLine: null, newLine, text, noNewline: false, tokens: [] };
}

function add(body: string, start = 3) {
  comments.start(target, { side: "new", start, end: start, quoted: [`line ${String(start)}`] });
  comments.setDraftBody(body);
  comments.save();
}

describe("CommentsStore", () => {
  beforeEach(() => {
    resetTauriMock();
    comments.items = [];
    comments.draft = null;
  });

  it("adds comments and ignores empty ones", () => {
    add("Rename this.");
    comments.start(target, null);
    comments.setDraftBody("  ");
    comments.save();
    expect(comments.draft).not.toBeNull();
    comments.cancel();
    expect(comments.of("api").map((comment) => comment.body)).toEqual(["Rename this."]);
    expect(comments.pendingOf("api")).toHaveLength(1);
    expect(comments.of("web")).toEqual([]);
  });

  it("notes outdated comments only from a diff of their own source", () => {
    add("Rename this.");
    comments.check({ ...target, source: "staged" }, []);
    expect(comments.items[0]?.outdated).toBe(false);
    comments.check(target, [line(3, "line 3")]);
    expect(comments.items[0]?.outdated).toBe(false);
    comments.check(target, [line(3, "changed")]);
    expect(comments.items[0]?.outdated).toBe(true);
  });

  it("keeps edits pending and prevents duplicate sends while a paste is in flight", async () => {
    let complete: () => void = () => {};
    respond(
      "paste_to_agent",
      () =>
        new Promise<void>((resolve) => {
          complete = resolve;
        }),
    );
    respond("focus_agent", () => null);
    add("Original.");
    const sending = comments.pasteTo("api", "w1:p1");
    await comments.pasteTo("api", "w1:p1");
    expect(invoke.mock.calls.filter(([name]) => name === "paste_to_agent")).toHaveLength(1);
    comments.edit(comments.items[0]?.id ?? "");
    comments.setDraftBody("Edited while sending.");
    comments.save();
    complete();
    await sending;
    expect(comments.pendingOf("api")).toHaveLength(1);
    expect(comments.busy).toBe(false);
  });

  it("pastes the pending comments, marks them sent, and pends an edited one again", async () => {
    respond("paste_to_agent", () => null);
    respond("focus_agent", () => null);
    respond("set_active_repo", () => ({
      repos: [],
      groups: [],
      activeRepoId: "api",
      settings: {
        fetchIntervalMinutes: 10,
        terminalFontSize: 17,
        soundMuted: false,
        muteWhenWatching: true,
      },
    }));
    add("Second.", 9);
    add("First.", 3);
    await comments.pasteTo("api", "w1:p1");
    const pasted = invoke.mock.calls.find(([name]) => name === "paste_to_agent")?.[1];
    expect(pasted).toEqual({
      paneId: "w1:p1",
      text: "src/a.ts:3\n```ts\nline 3\n```\nFirst.\n\nsrc/a.ts:9\n```ts\nline 9\n```\nSecond.",
    });
    expect(comments.items.map((comment) => comment.status)).toEqual(["sent", "sent"]);

    const id = comments.items[0]?.id ?? "";
    comments.edit(id);
    comments.setDraftBody("Second, reworded.");
    comments.save();
    expect(comments.pendingOf("api").map((comment) => comment.body)).toEqual(["Second, reworded."]);
  });
});
