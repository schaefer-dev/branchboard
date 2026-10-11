// Central registry of user commands. Shortcuts, the quick switcher and (later)
// the command palette all go through here.

export interface Shortcut {
  /** `KeyboardEvent.key`, case-insensitive for letters. */
  key: string;
  /** Cmd on macOS, Ctrl elsewhere. */
  mod?: boolean;
  shift?: boolean;
  alt?: boolean;
}

export interface Command {
  id: string;
  title: string;
  shortcut?: Shortcut;
  run: () => unknown;
}

type KeyEventLike = Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey">;

export function matchesShortcut(shortcut: Shortcut, event: KeyEventLike, mac: boolean): boolean {
  const mod = mac ? event.metaKey : event.ctrlKey;
  const otherCtrl = mac ? event.ctrlKey : event.metaKey;
  return (
    event.key.toLowerCase() === shortcut.key.toLowerCase() &&
    mod === (shortcut.mod ?? false) &&
    event.shiftKey === (shortcut.shift ?? false) &&
    event.altKey === (shortcut.alt ?? false) &&
    !otherCtrl
  );
}

export function formatShortcut(shortcut: Shortcut, mac: boolean): string {
  const key = shortcut.key.length === 1 ? shortcut.key.toUpperCase() : shortcut.key;
  if (mac) {
    return `${shortcut.alt ? "⌥" : ""}${shortcut.shift ? "⇧" : ""}${shortcut.mod ? "⌘" : ""}${key}`;
  }
  const parts = [shortcut.mod && "Ctrl", shortcut.alt && "Alt", shortcut.shift && "Shift", key];
  return parts.filter(Boolean).join("+");
}

export class CommandRegistry {
  private readonly commands = new Map<string, Command>();
  private readonly mac: boolean;

  constructor(mac: boolean) {
    this.mac = mac;
  }

  /** Registers a command; returns a function that unregisters it. */
  register(command: Command): () => void {
    if (this.commands.has(command.id)) {
      throw new Error(`Command ${command.id} is already registered`);
    }
    this.commands.set(command.id, command);
    return () => this.commands.delete(command.id);
  }

  list(): Command[] {
    return [...this.commands.values()];
  }

  shortcutLabel(id: string): string | null {
    const shortcut = this.commands.get(id)?.shortcut;
    return shortcut ? formatShortcut(shortcut, this.mac) : null;
  }

  /**
   * Runs the command bound to the pressed keys. Inside text fields only
   * shortcuts with a modifier fire, so typing is never hijacked.
   */
  handleKeydown(event: KeyboardEvent): boolean {
    const typing =
      event.target instanceof HTMLElement &&
      (event.target.isContentEditable ||
        ["INPUT", "TEXTAREA", "SELECT"].includes(event.target.tagName));
    for (const command of this.commands.values()) {
      const { shortcut } = command;
      if (!shortcut || (typing && !shortcut.mod)) continue;
      if (matchesShortcut(shortcut, event, this.mac)) {
        event.preventDefault();
        void command.run();
        return true;
      }
    }
    return false;
  }
}
