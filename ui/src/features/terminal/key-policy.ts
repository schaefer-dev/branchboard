// Which keys the embedded terminal keeps and which reach Branchboard's
// shortcuts while the terminal has focus.

type KeyEventLike = Pick<KeyboardEvent, "key" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey">;

/**
 * On macOS every Cmd chord belongs to the app (copy and paste included);
 * everything else goes to the terminal. Elsewhere Ctrl belongs to the
 * terminal (Ctrl+C, Ctrl+K, …), so only Ctrl+Shift chords and Ctrl+digit
 * reach the app.
 */
export function keyOwner(event: KeyEventLike, mac: boolean): "app" | "terminal" {
  if (mac) return event.metaKey ? "app" : "terminal";
  if (event.ctrlKey && !event.altKey && (event.shiftKey || /^[0-9]$/.test(event.key))) {
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
