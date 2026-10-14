import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { invoke, resetTauriMock, respond } from "../../test/tauri-mock";
import type { RepoView, WorkspaceSnapshot } from "../../lib/api";
import { workspace } from "../../lib/state/workspace.svelte";
import QuickSwitcher from "./QuickSwitcher.svelte";

function repo(id: string, name: string): RepoView {
  return {
    id,
    name,
    customName: null,
    path: `/code/${name}`,
    groupId: null,
    favorite: false,
    mainline: "origin/development",
    missing: false,
  };
}

const snapshot: WorkspaceSnapshot = {
  repos: [repo("1", "api"), repo("2", "payment-service"), repo("3", "web")],
  groups: [],
  activeRepoId: "1",
  settings: { fetchIntervalMinutes: 10, terminalFontSize: 17 },
};

describe("QuickSwitcher", () => {
  let component: ReturnType<typeof mount> | undefined;

  beforeEach(() => {
    resetTauriMock();
    workspace.snapshot = structuredClone(snapshot);
    respond("set_active_repo", ({ repoId }) => ({
      ...snapshot,
      activeRepoId: repoId,
      settings: { fetchIntervalMinutes: 10, terminalFontSize: 17 },
    }));
  });

  afterEach(() => {
    if (component) void unmount(component);
    document.body.innerHTML = "";
  });

  function open() {
    component = mount(QuickSwitcher, { target: document.body, props: { open: true } });
    flushSync();
    const input = document.querySelector<HTMLInputElement>("input.search");
    if (!input) throw new Error("search field missing");
    return input;
  }

  function type(input: HTMLInputElement, text: string) {
    input.value = text;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
  }

  function options() {
    return [...document.querySelectorAll('[role="option"]')].map(
      (option) => option.querySelector(".name")?.textContent,
    );
  }

  it("filters repositories fuzzily", () => {
    const input = open();
    expect(options()).toHaveLength(3);
    type(input, "pay");
    expect(options()).toEqual(["payment-service"]);
  });

  it("activates the selected repository with the keyboard", async () => {
    const input = open();
    type(input, "w");
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await Promise.resolve();

    expect(invoke).toHaveBeenCalledWith("set_active_repo", { repoId: "3" });
  });

  it("moves the selection with the arrow keys", () => {
    const input = open();
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    flushSync();
    const selected = document.querySelector('[aria-selected="true"] .name');
    expect(selected?.textContent).toBe("payment-service");
  });
});
