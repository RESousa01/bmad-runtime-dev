/**
 * FND-050 Phase A: the model picker.
 *
 * Renders from two host-confirmed projections only -- `ModelCatalogProjection`
 * (by way of `ModelCatalogStoreState`, so loading/stale/refreshing/error/
 * unavailable are all distinct, not folded into one boolean) and
 * `ModelSelectionProjection`. Selection is never optimistic: picking a row
 * calls `onSelectProfile` and the caller is expected to wait for the host's
 * confirmed reply before this component's `selection` prop changes. Nothing
 * here renders a deployment name, endpoint, subscription, resource group,
 * identity, API version, policy hash, or credential -- the projection types
 * this component reads have nowhere to carry any of those.
 */

import { useMemo, useRef, useState } from "react";
import type { ModelCatalogStoreState } from "./modelCatalogStore";
import type {
  ModelProfileProjection,
  ModelSelectionProjection,
} from "../../lib/hostClient/modelProtocol";
import { ModelProfileRow } from "./ModelProfileRow";
import { ModelUnavailableReason } from "./ModelAvailability";

export interface ModelPickerProps {
  readonly catalogStore: ModelCatalogStoreState;
  readonly selection: ModelSelectionProjection | null;
  /** Profile ids the user picked previously in this workspace, most recent
   * first. Purely a display hint -- never used to pick anything on the
   * user's behalf. */
  readonly recentProfileIds?: readonly string[];
  readonly onSelectProfile: (profileId: string, variantId: string) => void;
  readonly onRefresh: () => void;
}

/// Subsequence fuzzy match: every character of `query`, in order, somewhere
/// in `haystack`. Cheap, dependency-free, and forgiving of typos in a way a
/// plain substring match is not -- this repo does not add a dependency for a
/// popover search box.
function fuzzyMatches(query: string, haystack: string): boolean {
  if (query.length === 0) return true;
  const q = query.toLowerCase();
  const h = haystack.toLowerCase();
  let qi = 0;
  for (let hi = 0; hi < h.length && qi < q.length; hi += 1) {
    if (h[hi] === q[qi]) qi += 1;
  }
  return qi === q.length;
}

function searchableText(profile: ModelProfileProjection): string {
  return [profile.displayName, profile.description, profile.family ?? ""].join(" ");
}

function moveFocus(container: HTMLElement | null, direction: 1 | -1) {
  if (!container) return;
  const rows = Array.from(
    container.querySelectorAll<HTMLButtonElement>(".model-picker__profile-row"),
  );
  if (rows.length === 0) return;
  const current = rows.findIndex((row) => row === document.activeElement);
  const next = current === -1
    ? (direction === 1 ? 0 : rows.length - 1)
    : (current + direction + rows.length) % rows.length;
  rows[next]?.focus();
}

export function ModelPicker({
  catalogStore,
  selection,
  recentProfileIds = [],
  onSelectProfile,
  onRefresh,
}: ModelPickerProps) {
  const [query, setQuery] = useState("");
  const listRef = useRef<HTMLUListElement>(null);

  const catalog = catalogStore.kind === "ready" || catalogStore.kind === "stale"
    ? catalogStore.catalog
    : catalogStore.kind === "refreshing" || catalogStore.kind === "error"
      ? catalogStore.catalog
      : null;

  const filteredProfiles = useMemo(() => {
    if (!catalog) return [];
    return catalog.profiles.filter((profile) => fuzzyMatches(query, searchableText(profile)));
  }, [catalog, query]);

  const isRefreshing = catalogStore.kind === "refreshing";

  if (catalogStore.kind === "loading") {
    return (
      <div className="model-picker model-picker--loading" role="status" aria-live="polite">
        Loading model catalog…
      </div>
    );
  }

  if (catalogStore.kind === "unavailable") {
    return (
      <div className="model-picker model-picker--unavailable" role="status">
        <p>No model catalog is available for this workspace.</p>
        <button
          className="model-picker__refresh"
          onClick={onRefresh}
          type="button"
        >
          Refresh
        </button>
      </div>
    );
  }

  if (!catalog) {
    // `error` with no prior catalog: nothing to show but the failure.
    const message = catalogStore.kind === "error" ? catalogStore.message : "Model catalog is unavailable.";
    return (
      <div className="model-picker model-picker--error" role="alert">
        <p>{message}</p>
        <button className="model-picker__refresh" onClick={onRefresh} type="button">
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="model-picker">
      {catalogStore.kind === "error" ? (
        <p className="model-picker__banner model-picker__banner--error" role="alert">
          {catalogStore.message} Showing the last known catalog.
        </p>
      ) : null}
      {catalog.catalogState !== "fresh" ? (
        <p className="model-picker__banner model-picker__banner--stale" role="status">
          {catalog.catalogState === "stale"
            ? "This catalog may be out of date."
            : "This catalog is degraded -- some profiles may not answer."}
        </p>
      ) : null}
      {selection?.status === "requires_explicit_choice" ? (
        <p className="model-picker__banner model-picker__banner--action" role="alert">
          Your previous model is no longer available
          {selection.blocked === "profile_removed" ? " (it was removed)." : null}
          {selection.blocked === "profile_unavailable" ? " (it is no longer available)." : null}
          {selection.blocked === "variant_removed" ? " (its variant was removed)." : null}
          {" "}Choose a model below to continue.
        </p>
      ) : null}
      {selection?.status === "none_selectable" ? (
        <p className="model-picker__banner model-picker__banner--action" role="alert">
          No model is available to select for this workspace.
        </p>
      ) : null}
      <div className="model-picker__toolbar">
        <label className="sr-only" htmlFor="model-picker-search">
          Search models
        </label>
        <input
          id="model-picker-search"
          className="model-picker__search"
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search models…"
          type="search"
          value={query}
        />
        <button
          aria-busy={isRefreshing}
          className="model-picker__refresh"
          disabled={isRefreshing}
          onClick={onRefresh}
          type="button"
        >
          {isRefreshing ? "Refreshing…" : "Refresh"}
        </button>
      </div>
      {filteredProfiles.length === 0 ? (
        <p className="model-picker__empty">
          {catalog.profiles.length === 0
            ? "No models are entitled to this workspace."
            : "No models match your search."}
        </p>
      ) : (
        <ul
          className="model-picker__profiles"
          ref={listRef}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") {
              event.preventDefault();
              moveFocus(listRef.current, 1);
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              moveFocus(listRef.current, -1);
            }
          }}
        >
          {filteredProfiles.map((profile) => (
            <ModelProfileRow
              isDefault={profile.profileId === catalog.defaultProfileId}
              isRecent={recentProfileIds.includes(profile.profileId)}
              isSelected={selection?.status === "selected" && selection.profileId === profile.profileId}
              key={profile.profileId}
              onSelectProfile={onSelectProfile}
              profile={profile}
              selectedVariantId={selection?.status === "selected" && selection.profileId === profile.profileId
                ? selection.variantId
                : null}
            />
          ))}
        </ul>
      )}
      {catalog.unavailableProfiles.length > 0 ? (
        <details className="model-picker__unavailable">
          <summary>Not offered to you ({catalog.unavailableProfiles.length})</summary>
          <ul className="model-picker__unavailable-list">
            {catalog.unavailableProfiles.map((profile) => (
              <ModelUnavailableReason
                availability={profile.availability}
                displayName={profile.displayName}
                key={profile.profileId}
                recoveryAction={profile.recoveryAction}
              />
            ))}
          </ul>
        </details>
      ) : null}
    </div>
  );
}
