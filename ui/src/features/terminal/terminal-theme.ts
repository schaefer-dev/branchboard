// Terminal colors from the design tokens, so the embedded herdr matches the
// app in light and dark mode. herdr draws its own UI with these as the
// default foreground and background.

import type { ITheme } from "@xterm/xterm";

function token(styles: CSSStyleDeclaration, name: string, fallback: string): string {
  return styles.getPropertyValue(name).trim() || fallback;
}

export function terminalTheme(): ITheme {
  const styles = getComputedStyle(document.documentElement);
  const text = token(styles, "--color-text", "#1c1f24");
  return {
    background: token(styles, "--color-surface", "#ffffff"),
    foreground: text,
    cursor: text,
    cursorAccent: token(styles, "--color-surface", "#ffffff"),
    selectionBackground: token(styles, "--color-row-selected", "#dde3ec"),
  };
}

/** Menlo where it exists (macOS), else the app's monospace font. */
export function terminalFont(): string {
  return `Menlo, ${token(getComputedStyle(document.documentElement), "--font-mono", "monospace")}`;
}
