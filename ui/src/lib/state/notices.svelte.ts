import { CommandError, toAppError } from "../api";

export interface Notice {
  id: number;
  tone: "info" | "error";
  message: string;
  detail: string | null;
}

const DISMISS_AFTER_MS = { info: 4000, error: 8000 } as const;

/** Transient messages shown at the bottom of the window. */
export class NoticeStore {
  items = $state<Notice[]>([]);
  private nextId = 1;

  show(tone: Notice["tone"], message: string, detail: string | null = null): void {
    const id = this.nextId++;
    this.items.push({ id, tone, message, detail });
    setTimeout(() => {
      this.dismiss(id);
    }, DISMISS_AFTER_MS[tone]);
  }

  /** Shows any thrown value as an error notice. */
  error(error: unknown): void {
    const appError = error instanceof CommandError ? error.error : toAppError(error);
    this.show("error", appError.message, appError.detail);
  }

  dismiss(id: number): void {
    this.items = this.items.filter((notice) => notice.id !== id);
  }
}

export const notices = new NoticeStore();
