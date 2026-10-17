import { SvelteMap } from "svelte/reactivity";

import {
  api,
  onRepoSummary,
  type GroupId,
  type RepoId,
  type RepoView,
  type SummaryState,
  type WorkspaceSnapshot,
} from "../api";
import { notices } from "./notices.svelte";
import { buildSections, visibleRepos } from "./sections";

/** The terminal font size until the settings are loaded, and for "reset"; matches
 * `DEFAULT_TERMINAL_FONT_SIZE` in the workspace crate. */
export const TERMINAL_FONT_SIZE = 17;

/** The repository list, the active repository and per-repo summaries. */
export class WorkspaceStore {
  snapshot = $state<WorkspaceSnapshot>({
    repos: [],
    groups: [],
    activeRepoId: null,
    settings: {
      fetchIntervalMinutes: 10,
      terminalFontSize: TERMINAL_FONT_SIZE,
      soundMuted: false,
      muteWhenWatching: true,
    },
  });
  summaries = new SvelteMap<RepoId, SummaryState>();
  loaded = $state(false);

  readonly sections = $derived(buildSections(this.snapshot));
  readonly visibleRepos = $derived(visibleRepos(this.sections));
  readonly activeRepo = $derived(
    this.snapshot.repos.find((repo) => repo.id === this.snapshot.activeRepoId) ?? null,
  );

  async init(): Promise<void> {
    await onRepoSummary((update) => {
      this.summaries.set(update.repoId, update.state);
    });
    await this.apply(api.getWorkspace());
    this.loaded = true;
    // Only now that the listener is registered can summaries be requested
    // without missing any results.
    await api.refreshSummaries();
  }

  repo(id: RepoId): RepoView | undefined {
    return this.snapshot.repos.find((repo) => repo.id === id);
  }

  activate(id: RepoId): Promise<void> {
    if (id === this.snapshot.activeRepoId) return Promise.resolve();
    return this.apply(api.setActiveRepo(id));
  }

  /** Adds the given folders one by one and reports problems per folder. */
  async addFolders(paths: string[]): Promise<void> {
    for (const path of paths) {
      try {
        const result = await api.addRepo(path);
        this.snapshot = result.workspace;
        if (result.alreadyAdded) {
          const name = this.repo(result.repoId)?.name ?? path;
          notices.show("info", `${name} is already in your list.`);
        }
      } catch (error) {
        notices.error(error);
      }
    }
  }

  relocate(id: RepoId, path: string): Promise<void> {
    return this.apply(api.relocateRepo(id, path));
  }

  remove(id: RepoId): Promise<void> {
    this.summaries.delete(id);
    return this.apply(api.removeRepo(id));
  }

  rename(id: RepoId, name: string | null): Promise<void> {
    return this.apply(api.renameRepo(id, name));
  }

  setFavorite(id: RepoId, favorite: boolean): Promise<void> {
    return this.apply(api.setRepoFavorite(id, favorite));
  }

  moveToGroup(id: RepoId, groupId: GroupId | null): Promise<void> {
    return this.apply(api.setRepoGroup(id, groupId));
  }

  createGroup(name: string, repoId: RepoId | null = null): Promise<void> {
    return this.apply(api.createGroup(name, repoId));
  }

  renameGroup(id: GroupId, name: string): Promise<void> {
    return this.apply(api.renameGroup(id, name));
  }

  deleteGroup(id: GroupId): Promise<void> {
    return this.apply(api.deleteGroup(id));
  }

  /** The backend keeps the size within bounds. */
  setTerminalFontSize(size: number): Promise<void> {
    return this.apply(api.setTerminalFontSize(size));
  }

  /** No sounds when agents need the user; notifications still show. */
  setSoundMuted(muted: boolean): Promise<void> {
    return this.apply(api.setSoundMuted(muted));
  }

  /** No sound for an agent the user watches in the terminal. */
  setMuteWhenWatching(mute: boolean): Promise<void> {
    return this.apply(api.setMuteWhenWatching(mute));
  }

  setGroupCollapsed(id: GroupId, collapsed: boolean): Promise<void> {
    return this.apply(api.setGroupCollapsed(id, collapsed));
  }

  private async apply(request: Promise<WorkspaceSnapshot>): Promise<void> {
    try {
      this.snapshot = await request;
    } catch (error) {
      notices.error(error);
    }
  }
}

export const workspace = new WorkspaceStore();
