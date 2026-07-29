import { describe, expect, it } from "vitest";
import { parseModelCatalog } from "./modelProtocol";

const capabilities = {
  textInput: true,
  imageInput: false,
  pdfInput: false,
  attachments: true,
  toolCalling: true,
  structuredOutput: true,
  reasoning: false,
};

const noPublishedLimits = {
  contextTokens: null as number | null,
  maxInputTokens: null as number | null,
  maxOutputTokens: null as number | null,
  maxAttachmentBytes: null as number | null,
};

function variant() {
  return {
    variantId: "standard",
    displayName: "Standard",
    reasoningEffort: "medium",
    maxOutputTokens: 4_096,
  };
}

function profile(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    profileId: "balanced",
    displayName: "Balanced reasoning",
    description: "General drafting and review.",
    family: "balanced-family",
    provider: "microsoft_foundry",
    availability: "available",
    statusDetail: null,
    capabilities,
    limits: noPublishedLimits,
    variants: [variant()],
    defaultVariantId: "standard",
    qualifiedAt: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}

function catalog(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    schemaVersion: "sapphirus.model-catalog-projection.v1",
    workspaceId: "workspace_01ARZ3NDEKTSV4RRFFQ69G5FAV",
    workspaceGrantEpoch: 3,
    appEpoch: 5,
    catalogEpoch: 12,
    catalogVersion: "v12.abc1234567890a",
    catalogState: "fresh",
    defaultProfileId: "balanced",
    profiles: [profile()],
    unavailableProfiles: [],
    generatedAt: 1_000,
    expiresAt: 61_000,
    refreshedAt: 1_200,
    ...overrides,
  };
}

describe("model protocol validators", () => {
  it("accepts the widened profile projection", () => {
    const parsed = parseModelCatalog(catalog());
    const parsedProfile = parsed.profiles[0];
    if (!parsedProfile) throw new Error("expected one profile");
    expect(parsedProfile.description).toBe("General drafting and review.");
    expect(parsedProfile.family).toBe("balanced-family");
    expect(parsedProfile.provider).toBe("microsoft_foundry");
    expect(parsedProfile.availability).toBe("available");
    expect(parsedProfile.capabilities.toolCalling).toBe(true);
    expect(parsedProfile.variants[0]?.variantId).toBe("standard");
    expect(parsedProfile.defaultVariantId).toBe("standard");
    expect(parsedProfile.qualifiedAt).toBe("2026-01-01T00:00:00.000Z");
  });

  /// The property most likely to be lost by a careless default: a limit the
  /// organization did not publish must stay `null`, never `0` or an invented
  /// ceiling. Mirrors the Rust-side test in `crates/desktop-ipc/src/model.rs`.
  it("keeps a null limit null rather than collapsing it to zero", () => {
    const parsed = parseModelCatalog(
      catalog({
        profiles: [
          profile({
            limits: {
              contextTokens: 128_000,
              maxInputTokens: null,
              maxOutputTokens: 8_192,
              maxAttachmentBytes: null,
            },
          }),
        ],
      }),
    );
    const parsedProfile = parsed.profiles[0];
    if (!parsedProfile) throw new Error("expected one profile");
    const { limits } = parsedProfile;
    expect(limits.contextTokens).toBe(128_000);
    expect(limits.maxInputTokens).toBeNull();
    expect(limits.maxOutputTokens).toBe(8_192);
    expect(limits.maxAttachmentBytes).toBeNull();
  });

  it("rejects a profile missing any of the widened fields", () => {
    const { family: _family, ...withoutFamily } = profile();
    expect(() => parseModelCatalog(catalog({ profiles: [withoutFamily] }))).toThrow();
  });

  it("rejects a default variant id absent from the variant list", () => {
    expect(() =>
      parseModelCatalog(catalog({ profiles: [profile({ defaultVariantId: "missing" })] })),
    ).toThrow();
  });

  it("rejects a provider that is not microsoft_foundry", () => {
    expect(() =>
      parseModelCatalog(catalog({ profiles: [profile({ provider: "azure_openai" })] })),
    ).toThrow();
  });

  /// `qualifiedAt` is a validated `UtcInstant` string, not a parsed numeric
  /// instant. Mirrors the Rust-side `a_malformed_qualified_at_is_refused`.
  it("rejects a malformed qualifiedAt and accepts a null one", () => {
    expect(() =>
      parseModelCatalog(
        catalog({ profiles: [profile({ qualifiedAt: "2026-01-01 00:00:00" })] }),
      ),
    ).toThrow();
    const parsed = parseModelCatalog(catalog({ profiles: [profile({ qualifiedAt: null })] }));
    expect(parsed.profiles[0]?.qualifiedAt).toBeNull();
  });
});
