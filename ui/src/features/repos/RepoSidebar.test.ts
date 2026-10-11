import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { invoke, resetTauriMock, respond } from "../../test/tauri-mock";
import type { RepoView, WorkspaceSnapshot } from "../../lib/api";
import { workspace } from "../../lib/state/workspace.svelte";
import { prompts } from "../../lib/ui/prompt.svelte";
import RepoSidebar from "./RepoSidebar.svelte";

function repo(id: string, name: string, groupId: string | null = null): RepoView {
  return {
    id,
    name,
    customName: null,
    path: `/code/${name}`,
    groupId,
    favorite: false,
    missing: false,
  };
}

const snapshot: WorkspaceSnapshot = {
  repos: [repo("1", "api", "g1"), repo("2", "web", "g1"), repo("3", "scratch")],
  groups: [{ id: "g1", name: "Work", collapsed: false }],
  activeRepoId: "2",
};

function press(key: string) {
  const target = document.activeElement;
  if (!(target instanceof HTMLElement)) throw new Error("nothing focused");
  target.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
  flushSync();
}

const focused = () => (document.activeElement as HTMLElement | null)?.dataset.nav;

describe("RepoSidebar keyboard navigation", () => {
  let component: ReturnType<typeof mount> | undefined;

  beforeEach(() => {
    resetTauriMock();
    workspace.snapshot = structuredClone(snapshot);
    component = mount(RepoSidebar, { target: document.body });
    flushSync();
  });

  afterEach(() => {
    if (component) void unmount(component);
    prompts.settle(null);
    document.body.innerHTML = "";
  });

  it("puts the tab stop on the active repository", () => {
    const tabStops = document.querySelectorAll('[data-nav][tabindex="0"]');
    expect([...tabStops].map((el) => (el as HTMLElement).dataset.nav)).toEqual(["repo:2"]);
  });

  it("moves focus with the arrow keys across sections", () => {
    document.querySelector<HTMLElement>('[data-nav="repo:2"]')?.focus();
    press("ArrowDown");
    expect(focused()).toBe("repo:3");
    press("ArrowUp");
    press("ArrowUp");
    expect(focused()).toBe("repo:1");
    press("ArrowUp");
    expect(focused()).toBe("group:g1");
  });

  it("collapses a group with ArrowLeft", () => {
    respond("set_group_collapsed", () => snapshot);
    document.querySelector<HTMLElement>('[data-nav="group:g1"]')?.focus();
    press("ArrowLeft");
    expect(invoke).toHaveBeenCalledWith("set_group_collapsed", { groupId: "g1", collapsed: true });
  });

  it("asks for a new name on F2", () => {
    document.querySelector<HTMLElement>('[data-nav="repo:1"]')?.focus();
    press("F2");
    expect(prompts.current?.title).toBe("Rename repository");
    expect(prompts.current?.placeholder).toBe("api");
  });

  it("removes a repository after confirmation", async () => {
    respond("plugin:dialog|message", () => "Remove");
    respond("remove_repo", () => ({ ...snapshot, repos: snapshot.repos.slice(1) }));
    document.querySelector<HTMLElement>('[data-nav="repo:1"]')?.focus();
    press("Delete");
    await vi.waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("remove_repo", { repoId: "1" });
    });
  });
});
