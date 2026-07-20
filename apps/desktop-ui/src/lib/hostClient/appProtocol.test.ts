import { describe, expect, it } from "vitest";
import {
  parseAboutReply,
  parseOffboardingErasedReply,
  parsePreferencesReply,
  parseRetentionManifestReply,
} from "./appProtocol";

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

describe("offboarding protocol validators", () => {
  const manifestValue = {
    schemaVersion: "sapphirus.retention-manifest.v1",
    categories: [
      { category: "workspace_and_authority_records", count: 4 },
      { category: "evidence_events", count: 128 },
    ],
    retainedBytes: 262144,
  };

  it("accepts an exact retention manifest reply", () => {
    const parsed = parseRetentionManifestReply(
      dispatchReply({ kind: "retention_manifest", value: manifestValue }),
      requestId,
    );
    expect(parsed.projection.categories).toHaveLength(2);
    expect(parsed.projection.categories[1]?.count).toBe(128);
    expect(parsed.projection.retainedBytes).toBe(262144);
  });

  it("rejects category labels that could leak paths or identifiers", () => {
    for (const category of [
      "C:/Users/someone",
      String.raw`evidence\events`,
      "Evidence Events",
      "install_01ARZ3NDEKTSV4RRFFQ69G5FAV",
      "",
    ]) {
      expect(() =>
        parseRetentionManifestReply(
          dispatchReply({
            kind: "retention_manifest",
            value: {
              ...manifestValue,
              categories: [{ category, count: 1 }],
            },
          }),
          requestId,
        ),
      ).toThrow();
    }
  });

  it("rejects retention manifests with extra keys or a wrong kind", () => {
    expect(() =>
      parseRetentionManifestReply(
        dispatchReply({
          kind: "retention_manifest",
          value: { ...manifestValue, paths: [] },
        }),
        requestId,
      ),
    ).toThrow();
    expect(() =>
      parseRetentionManifestReply(
        dispatchReply({ kind: "about", value: manifestValue }),
        requestId,
      ),
    ).toThrow();
  });

  it("accepts only the erased terminal acknowledgement", () => {
    const parsed = parseOffboardingErasedReply(
      dispatchReply({
        kind: "offboarding_erased",
        value: {
          schemaVersion: "sapphirus.offboarding-erased.v1",
          status: "erased",
          restartRequired: true,
        },
      }),
      requestId,
    );
    expect(parsed.projection.restartRequired).toBe(true);
    expect(() =>
      parseOffboardingErasedReply(
        dispatchReply({
          kind: "offboarding_erased",
          value: {
            schemaVersion: "sapphirus.offboarding-erased.v1",
            status: "partial",
            restartRequired: true,
          },
        }),
        requestId,
      ),
    ).toThrow();
  });
});
