export interface PromptOptions {
  title: string;
  label: string;
  initial?: string;
  placeholder?: string;
  hint?: string;
  confirmLabel: string;
  /** Allow confirming with an empty value. */
  allowEmpty?: boolean;
}

interface PendingPrompt extends PromptOptions {
  resolve: (value: string | null) => void;
}

/** Asks the user for a line of text in a modal dialog. */
export class PromptService {
  current = $state<PendingPrompt | null>(null);

  ask(options: PromptOptions): Promise<string | null> {
    this.current?.resolve(null);
    return new Promise((resolve) => {
      this.current = { ...options, resolve };
    });
  }

  settle(value: string | null): void {
    const pending = this.current;
    this.current = null;
    pending?.resolve(value);
  }
}

export const prompts = new PromptService();
