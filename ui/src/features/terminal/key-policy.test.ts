import { describe, expect, it } from "vitest";

import { isCopy, keyOwner } from "./key-policy";

const key = (
  key: string,
  mods: Partial<Record<"meta" | "ctrl" | "shift" | "alt", boolean>> = {},
) => ({
  key,
  metaKey: mods.meta ?? false,
  ctrlKey: mods.ctrl ?? false,
  shiftKey: mods.shift ?? false,
  altKey: mods.alt ?? false,
});

describe("terminal key policy", () => {
  it("gives Cmd chords to the app on macOS", () => {
    expect(keyOwner(key("k", { meta: true }), true)).toBe("app");
    expect(keyOwner(key("v", { meta: true }), true)).toBe("app");
    expect(keyOwner(key("k", { ctrl: true }), true)).toBe("terminal");
    expect(keyOwner(key("b", { ctrl: true }), true)).toBe("terminal");
    expect(keyOwner(key("Enter", { shift: true }), true)).toBe("terminal");
    // Option composes characters such as @ on German layouts.
    expect(keyOwner(key("@", { alt: true }), true)).toBe("terminal");
  });

  it("keeps Ctrl for the terminal elsewhere", () => {
    expect(keyOwner(key("k", { ctrl: true }), false)).toBe("terminal");
    expect(keyOwner(key("c", { ctrl: true }), false)).toBe("terminal");
    expect(keyOwner(key("T", { ctrl: true, shift: true }), false)).toBe("app");
    expect(keyOwner(key("1", { ctrl: true }), false)).toBe("app");
    // AltGr arrives as Ctrl+Alt on Windows (e.g. @ on German layouts).
    expect(keyOwner(key("@", { ctrl: true, alt: true }), false)).toBe("terminal");
  });

  it("copies with Ctrl+Shift+C outside macOS", () => {
    expect(isCopy(key("C", { ctrl: true, shift: true }), false)).toBe(true);
    expect(isCopy(key("c", { ctrl: true }), false)).toBe(false);
    expect(isCopy(key("C", { ctrl: true, shift: true }), true)).toBe(false);
  });
});
