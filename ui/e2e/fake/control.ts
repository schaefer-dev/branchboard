// The API the fake backend exposes to Playwright as `window.__fake`.

export interface RecordedCall {
  cmd: string;
  args: Record<string, unknown>;
}

export interface FakeControl {
  /** Every IPC call made by the app so far. */
  calls(): RecordedCall[];
  /** Texts of the most recently shown context menu (submenu items prefixed with `parent/`). */
  menu(): string[];
  /** Activates an item of the most recently shown context menu by its text. */
  clickMenu(text: string): void;
  /** Answer for the next confirmation dialogs. */
  setConfirm(answer: boolean): void;
  /** Folders returned by the next folder picker. */
  setPickedFolders(paths: string[]): void;
  /** Simulates dropping folders onto the window. */
  drop(paths: string[]): Promise<void>;
  /** Makes the next branch switch or fetch fail with git's `stderr`. */
  failNext(command: "switch_branch" | "fetch_repo", stderr: string): void;
  /** Simulates an external change to the working tree of a repository. */
  replaceStatus(repoId: string, unstagedPaths: string[]): Promise<void>;
}

declare global {
  interface Window {
    __fake: FakeControl;
  }
}
