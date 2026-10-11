import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { call, CommandError, toAppError } from "./client";

describe("toAppError", () => {
  it("passes structured backend errors through", () => {
    const error = { kind: "gitFailed", message: "boom", detail: "stderr" };
    expect(toAppError(error)).toEqual(error);
  });

  it("wraps plain strings and errors as internal errors", () => {
    expect(toAppError("nope")).toEqual({ kind: "internal", message: "nope", detail: null });
    expect(toAppError(new Error("bad"))).toEqual({
      kind: "internal",
      message: "bad",
      detail: null,
    });
    expect(toAppError(42)).toMatchObject({ kind: "internal", message: "Unexpected error" });
  });
});

describe("call", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("resolves with the command result", async () => {
    invoke.mockResolvedValue({ ok: true });
    await expect(call("cmd", { a: 1 })).resolves.toEqual({ ok: true });
    expect(invoke).toHaveBeenCalledWith("cmd", { a: 1 });
  });

  it("rejects with a CommandError", async () => {
    invoke.mockRejectedValue({
      kind: "gitNotFound",
      message: "git executable not found",
      detail: null,
    });
    const rejection = call("cmd");
    await expect(rejection).rejects.toBeInstanceOf(CommandError);
    await expect(rejection).rejects.toMatchObject({ error: { kind: "gitNotFound" } });
  });
});
