// Clipboard shortcuts bypass terminal encoding; other keys either stay in
// the terminal or reach Branchboard's shortcuts.

type KeyEventLike = Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey">;

/** Leave paste and Windows clipboard history to the WebView and OS. */
export function isNativePasteShortcut(event: KeyEventLike, mac: boolean): boolean {
  if (event.altKey) return false;
  if (event.key.toLowerCase() === "v") {
    if (mac) return event.metaKey && !event.ctrlKey && !event.shiftKey;
    return (
      (event.ctrlKey && !event.metaKey) || (event.metaKey && !event.ctrlKey && !event.shiftKey)
    );
  }
  return !mac && event.key === "Insert" && event.shiftKey && !event.ctrlKey && !event.metaKey;
}

/**
 * Used after native clipboard shortcuts have bypassed terminal encoding.
 * On macOS every Cmd chord belongs to the app (copy and paste included);
 * everything else goes to the terminal. Elsewhere Ctrl belongs to the
 * terminal (Ctrl+C, Ctrl+K, …), so only Ctrl+Shift chords, Ctrl+digit and,
 * as in Windows Terminal, the font size keys Ctrl+= / Ctrl+- reach the app.
 */
export function keyOwner(event: KeyEventLike, mac: boolean): "app" | "terminal" {
  if (mac) return event.metaKey ? "app" : "terminal";
  if (event.ctrlKey && !event.altKey && (event.shiftKey || /^[0-9=+-]$/.test(event.key))) {
    return "app";
  }
  return "terminal";
}

/** Ctrl+Shift+C outside macOS: copy the terminal's selection. */
export function isCopy(event: KeyEventLike, mac: boolean): boolean {
  return (
    !mac && event.ctrlKey && event.shiftKey && !event.altKey && event.key.toLowerCase() === "c"
  );
}
