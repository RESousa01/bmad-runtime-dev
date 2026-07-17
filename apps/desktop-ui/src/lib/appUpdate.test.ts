import { describe, expect, it, vi } from "vitest";
import { installAppUpdate } from "./appUpdate";

describe("installAppUpdate", () => {
  it("invokes the native update command and validates its result", async () => {
    const invoke = vi.fn(async () => ({ state: "current", version: "0.1.0-beta.1" }));

    await expect(installAppUpdate(async () => invoke)).resolves.toEqual({
      state: "current",
      version: "0.1.0-beta.1",
    });
    expect(invoke).toHaveBeenCalledWith("install_app_update");
  });

  it("rejects malformed native responses", async () => {
    const invoke = vi.fn(async () => ({ state: "installed", version: "" }));

    await expect(installAppUpdate(async () => invoke)).rejects.toThrow(
      "Invalid app update response",
    );
  });
});