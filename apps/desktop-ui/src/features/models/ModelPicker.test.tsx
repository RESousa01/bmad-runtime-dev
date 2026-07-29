import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import type { ModelCatalogProjection } from "../../lib/hostClient";
import type { ModelSelectionProjection } from "../../lib/hostClient/modelProtocol";
import type { ModelCatalogStoreState } from "./modelCatalogStore";
import { ModelPicker } from "./ModelPicker";

function catalog(overrides: Partial<ModelCatalogProjection> = {}): ModelCatalogProjection {
  return {
    schemaVersion: "sapphirus.model-catalog-projection.v1",
    workspaceId: "workspace_1",
    workspaceGrantEpoch: 1,
    appEpoch: 1,
    catalogEpoch: 1,
    catalogVersion: "v1.abc",
    catalogState: "fresh",
    defaultProfileId: "balanced",
    profiles: [
      {
        profileId: "balanced",
        displayName: "Balanced",
        description: "General drafting and review.",
        family: null,
        provider: "microsoft_foundry",
        availability: "available",
        statusDetail: null,
        capabilities: {
          textInput: true,
          imageInput: false,
          pdfInput: false,
          attachments: true,
          toolCalling: true,
          structuredOutput: true,
          reasoning: false,
        },
        limits: {
          contextTokens: null,
          maxInputTokens: null,
          maxOutputTokens: null,
          maxAttachmentBytes: null,
        },
        variants: [
          { variantId: "standard", displayName: "Standard", reasoningEffort: "medium", maxOutputTokens: 4_096 },
        ],
        defaultVariantId: "standard",
        qualifiedAt: null,
      },
      {
        profileId: "reasoning-deep",
        displayName: "Deep Reasoning",
        description: "Slow, careful multi-step analysis.",
        family: "reasoning",
        provider: "microsoft_foundry",
        availability: "degraded",
        statusDetail: "Elevated latency reported by the operator.",
        capabilities: {
          textInput: true,
          imageInput: true,
          pdfInput: true,
          attachments: true,
          toolCalling: false,
          structuredOutput: false,
          reasoning: true,
        },
        limits: {
          contextTokens: 200_000,
          maxInputTokens: 180_000,
          maxOutputTokens: 32_000,
          maxAttachmentBytes: 10_485_760,
        },
        variants: [
          { variantId: "low", displayName: "Low", reasoningEffort: "low", maxOutputTokens: 8_000 },
          { variantId: "high", displayName: "High", reasoningEffort: "high", maxOutputTokens: 32_000 },
        ],
        defaultVariantId: "low",
        qualifiedAt: "2026-07-01T00:00:00.000Z",
      },
    ],
    unavailableProfiles: [
      {
        profileId: "vision-pro",
        displayName: "Vision Pro",
        availability: "not_entitled",
        recoveryAction: "request_entitlement",
      },
    ],
    generatedAt: 1_000,
    expiresAt: 61_000,
    refreshedAt: 1_200,
    ...overrides,
  };
}

function selected(overrides: Partial<Extract<ModelSelectionProjection, { status: "selected" }>> = {}): ModelSelectionProjection {
  return {
    schemaVersion: "sapphirus.model-selection-projection.v1",
    workspaceId: "workspace_1",
    workspaceGrantEpoch: 1,
    appEpoch: 1,
    catalogEpoch: 1,
    catalogVersion: "v1.abc",
    status: "selected",
    profileId: "balanced",
    variantId: "standard",
    source: "current_selection",
    ...overrides,
  };
}

