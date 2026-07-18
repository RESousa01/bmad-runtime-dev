import { describe, expect, it } from "vitest";
import { parseAboutReply, parsePreferencesReply } from "./appProtocol";

const requestId = "req_01ARZ3NDEKTSV4RRFFQ69G5FAV";

function dispatchReply(data: unknown) {
  return {
    schemaVersion: "desktop-dispatch-reply.v1",
    requestId,
    sequence: 7,
    status: "ok",
    receipt: {
      requestId,
      acceptedAt: 1_725_000_000_005,
      operationId: null as string | null,
    },
    data,
  };
}

const preferencesValue = {
  schemaVersion: "desktop-preferences.v1",
  theme: "system",
  density: "compact",
  updatedAt: 1_725_000_000_000,
};

const aboutValue = {
  appVersion: "0.1.0",
  installationId: "install_01ARZ3NDEKTSV4RRFFQ69G5FAV",
  bootMode: "ready",
  foundationPackageName: "bmad-method",
  foundationPackageVersion: "6.10.0",
  inactiveBuilderPackageCount: 2,
  updateConfigured: false,
  updateInstallAvailable: false,
};

describe("app protocol validators", () => {
  it("accepts an exact preferences reply", () => {
    const parsed = parsePreferencesReply(
      dispatchReply({ kind: "preferences", value: preferencesValue }),
      requestId,
    );
    expect(parsed.projection.theme).toBe("system");
    expect(parsed.projection.density).toBe("compact");
    expect(parsed.projection.updatedAt).toBe(1_725_000_000_000);
    expect(parsed.sequence).toBe(7);
  });

  it("accepts a null updatedAt for default preferences", () => {
    const parsed = parsePreferencesReply(
      dispatchReply({
        kind: "preferences",
        value: { ...preferencesValue, updatedAt: null },
      }),
      requestId,
    );
    expect(parsed.projection.updatedAt).toBeNull();
  });

  it("rejects snake_case casing drift in preferences", () => {
    expect(() =>
      parsePreferencesReply(
        dispatchReply({
          kind: "preferences",
          value: {
            schema_version: "desktop-preferences.v1",
            theme: "dark",
            density: "compact",
            updated_at: null,
          },
        }),
        requestId,
      ),
    ).toThrow();
  });

  it("rejects unknown preference keys and values", () => {
    expect(() =>
      parsePreferencesReply(
        dispatchReply({
          kind: "preferences",
          value: { ...preferencesValue, accent: "crimson" },
        }),
        requestId,
      ),
    ).toThrow();
    expect(() =>
      parsePreferencesReply(
        dispatchReply({
          kind: "preferences",
          value: { ...preferencesValue, theme: "crimson" },
        }),
        requestId,
      ),
    ).toThrow();
  });

  it("accepts an exact about reply", () => {
    const parsed = parseAboutReply(
      dispatchReply({ kind: "about", value: aboutValue }),
      requestId,
    );
    expect(parsed.projection.appVersion).toBe("0.1.0");
    expect(parsed.projection.inactiveBuilderPackageCount).toBe(2);
    expect(parsed.projection.updateInstallAvailable).toBe(false);
  });

  it("rejects an about reply with extra keys or a wrong kind", () => {
    expect(() =>
      parseAboutReply(
        dispatchReply({
          kind: "about",
          value: { ...aboutValue, installPath: "C:/secret" },
        }),
        requestId,
      ),
    ).toThrow();
    expect(() =>
      parseAboutReply(
        dispatchReply({ kind: "preferences", value: aboutValue }),
        requestId,
      ),
    ).toThrow();
  });
});
