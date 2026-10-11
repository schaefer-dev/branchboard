// Native dialogs.

import { ask, open } from "@tauri-apps/plugin-dialog";

/** Lets the user pick one or more folders. Resolves to `[]` if cancelled. */
export async function pickFolders(title: string, multiple = true): Promise<string[]> {
  const result = await open({ title, directory: true, multiple });
  if (result === null) return [];
  return Array.isArray(result) ? result : [result];
}

/** Asks for confirmation of a destructive action. */
export function confirmDestructive(
  title: string,
  message: string,
  okLabel: string,
): Promise<boolean> {
  return ask(message, { title, kind: "warning", okLabel, cancelLabel: "Cancel" });
}