describe("ModelPicker", () => {
  it("shows a loading status without a catalog", () => {
    render(
      <ModelPicker
        catalogStore={{ kind: "loading" }}
        onRefresh={() => {}}
        onSelectProfile={() => {}}
        selection={null}
      />,
    );
    expect(screen.getByRole("status").textContent).toMatch(/loading model catalog/i);
  });

  it("shows the unavailable state and offers a refresh", () => {
    const onRefresh = vi.fn();
    render(
      <ModelPicker
        catalogStore={{ kind: "unavailable" }}
        onRefresh={onRefresh}
        onSelectProfile={() => {}}
        selection={null}
      />,
    );
    expect(screen.getByText(/no model catalog is available/i)).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("renders profiles, capabilities, and unknown limits as unknown, never zero", () => {
    render(
      <ModelPicker
        catalogStore={{ kind: "ready", catalog: catalog() }}
        onRefresh={() => {}}
        onSelectProfile={() => {}}
        selection={selected()}
      />,
    );
    const balancedRow = screen.getByRole("button", { name: /Balanced/ });
    expect(balancedRow.getAttribute("aria-pressed")).toBe("true");
    const limits = within(balancedRow).getByLabelText("Limits");
    expect(within(limits).getAllByText("Unknown").length).toBeGreaterThan(0);
    expect(within(limits).queryByText("0")).toBeNull();

    const deepRow = screen.getByRole("button", { name: /Deep Reasoning/ });
    expect(within(deepRow).getByText(/200,000 tokens/)).not.toBeNull();
    expect(within(deepRow).getByText("Degraded")).not.toBeNull();
    expect(within(deepRow).getByText(/Elevated latency reported/)).not.toBeNull();
  });

  it("calls onSelectProfile with the profile's default variant", () => {
    const onSelectProfile = vi.fn();
    render(
      <ModelPicker
        catalogStore={{ kind: "ready", catalog: catalog() }}
        onRefresh={() => {}}
        onSelectProfile={onSelectProfile}
        selection={selected()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Deep Reasoning/ }));
    expect(onSelectProfile).toHaveBeenCalledWith("reasoning-deep", "low");
  });

  it("filters profiles by fuzzy search", () => {
    render(
      <ModelPicker
        catalogStore={{ kind: "ready", catalog: catalog() }}
        onRefresh={() => {}}
        onSelectProfile={() => {}}
        selection={selected()}
      />,
    );
    fireEvent.change(screen.getByLabelText("Search models"), { target: { value: "dprsn" } });
    expect(screen.getByRole("button", { name: /Deep Reasoning/ })).not.toBeNull();
    expect(screen.queryByRole("button", { name: /^Balanced/ })).toBeNull();
  });

  it("shows a requires_explicit_choice banner instead of an error state", () => {
    render(
      <ModelPicker
        catalogStore={{ kind: "ready", catalog: catalog() }}
        onRefresh={() => {}}
        onSelectProfile={() => {}}
        selection={{
          schemaVersion: "sapphirus.model-selection-projection.v1",
          workspaceId: "workspace_1",
          workspaceGrantEpoch: 1,
          appEpoch: 1,
          catalogEpoch: 1,
          catalogVersion: "v1.abc",
          status: "requires_explicit_choice",
          previousProfileId: "balanced",
          previousVariantId: "standard",
          blocked: "profile_removed",
        }}
      />,
    );
    const alertText = screen.getByRole("alert").textContent ?? "";
    expect(alertText).toMatch(/no longer available/i);
    expect(alertText).toMatch(/choose a model below/i);
  });

  it("lists unavailable profiles with reason and recovery action", () => {
    render(
      <ModelPicker
        catalogStore={{ kind: "ready", catalog: catalog() }}
        onRefresh={() => {}}
        onSelectProfile={() => {}}
        selection={selected()}
      />,
    );
    fireEvent.click(screen.getByText(/Not offered to you/));
    expect(screen.getByText("Vision Pro")).not.toBeNull();
    expect(screen.getByText("Not entitled")).not.toBeNull();
    expect(screen.getByText(/Request entitlement/)).not.toBeNull();
  });

  it("shows a stale banner without discarding the catalog", () => {
    const state: ModelCatalogStoreState = { kind: "stale", catalog: catalog({ catalogState: "stale" }) };
    render(
      <ModelPicker
        catalogStore={state}
        onRefresh={() => {}}
        onSelectProfile={() => {}}
        selection={selected()}
      />,
    );
    expect(screen.getByText(/may be out of date/i)).not.toBeNull();
    expect(screen.getByRole("button", { name: /Balanced/ })).not.toBeNull();
  });
});
