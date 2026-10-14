import { describe, expect, it, vi } from "vitest";

import { CommandRegistry, formatShortcut, matchesShortcut } from "./registry";

const key = (
  k: string,
  mods: Partial<Record<"metaKey" | "ctrlKey" | "shiftKey" | "altKey", boolean>> = {},
) => ({
  key: k,
  metaKey: false,
  ctrlKey: false,
  shiftKey: false,
  altKey: false,
  ...mods,
});

describe("matchesShortcut", () => {
  it("maps mod to Cmd on macOS and Ctrl elsewhere", () => {
    const shortcut = { key: "p", mod: true };
    expect(matchesShortcut(shortcut, key("p", { metaKey: true }), true)).toBe(true);
    expect(matchesShortcut(shortcut, key("p", { ctrlKey: true }), true)).toBe(false);
    expect(matchesShortcut(shortcut, key("P", { ctrlKey: true }), false)).toBe(true);
  });

  it("requires exact modifiers", () => {
    expect(
      matchesShortcut({ key: "p", mod: true }, key("p", { metaKey: true, shiftKey: true }), true),
    ).toBe(false);
    expect(matchesShortcut({ key: "F2" }, key("F2"), false)).toBe(true);
  });
});

describe("formatShortcut", () => {
  it("uses platform conventions", () => {
    expect(formatShortcut({ key: "p", mod: true, shift: true }, true)).toBe("⇧⌘P");
    expect(formatShortcut({ key: "p", mod: true, shift: true }, false)).toBe("Ctrl+Shift+P");
  });
});

describe("CommandRegistry", () => {
  it("runs the matching command and rejects duplicate ids", () => {
    const registry = new CommandRegistry(true);
    const run = vi.fn();
    const unregister = registry.register({
      id: "a",
      title: "A",
      shortcut: { key: "o", mod: true },
      run,
    });
    expect(() => registry.register({ id: "a", title: "A", run })).toThrow();

    const event = new KeyboardEvent("keydown", { key: "o", metaKey: true });
    expect(registry.handleKeydown(event)).toBe(true);
    expect(run).toHaveBeenCalledOnce();

    unregister();
    expect(registry.handleKeydown(event)).toBe(false);
  });

  it("skips commands that do not apply", () => {
    const registry = new CommandRegistry(false);
    let enabled = false;
    const run = vi.fn();
    registry.register({
      id: "a",
      title: "A",
      shortcut: { key: "k", mod: true },
      enabled: () => enabled,
      run,
    });
    registry.register({ id: "b", title: "B", run });

    expect(registry.available().map((command) => command.id)).toEqual(["b"]);
    expect(registry.handleKeydown(new KeyboardEvent("keydown", { key: "k", ctrlKey: true }))).toBe(
      false,
    );
    enabled = true;
    expect(registry.available().map((command) => command.id)).toEqual(["a", "b"]);
  });

  it("matches alternate keys of a shortcut", () => {
    const larger = { key: "+", alternateKeys: ["="], mod: true };
    const event = (key: string, shiftKey = false) => ({
      key,
      metaKey: true,
      ctrlKey: false,
      shiftKey,
      altKey: false,
    });
    // German layouts have a "+" key, US layouts put it on Shift+"=".
    expect(matchesShortcut(larger, event("+"), true)).toBe(true);
    expect(matchesShortcut(larger, event("="), true)).toBe(true);
    expect(matchesShortcut(larger, event("-"), true)).toBe(false);
  });
});
