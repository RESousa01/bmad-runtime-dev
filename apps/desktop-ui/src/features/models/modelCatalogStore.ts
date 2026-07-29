/**
 * FND-050: the catalog/selection store.
 *
 * A pure reducer, not a hook -- so the loading/ready/stale/unavailable/
 * refreshing/error transitions can be tested without React, a fake host
 * client, or a timer. The one rule that matters here: a reply is only ever
 * applied if it answers the binding that is still current. A workspace
 * switch, a re-grant (`workspaceGrantEpoch`), or a model-identity change
 * (`appEpoch`) between the request and the reply must make that reply inert,
 * because applying it would show a catalog or a selection that belongs to a
 * workspace or an identity the user has since left.
 */

import type { ModelCatalogOutcome, ModelCatalogProjection } from "../../lib/hostClient";

/** The workspace and model-identity binding one catalog request was made under. */
export interface ModelCatalogBinding {
  readonly workspaceId: string;
  readonly workspaceGrantEpoch: number;
  readonly appEpoch: number;
}

export type ModelCatalogStoreState =
  | { readonly kind: "loading" }
  | { readonly kind: "ready"; readonly catalog: ModelCatalogProjection }
  | { readonly kind: "stale"; readonly catalog: ModelCatalogProjection }
  | { readonly kind: "unavailable" }
  | { readonly kind: "refreshing"; readonly catalog: ModelCatalogProjection | null }
  | {
    readonly kind: "error";
    readonly message: string;
    /** The last catalog known good, if any -- a transport failure must not
     * discard a perfectly usable stale catalog the user was already looking
     * at. */
    readonly catalog: ModelCatalogProjection | null;
  };

export const initialModelCatalogStoreState: ModelCatalogStoreState = { kind: "loading" };

export type ModelCatalogStoreEvent =
  | { readonly type: "load_started" }
  | { readonly type: "refresh_started" }
  | {
    readonly type: "outcome";
    readonly requested: ModelCatalogBinding;
    readonly current: ModelCatalogBinding;
    readonly outcome: ModelCatalogOutcome;
  }
  | {
    readonly type: "failed";
    readonly requested: ModelCatalogBinding;
    readonly current: ModelCatalogBinding;
    readonly message: string;
  };

function priorCatalog(state: ModelCatalogStoreState): ModelCatalogProjection | null {
  switch (state.kind) {
    case "ready":
    case "stale":
      return state.catalog;
    case "refreshing":
    case "error":
      return state.catalog;
    case "loading":
    case "unavailable":
      return null;
  }
}

/// Whether a request made under `requested` still answers to `current`. Not
/// just an equality check on the workspace: a re-grant or an identity change
/// both mint a new epoch, so comparing every field is what makes a late reply
/// from before either change detectably stale.
export function modelCatalogBindingIsCurrent(
  requested: ModelCatalogBinding,
  current: ModelCatalogBinding,
): boolean {
  return requested.workspaceId === current.workspaceId
    && requested.workspaceGrantEpoch === current.workspaceGrantEpoch
    && requested.appEpoch === current.appEpoch;
}

function bucketFor(catalog: ModelCatalogProjection): "ready" | "stale" {
  // `degraded` is folded into the same "treat with suspicion" bucket as
  // `stale` rather than getting a seventh top-level state: both mean "usable,
  // but do not present this as current truth" to a caller that only reads the
  // coarse `kind`. `catalog.catalogState` itself is preserved unmodified for
  // any caller that wants the finer distinction.
  return catalog.catalogState === "fresh" ? "ready" : "stale";
}

export function transitionModelCatalogStore(
  state: ModelCatalogStoreState,
  event: ModelCatalogStoreEvent,
): ModelCatalogStoreState {
  switch (event.type) {
    case "load_started":
      return { kind: "loading" };
    case "refresh_started":
      return { kind: "refreshing", catalog: priorCatalog(state) };
    case "outcome": {
      if (!modelCatalogBindingIsCurrent(event.requested, event.current)) {
        // A late reply for a binding nothing is asking about anymore. Ignored
        // rather than applied -- the whole point of carrying the binding
        // through the async gap.
        return state;
      }
      if (event.outcome.kind === "unavailable") {
        return { kind: "unavailable" };
      }
      return { kind: bucketFor(event.outcome.catalog), catalog: event.outcome.catalog };
    }
    case "failed": {
      if (!modelCatalogBindingIsCurrent(event.requested, event.current)) {
        return state;
      }
      return { kind: "error", message: event.message, catalog: priorCatalog(state) };
    }
  }
}
