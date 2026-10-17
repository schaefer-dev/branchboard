// The API the fake backend exposes to Playwright as `window.__fake`.

import type { Terminal } from "@xterm/xterm";

import type { AgentsSnapshot } from "../../src/lib/api";

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
  /** Makes the next branch switch, fetch or commit fail with git's `stderr`. */
  failNext(command: "switch_branch" | "fetch_repo" | "commit", stderr: string): void;
  /** Holds the next fetch in its running state until `releaseFetch` is called. */
  holdNextFetch(): void;
  /** Completes the held fetch, including any failure configured with `failNext`. */
  releaseFetch(): void;
  /** Everything typed into the embedded terminal since it opened. */
  terminalInput(): string;
  /** The size the terminal was opened with or last resized to. */
  terminalSize(): { cols: number; rows: number } | null;
  /** Writes `text` as if the herdr client drew it. */
  terminalOutput(text: string): void;
  /** Ends the herdr client, as when the user detaches. */
  terminalExit(code: number | null): void;
  /** herdr switches to the workspace of `repoId`, e.g. navigated in the terminal. */
  herdrFocus(repoId: string): Promise<void>;
  /** The user clicks the system notification about the agent in `paneId`. */
  openAlert(paneId: string): Promise<void>;
  /** Replaces what herdr reports, as when an agent changes state. */
  setAgents(agents: AgentsSnapshot): Promise<void>;
  /** Simulates an external change to the working tree of a repository. */
  replaceStatus(repoId: string, unstagedPaths: string[]): Promise<void>;
}

declare global {
  interface Window {
    __fake: FakeControl;
    /** The embedded terminal, set by the e2e build so specs can read its screen. */
    __terminal?: Terminal;
  }
}
