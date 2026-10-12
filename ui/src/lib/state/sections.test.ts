import { describe, expect, it } from "vitest";

import type { RepoView, WorkspaceSnapshot } from "../api";
import { buildSections, visibleRepos } from "./sections";

function repo(name: string, extra: Partial<RepoView> = {}): RepoView {
  return {
    id: name,
    name,
    customName: null,
    path: `/code/${name}`,
    groupId: null,
    favorite: false,
    mainline: "origin/development",
    missing: false,
    ...extra,
  };
}

function snapshot(repos: RepoView[], groups: WorkspaceSnapshot["groups"] = []): WorkspaceSnapshot {
  return { repos, groups, activeRepoId: null, settings: { fetchIntervalMinutes: 10 } };
}

describe("buildSections", () => {
  it("shows a plain list when there are no groups or favorites", () => {
    const sections = buildSections(snapshot([repo("web"), repo("api")]));
    expect(sections).toHaveLength(1);
    expect(sections[0]?.title).toBeNull();
    expect(sections[0]?.repos.map((r) => r.name)).toEqual(["api", "web"]);
  });

  it("orders favorites, groups and ungrouped repositories", () => {
    const groups = [
      { id: "g2", name: "Work", collapsed: false },
      { id: "g1", name: "Side projects", collapsed: true },
    ];
    const sections = buildSections(
      snapshot(
        [
          repo("api", { groupId: "g2" }),
          repo("blog", { groupId: "g1" }),
          repo("dotfiles", { favorite: true, groupId: "g1" }),
          repo("scratch"),
          repo("orphan", { groupId: "deleted" }),
        ],
        groups,
      ),
    );
    expect(sections.map((s) => [s.title, s.repos.map((r) => r.name)])).toEqual([
      ["Favorites", ["dotfiles"]],
      ["Side projects", ["blog"]],
      ["Work", ["api"]],
      ["Ungrouped", ["orphan", "scratch"]],
    ]);
    expect(visibleRepos(sections).map((r) => r.name)).toEqual([
      "dotfiles",
      "api",
      "orphan",
      "scratch",
    ]);
  });

  it("sorts names naturally", () => {
    const sections = buildSections(snapshot([repo("app10"), repo("App2"), repo("app1")]));
    expect(sections[0]?.repos.map((r) => r.name)).toEqual(["app1", "App2", "app10"]);
  });
});
