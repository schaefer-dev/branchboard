import { describe, expect, it } from "vitest";

import { isCopy, isNativePasteShortcut, keyOwner } from "./key-policy";

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
  it("leaves Windows paste and clipboard history shortcuts native", () => {
    for (const event of [
      key("v", { ctrl: true }),
      key("V", { ctrl: true, shift: true }),
      key("Insert", { shift: true }),
      key("v", { meta: true }),
    ]) {
      expect(isNativePasteShortcut(event, false)).toBe(true);
    }
  });

  it("only treats Cmd+V without extra modifiers as native paste on macOS", () => {
    expect(isNativePasteShortcut(key("v", { meta: true }), true)).toBe(true);
    expect(isNativePasteShortcut(key("V", { meta: true }), true)).toBe(true);
    for (const event of [
      key("v", { ctrl: true }),
      key("V", { ctrl: true, shift: true }),
      key("Insert", { shift: true }),
      key("V", { meta: true, shift: true }),
    ]) {
      expect(isNativePasteShortcut(event, true)).toBe(false);
    }
  });

  it("does not mistake typing, AltGr or extra modifiers for native paste", () => {
    for (const mac of [false, true]) {
      for (const event of [
        key("v"),
        key("V", { shift: true }),
        key("c", { ctrl: true }),
        key("v", { ctrl: true, alt: true }),
        key("v", { meta: true, alt: true }),
        key("v", { ctrl: true, meta: true }),
        key("V", { meta: true, shift: true }),
        key("Insert"),
        key("Insert", { ctrl: true, shift: true }),
        key("Insert", { alt: true, shift: true }),
        key("Insert", { meta: true, shift: true }),
      ]) {
        expect(isNativePasteShortcut(event, mac)).toBe(false);
      }
    }
  });

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
    // Font size, as in Windows Terminal.
    expect(keyOwner(key("=", { ctrl: true }), false)).toBe("app");
    expect(keyOwner(key("-", { ctrl: true }), false)).toBe("app");
    // AltGr arrives as Ctrl+Alt on Windows (e.g. @ on German layouts).
    expect(keyOwner(key("@", { ctrl: true, alt: true }), false)).toBe("terminal");
  });

  it("copies with Ctrl+Shift+C outside macOS", () => {
    expect(isCopy(key("C", { ctrl: true, shift: true }), false)).toBe(true);
    expect(isCopy(key("c", { ctrl: true }), false)).toBe(false);
    expect(isCopy(key("C", { ctrl: true, shift: true }), true)).toBe(false);
  });
});
