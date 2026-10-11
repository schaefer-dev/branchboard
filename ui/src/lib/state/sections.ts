import type { Group, RepoView, WorkspaceSnapshot } from "../api";

export interface SidebarSection {
  key: string;
  /** `null` renders the repositories without a header. */
  title: string | null;
  group: Group | null;
  collapsed: boolean;
  repos: RepoView[];
}

const byName = (a: { name: string }, b: { name: string }) =>
  a.name.localeCompare(b.name, undefined, { sensitivity: "base", numeric: true });

/**
 * Splits the repositories into sidebar sections: favorites first, then one
 * section per group (alphabetical), then everything ungrouped. Favorites are
 * shown only in the favorites section.
 */
export function buildSections(snapshot: WorkspaceSnapshot): SidebarSection[] {
  const groupIds = new Set(snapshot.groups.map((group) => group.id));
  const favorites = snapshot.repos.filter((repo) => repo.favorite).sort(byName);
  const others = snapshot.repos.filter((repo) => !repo.favorite);

  const sections: SidebarSection[] = [];
  if (favorites.length > 0) {
    sections.push({
      key: "favorites",
      title: "Favorites",
      group: null,
      collapsed: false,
      repos: favorites,
    });
  }
  for (const group of [...snapshot.groups].sort(byName)) {
    sections.push({
      key: `group:${group.id}`,
      title: group.name,
      group,
      collapsed: group.collapsed,
      repos: others.filter((repo) => repo.groupId === group.id).sort(byName),
    });
  }
  const ungrouped = others
    .filter((repo) => repo.groupId === null || !groupIds.has(repo.groupId))
    .sort(byName);
  if (ungrouped.length > 0) {
    const title =
      snapshot.groups.length > 0 ? "Ungrouped" : favorites.length > 0 ? "Repositories" : null;
    sections.push({ key: "ungrouped", title, group: null, collapsed: false, repos: ungrouped });
  }
  return sections;
}

/** Repositories in display order, skipping collapsed sections. */
export function visibleRepos(sections: SidebarSection[]): RepoView[] {
  return sections.flatMap((section) => (section.collapsed ? [] : section.repos));
}
