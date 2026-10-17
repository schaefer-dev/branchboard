import { Channel } from "@tauri-apps/api/core";

import { api, CommandError, toAppError, type AppError, type TerminalExit } from "../api";

export type TerminalStatus = "closed" | "starting" | "running" | "exited" | "failed";

/** The embedded herdr client. */
export class TerminalStore {
  /** herdr was found, so the terminal can be shown. */
  available = $state(false);
  status = $state<TerminalStatus>("closed");
  error = $state<AppError | null>(null);
  /** Input is sent in order; each write waits for the previous one. */
  private sending: Promise<unknown> = Promise.resolve();
  /** The channel of the current client; older ones are ignored. */
  private channel: Channel<ArrayBuffer | TerminalExit> | null = null;
  /** What the backend was last told about the terminal being on screen. */
  private visible = false;

  async init(): Promise<void> {
    this.available = (await api.terminalInfo()).available;
  }

  /** Starts the herdr client; its output goes to `output`. */
  async start(cols: number, rows: number, output: (data: Uint8Array) => void): Promise<void> {
    this.status = "starting";
    this.error = null;
    // herdr may quit before `terminalOpen` returns.
    const ended = { exited: false };
    const channel = new Channel<ArrayBuffer | TerminalExit>((message) => {
      if (message instanceof ArrayBuffer) {
        output(new Uint8Array(message));
      } else if (this.channel === channel) {
        ended.exited = true;
        this.status = "exited";
      }
    });
    this.channel = channel;
    try {
      await api.terminalOpen(cols, rows, channel);
      if (!ended.exited) this.status = "running";
    } catch (error) {
      this.status = "failed";
      this.error = error instanceof CommandError ? error.error : toAppError(error);
    }
  }

  write(data: string): void {
    if (this.status !== "running") return;
    this.sending = this.sending.then(() => api.terminalWrite(data)).catch(() => undefined);
  }

  resize(cols: number, rows: number): void {
    if (this.status !== "running" && this.status !== "starting") return;
    this.sending = this.sending.then(() => api.terminalResize(cols, rows)).catch(() => undefined);
  }

  /** Tells the backend whether the terminal is on screen, so an agent the
   * user watches there can stay silent. */
  setVisible(visible: boolean): void {
    if (visible === this.visible) return;
    this.visible = visible;
    void api.setTerminalVisible(visible).catch(() => undefined);
  }
}

export const terminal = new TerminalStore();
