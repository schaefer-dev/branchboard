// Native context menus from a declarative description.

import { Menu } from "@tauri-apps/api/menu";

export type MenuEntry =
  | { kind: "item"; text: string; run: () => void; enabled?: boolean; accelerator?: string }
  | { kind: "check"; text: string; checked: boolean; run: () => void }
  | { kind: "submenu"; text: string; items: MenuEntry[]; enabled?: boolean }
  | { kind: "separator" };

type NativeItem = NonNullable<Parameters<typeof Menu.new>[0]>["items"] extends
  Array<infer T> | undefined
  ? T
  : never;

function toNative(entry: MenuEntry): NativeItem {
  switch (entry.kind) {
    case "item":
      return {
        text: entry.text,
        enabled: entry.enabled ?? true,
        ...(entry.accelerator ? { accelerator: entry.accelerator } : {}),
        action: entry.run,
      };
    case "check":
      return { text: entry.text, checked: entry.checked, action: entry.run };
    case "submenu":
      return { text: entry.text, enabled: entry.enabled ?? true, items: entry.items.map(toNative) };
    case "separator":
      return { item: "Separator" };
  }
}

/** Shows a native context menu at the mouse position. */
export async function showContextMenu(entries: MenuEntry[]): Promise<void> {
  const menu = await Menu.new({ items: entries.map(toNative) });
  try {
    await menu.popup();
  } finally {
    await menu.close();
  }
}
