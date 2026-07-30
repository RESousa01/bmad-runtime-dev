/**
 * Renderer compatibility projection derived from the generated canonical
 * capability catalog. There is deliberately no hand-copied decision matrix in
 * this module: Library, skill selection, Help, and native dispatch all consume
 * projections of the same sealed catalog.
 */

import { bmadCapabilities } from "../../generated/bmadCapabilities";

export const COVERAGE_SOURCE = "packages/bmad-foundation/capability-catalog.json";
export const PINNED_UPSTREAM = "bmad-method 6.10.0";

export type BmadSnapshotState =
  | "identical"
  | "modified"
  | "relocated_or_legacy"
  | "missing";

export type BmadRuntimeState =
  | "active_projection"
  | "compatibility_shim"
  | "legacy_split_active"
  | "normalized_not_in_closure"
  | "not_projected"
  | "upstream_shim_unprojected";

export type BmadCoverageDecision = "adopt" | "adapt" | "shim" | "defer" | "reject";

export interface BmadCoverageRow {
  readonly upstreamSkill: string;
  readonly snapshotState: BmadSnapshotState;
  readonly runtimeState: BmadRuntimeState;
  readonly capabilityId: string | null;
  readonly decision: BmadCoverageDecision;
}

function upstreamSkill(path: string): string {
  const parts = path.split("/");
  return parts.at(-2) ?? "";
}

function runtimeState(row: (typeof bmadCapabilities)[number]): BmadRuntimeState {
  if (row.source.path.includes("/v6-shims/")) return "compatibility_shim";
  if (row.supportState === "implementation_pending") return "normalized_not_in_closure";
  if (row.supportState === "unavailable") return "not_projected";
  return "active_projection";
}

function snapshotState(row: (typeof bmadCapabilities)[number]): BmadSnapshotState {
  if (row.source.path.includes("/v6-shims/")) return "relocated_or_legacy";
  if (row.supportState === "adapted") return "modified";
  if (row.supportState === "unavailable") return "missing";
  return "identical";
}

function decision(row: (typeof bmadCapabilities)[number]): BmadCoverageDecision {
  if (row.source.path.includes("/v6-shims/")) return "shim";
  switch (row.supportState) {
    case "exact":
      return "adopt";
    case "adapted":
      return "adapt";
    case "implementation_pending":
      return "defer";
    case "unavailable":
      return "reject";
  }
}

/** The 48 Method source rows, projected directly from generated catalog data. */
export const BMAD_COVERAGE: readonly BmadCoverageRow[] = bmadCapabilities
  .filter((row) => row.source.id === "method")
  .map((row) => {
    const state = runtimeState(row);
    return {
      upstreamSkill: upstreamSkill(row.source.path),
      snapshotState: snapshotState(row),
      runtimeState: state,
      capabilityId:
        state === "active_projection" || state === "compatibility_shim"
          ? row.canonicalId
          : null,
      decision: decision(row),
    };
  });

const BY_UPSTREAM_SKILL: ReadonlyMap<string, BmadCoverageRow> = new Map(
  BMAD_COVERAGE.map((row) => [row.upstreamSkill, row]),
);

export function coverageForUpstreamSkill(upstream: string): BmadCoverageRow | null {
  return BY_UPSTREAM_SKILL.get(upstream) ?? null;
}

export function coverageForCapability(capabilityId: string): readonly BmadCoverageRow[] {
  return BMAD_COVERAGE.filter((row) => row.capabilityId === capabilityId);
}

export function capabilityIsProjected(capabilityId: string): boolean | null {
  const rows = coverageForCapability(capabilityId);
  if (rows.length === 0) return null;
  return rows.some(
    (row) => row.runtimeState === "active_projection" || row.runtimeState === "compatibility_shim",
  );
}
