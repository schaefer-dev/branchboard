import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke, resetTauriMock, respond } from "../../test/tauri-mock";
import type { CommitSummary, HistoryPage } from "../api";
import { HistorySession, HistoryStore } from "./history.svelte";

const commit = (oid: string): CommitSummary => ({
  oid,
  shortOid: oid.slice(0, 7),
  author: "Ada",
  authoredAt: 100,
  subject: oid,
});
const page = (ids = ["a", "b"], more = false): HistoryPage => ({
  commits: ids.map(commit),
  tip: "tip",
  cursor: more ? { tip: "tip", reference: "HEAD", query: "", offset: ids.length } : null,
});
const flush = async () => {
  for (let i = 0; i < 8; i++) await Promise.resolve();
};

describe("History", () => {
  beforeEach(() => {
    resetTauriMock();
    respond("get_review_files", (args) => ({
      range: { from: "parent", to: (args.target as { oid: string }).oid },
      files: [],
    }));
    respond("get_branches", () => ({ branches: [] }));
    respond("get_history_tip", () => "tip");
    respond("get_history", () => page());
  });
  afterEach(() => vi.useRealTimers());

  it("retains independent browsing state across tabs and repositories", async () => {
    const store = new HistoryStore();
    store.show("one", true);
    await flush();
    const first = store.current;
    first?.select("b");
    if (first) first.scrollTop = 123;
    store.show("one", false);
    store.show("two", true);
    await flush();
    expect(store.current).not.toBe(first);
    store.show("one", true);
    expect(store.current).toBe(first);
    expect(store.current?.review.target).toEqual({ kind: "commit", oid: "b" });
    expect(store.current?.scrollTop).toBe(123);
  });

  it("ignores superseded results and debounces queries while preserving the pin", async () => {
    vi.useFakeTimers();
    const session = new HistorySession("one");
    await session.load();
    let finish: (value: HistoryPage) => void = () => {};
    respond(
      "get_history",
      () =>
        new Promise<HistoryPage>((resolve) => {
          finish = resolve;
        }),
    );
    const old = session.load();
    session.setQuery("message");
    session.setQuery("author");
    finish(page(["outdated"]));
    await old;
    expect(session.commits).toEqual([]);
    respond("get_history", () => page(["match"]));
    await vi.advanceTimersByTimeAsync(250);
    expect(session.commits[0]?.oid).toBe("match");
    expect(invoke).toHaveBeenLastCalledWith("get_review_files", expect.anything());
    const requests = invoke.mock.calls.filter(([name]) => name === "get_history");
    expect(requests.at(-1)?.[1]).toMatchObject({
      request: { query: "author", tip: "tip", cursor: null },
    });
  });

  it("keeps rows and selection on load-more failure, then retries", async () => {
    const session = new HistorySession("one");
    respond("get_history", () => page(["a", "b"], true));
    await session.load();
    session.select("b");
    respond("get_history", () => Promise.reject(new Error("temporarily unavailable")));
    await session.load(true);
    expect(session.commits.map((c) => c.oid)).toEqual(["a", "b"]);
    expect(session.review.target).toEqual({ kind: "commit", oid: "b" });
    expect(session.cursor).not.toBeNull();
    respond("get_history", () => page(["c"]));
    await session.load(true);
    expect(session.commits.map((c) => c.oid)).toEqual(["a", "b", "c"]);
    expect(session.error).toBeNull();
  });

  it("marks moved/deleted refs stale without replacing rows; refresh retains only present selections", async () => {
    const session = new HistorySession("one");
    await session.load();
    session.select("b");
    respond("get_history_tip", () => "new tip");
    await session.checkFreshness();
    expect(session.stale).toBe(true);
    expect(session.commits.map((c) => c.oid)).toEqual(["a", "b"]);
    await session.load(false, true);
    expect(session.stale).toBe(false);
    expect(session.review.target).toEqual({ kind: "commit", oid: "b" });
    respond("get_history", () => page(["c"]));
    await session.load(false, true);
    expect(session.review.target).toEqual({ kind: "commit", oid: "c" });
    respond("get_history_tip", () => Promise.reject(new Error("deleted")));
    await session.checkFreshness();
    expect(session.stale).toBe(true);
  });

  it("reveals a linked commit outside the list, including during the initial request", async () => {
    const session = new HistorySession("one");
    const loading = session.load();
    session.reveal("linked", "a.ts");
    await loading;
    await flush();
    expect(session.review.target).toEqual({ kind: "commit", oid: "linked" });
    expect(session.commits.some((c) => c.oid === "linked")).toBe(false);
  });

  it("retries failed refreshes as refreshes, without losing the previous snapshot", async () => {
    const session = new HistorySession("one");
    await session.load();
    respond("get_history", () => Promise.reject(new Error("offline")));
    await session.load(false, true);
    expect(session.commits.map((c) => c.oid)).toEqual(["a", "b"]);
    respond("get_history", () => page(["new"]));
    await session.retry();
    expect(invoke.mock.calls.filter(([name]) => name === "get_history").at(-1)?.[1]).toMatchObject({
      request: { tip: null, cursor: null },
    });
    expect(session.commits[0]?.oid).toBe("new");
  });

  it("checks a ref invalidation received during a scan after the scan completes", async () => {
    const session = new HistorySession("one");
    await session.load();
    let finish: (value: HistoryPage) => void = () => {};
    respond(
      "get_history",
      () =>
        new Promise<HistoryPage>((resolve) => {
          finish = resolve;
        }),
    );
    session.cursor = { tip: "tip", reference: "HEAD", query: "", offset: 2 };
    const pending = session.load(true);
    respond("get_history_tip", () => "moved");
    await session.checkFreshness();
    finish(page(["c"]));
    await pending;
    await flush();
    expect(session.stale).toBe(true);
    expect(session.commits.map((c) => c.oid)).toEqual(["a", "b", "c"]);
  });
});
