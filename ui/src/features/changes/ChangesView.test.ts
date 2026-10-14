import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { invoke, resetTauriMock, respond } from "../../test/tauri-mock";
import type { FileChange, FileDiff, StatusSnapshot } from "../../lib/api";
import { changes } from "../../lib/state/changes.svelte";
import ChangesView from "./ChangesView.svelte";

const change = (path: string, kind: FileChange["kind"] = "modified"): FileChange => ({
  path,
  origPath: null,
  kind,
});

const emptyDiff: FileDiff = {
  notes: [],
  binary: false,
  hunks: [],
  additions: 0,
  deletions: 0,
  tooLarge: false,
  lineCount: 0,
};

function status(staged: FileChange[], unstaged: FileChange[]): StatusSnapshot {
  return { head: { kind: "branch", name: "main" }, headOid: "0".repeat(40), staged, unstaged };
}

const initial = status(
  [change("staged.txt")],
  [change("a.txt"), change("b.txt"), change("new.txt", "untracked")],
);

function listbox(): HTMLElement {
  const element = document.querySelector<HTMLElement>('[role="listbox"]');
  if (!element) throw new Error("listbox missing");
  return element;
}

function press(key: string) {
  listbox().dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
  flushSync();
}

function diffRequests() {
  return invoke.mock.calls
    .filter(([command]) => command === "get_diff")
    .map(([, args]) => (args as { request: { path: string; area: string } }).request);
}

describe("ChangesView", () => {
  let component: ReturnType<typeof mount> | undefined;

  beforeEach(async () => {
    resetTauriMock();
    respond("get_status", () => initial);
    respond("get_diff", () => emptyDiff);
    await changes.show(null);
    await changes.show("repo");
    component = mount(ChangesView, { target: document.body });
    flushSync();
  });

  afterEach(() => {
    if (component) void unmount(component);
    document.body.innerHTML = "";
  });

  it("lists both areas and selects the first file", () => {
    const names = [...document.querySelectorAll('[role="option"] .name')].map(
      (el) => el.textContent,
    );
    expect(names).toEqual(["staged.txt", "a.txt", "b.txt", "new.txt"]);
    expect(changes.cursor).toBe("staged:staged.txt");
    expect(diffRequests().at(-1)).toMatchObject({ path: "staged.txt", area: "staged" });
  });

  it("moves through files with the arrow keys and loads their diff", () => {
    press("ArrowDown");
    expect(changes.cursor).toBe("unstaged:a.txt");
    expect(diffRequests().at(-1)).toMatchObject({ path: "a.txt", area: "unstaged" });
  });

  it("stages with Space and keeps the cursor in place for the next file", async () => {
    respond("stage_paths", () =>
      status(
        [change("staged.txt"), change("a.txt")],
        [change("b.txt"), change("new.txt", "untracked")],
      ),
    );
    press("ArrowDown");
    press(" ");
    await vi.waitFor(() => {
      expect(changes.cursor).toBe("unstaged:b.txt");
    });
    expect(invoke).toHaveBeenCalledWith("stage_paths", { repoId: "repo", paths: ["a.txt"] });
  });

  it("extends the selection with Shift and stages all selected files", async () => {
    respond("stage_paths", () =>
      status([change("staged.txt"), change("a.txt"), change("b.txt")], []),
    );
    press("ArrowDown");
    listbox().dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", shiftKey: true, bubbles: true }),
    );
    flushSync();
    expect([...changes.selected]).toEqual(["unstaged:a.txt", "unstaged:b.txt"]);
    press("Enter");
    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("stage_paths", {
        repoId: "repo",
        paths: ["a.txt", "b.txt"],
      });
    });
  });

  it("discards after confirmation", async () => {
    respond("plugin:dialog|message", () => "Discard");
    respond("discard_paths", () =>
      status([change("staged.txt")], [change("b.txt"), change("new.txt", "untracked")]),
    );
    press("ArrowDown");
    press("Delete");
    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("discard_paths", { repoId: "repo", paths: ["a.txt"] });
    });
  });

  it("does not discard staged files", () => {
    press("Delete");
    expect(invoke.mock.calls.some(([command]) => command === "plugin:dialog|message")).toBe(false);
  });
});
