import { describe, expect, it } from "vitest";
import type { ModelCatalogProjection } from "../../lib/hostClient";
import {
  type ModelCatalogBinding,
  initialModelCatalogStoreState,
  modelCatalogBindingIsCurrent,
  transitionModelCatalogStore,
} from "./modelCatalogStore";

function binding(overrides: Partial<ModelCatalogBinding> = {}): ModelCatalogBinding {
  return { workspaceId: "workspace_1", workspaceGrantEpoch: 1, appEpoch: 1, ...overrides };
}

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
    profiles: [{ profileId: "balanced", displayName: "Balanced", summary: null, variantIds: ["standard"] }],
    unavailableProfiles: [],
    generatedAt: 1_000,
    expiresAt: 61_000,
    refreshedAt: 1_200,
    ...overrides,
  };
}

describe("modelCatalogBindingIsCurrent", () => {
  it("agrees when every field matches", () => {
    expect(modelCatalogBindingIsCurrent(binding(), binding())).toBe(true);
  });

  it("disagrees on a workspace switch, a re-grant, or an identity change", () => {
    expect(modelCatalogBindingIsCurrent(binding(), binding({ workspaceId: "workspace_2" }))).toBe(false);
    expect(modelCatalogBindingIsCurrent(binding(), binding({ workspaceGrantEpoch: 2 }))).toBe(false);
    expect(modelCatalogBindingIsCurrent(binding(), binding({ appEpoch: 2 }))).toBe(false);
  });
});

describe("transitionModelCatalogStore", () => {
  it("starts loading", () => {
    expect(initialModelCatalogStoreState).toEqual({ kind: "loading" });
  });

  it("buckets a fresh catalog as ready and a stale one as stale", () => {
    const ready = transitionModelCatalogStore(initialModelCatalogStoreState, {
      type: "outcome",
      requested: binding(),
      current: binding(),
      outcome: { kind: "catalog", catalog: catalog() },
    });
    expect(ready.kind).toBe("ready");

    const stale = transitionModelCatalogStore(initialModelCatalogStoreState, {
      type: "outcome",
      requested: binding(),
      current: binding(),
      outcome: { kind: "catalog", catalog: catalog({ catalogState: "stale" }) },
    });
    expect(stale.kind).toBe("stale");
  });

  it("reports unavailable distinctly from an empty selectable list", () => {
    const state = transitionModelCatalogStore(initialModelCatalogStoreState, {
      type: "outcome",
      requested: binding(),
      current: binding(),
      outcome: { kind: "unavailable" },
    });
    expect(state).toEqual({ kind: "unavailable" });
  });

  it("ignores a reply whose binding is no longer current", () => {
    const loading = initialModelCatalogStoreState;
    const state = transitionModelCatalogStore(loading, {
      type: "outcome",
      requested: binding({ workspaceGrantEpoch: 1 }),
      current: binding({ workspaceGrantEpoch: 2 }),
      outcome: { kind: "catalog", catalog: catalog() },
    });
    expect(state).toBe(loading);
  });

  it("keeps the last known-good catalog through a refresh and through an error", () => {
    const ready = transitionModelCatalogStore(initialModelCatalogStoreState, {
      type: "outcome",
      requested: binding(),
      current: binding(),
      outcome: { kind: "catalog", catalog: catalog() },
    });
    const refreshing = transitionModelCatalogStore(ready, { type: "refresh_started" });
    expect(refreshing.kind).toBe("refreshing");
    expect(refreshing.kind === "refreshing" && refreshing.catalog).toEqual(catalog());

    const failed = transitionModelCatalogStore(refreshing, {
      type: "failed",
      requested: binding(),
      current: binding(),
      message: "The Windows host is unreachable.",
    });
    expect(failed.kind).toBe("error");
    expect(failed.kind === "error" && failed.catalog).toEqual(catalog());
  });

  it("ignores a stale failure the same way it ignores a stale success", () => {
    const loading = initialModelCatalogStoreState;
    const state = transitionModelCatalogStore(loading, {
      type: "failed",
      requested: binding({ appEpoch: 1 }),
      current: binding({ appEpoch: 2 }),
      message: "stale",
    });
    expect(state).toBe(loading);
  });
});
