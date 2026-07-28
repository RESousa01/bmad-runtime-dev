/**
 * What the repo has recorded about each upstream BMAD skill, in the renderer.
 *
 * BM-02 keeps the authoritative matrix at `docs/bmad/skill-coverage.json`:
 * 48 upstream directories with a real `SKILL.md`, each ending in one explicit
 * state, checked against the capability closure by `tools/check-bmad-coverage.mjs`.
 * Audit finding F-21 is the reason it exists — without it "the UI can present a
 * capability as available when it is a shim to a legacy name, or count a
 * normalized package that never reached capability closure."
 *
 * This module is a transcription of five fields of that file, not a second
 * source of truth. `bmadCoverage.test.ts` reads the JSON off disk and fails if
 * the two disagree, so drift is a red test rather than a panel quietly telling
 * a user something the matrix stopped saying.
 *
 * ## What this data does *not* say
 *
 * `snapshotState` is a source path-and-hash relationship. `runtimeState` is what
 * this build's normalization and capability closure expose. **Neither is a
 * compatibility decision.** The governed decision — adopt/adapt/shim/defer/reject
 * with a named owner and semantic tests — is BM-03, and BM-03 has not been
 * recorded: every row carries `decision: null`. So nothing derived from this
 * file may say a skill is "compatible with upstream". It may say what this build
 * projects, and it must say that no decision has been reviewed.
 *
 * A capability with no row here is *not* an unsupported one. The matrix's
 * denominator is upstream BMAD skills; Forge-native capabilities
 * (`bmm:bmad-shard-doc`, `bmm:bmad-index-docs`, the `builder:` operations) are
 * outside it by construction, which is why absence resolves to "unrecorded" and
 * never to a verdict.
 */

/** Where the embedded copy came from. Quoted in the drift test's failure. */
export const COVERAGE_SOURCE = "docs/bmad/skill-coverage.json";

/** The pinned upstream the matrix was taken against. */
export const PINNED_UPSTREAM = "bmad-method 6.10.0";

/** The source path-and-hash relationship to the pinned upstream. */
export type BmadSnapshotState =
  | "identical"
  | "modified"
  | "relocated_or_legacy"
  | "missing";

/** What this build's normalization and capability closure expose. */
export type BmadRuntimeState =
  | "active_projection"
  | "compatibility_shim"
  | "legacy_split_active"
  | "normalized_not_in_closure"
  | "not_projected"
  | "upstream_shim_unprojected";

/**
 * The governed BM-03 vocabulary.
 *
 * Declared although every row is `null` today: the point of the type is that a
 * decision has a closed set of values and its absence is representable. A row
 * that quietly defaulted to "adopt" would assert a compatibility position
 * nobody has taken.
 */
export type BmadCoverageDecision = "adopt" | "adapt" | "shim" | "defer" | "reject";

export interface BmadCoverageRow {
  /** Upstream skill directory name, e.g. `bmad-architecture`. Unique. */
  readonly upstreamSkill: string;
  readonly snapshotState: BmadSnapshotState;
  readonly runtimeState: BmadRuntimeState;
  /** The capability this row routes to, or null when it routes to none. */
  readonly capabilityId: string | null;
  /** Null until BM-03 records the ledger. Null is a state, not an omission. */
  readonly decision: BmadCoverageDecision | null;
}

/** Transcribed from `docs/bmad/skill-coverage.json`; guarded by its test. */
export const BMAD_COVERAGE: readonly BmadCoverageRow[] = [
  { upstreamSkill: "bmad-agent-analyst", snapshotState: "identical", runtimeState: "normalized_not_in_closure", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-agent-tech-writer", snapshotState: "identical", runtimeState: "normalized_not_in_closure", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-document-project", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-document-project", decision: null },
  { upstreamSkill: "bmad-prfaq", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-prfaq", decision: null },
  { upstreamSkill: "bmad-product-brief", snapshotState: "modified", runtimeState: "active_projection", capabilityId: "bmm:bmad-product-brief", decision: null },
  { upstreamSkill: "bmad-agent-pm", snapshotState: "identical", runtimeState: "normalized_not_in_closure", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-agent-ux-designer", snapshotState: "identical", runtimeState: "normalized_not_in_closure", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-prd", snapshotState: "modified", runtimeState: "active_projection", capabilityId: "bmm:bmad-prd", decision: null },
  { upstreamSkill: "bmad-spec", snapshotState: "relocated_or_legacy", runtimeState: "active_projection", capabilityId: "bmm:bmad-spec", decision: null },
  { upstreamSkill: "bmad-ux", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-ux", decision: null },
  { upstreamSkill: "bmad-agent-architect", snapshotState: "identical", runtimeState: "normalized_not_in_closure", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-architecture", snapshotState: "modified", runtimeState: "active_projection", capabilityId: "bmm:bmad-architecture", decision: null },
  { upstreamSkill: "bmad-check-implementation-readiness", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-check-implementation-readiness", decision: null },
  { upstreamSkill: "bmad-create-epics-and-stories", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-create-epics-and-stories", decision: null },
  { upstreamSkill: "bmad-generate-project-context", snapshotState: "identical", runtimeState: "not_projected", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-agent-dev", snapshotState: "identical", runtimeState: "normalized_not_in_closure", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-checkpoint-preview", snapshotState: "identical", runtimeState: "not_projected", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-code-review", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-code-review", decision: null },
  { upstreamSkill: "bmad-correct-course", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-correct-course", decision: null },
  { upstreamSkill: "bmad-create-story", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-create-story", decision: null },
  { upstreamSkill: "bmad-dev-auto", snapshotState: "identical", runtimeState: "not_projected", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-dev-story", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-dev-story", decision: null },
  { upstreamSkill: "bmad-qa-generate-e2e-tests", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-qa-generate-e2e-tests", decision: null },
  { upstreamSkill: "bmad-quick-dev", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-quick-dev", decision: null },
  { upstreamSkill: "bmad-retrospective", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-retrospective", decision: null },
  { upstreamSkill: "bmad-sprint-planning", snapshotState: "identical", runtimeState: "active_projection", capabilityId: "bmm:bmad-sprint-planning", decision: null },
  { upstreamSkill: "bmad-sprint-status", snapshotState: "identical", runtimeState: "not_projected", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-create-architecture", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-architecture", decision: null },
  { upstreamSkill: "bmad-create-prd", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-prd", decision: null },
  { upstreamSkill: "bmad-domain-research", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-domain-research", decision: null },
  { upstreamSkill: "bmad-edit-prd", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-prd", decision: null },
  { upstreamSkill: "bmad-market-research", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-market-research", decision: null },
  { upstreamSkill: "bmad-technical-research", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-technical-research", decision: null },
  { upstreamSkill: "bmad-validate-prd", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-prd", decision: null },
  { upstreamSkill: "bmad-advanced-elicitation", snapshotState: "modified", runtimeState: "active_projection", capabilityId: "bmm:bmad-advanced-elicitation", decision: null },
  { upstreamSkill: "bmad-brainstorming", snapshotState: "modified", runtimeState: "active_projection", capabilityId: "bmm:bmad-brainstorming", decision: null },
  { upstreamSkill: "bmad-customize", snapshotState: "modified", runtimeState: "not_projected", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-deep-recon", snapshotState: "missing", runtimeState: "legacy_split_active", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-forge-idea", snapshotState: "modified", runtimeState: "not_projected", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-help", snapshotState: "modified", runtimeState: "normalized_not_in_closure", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-party-mode", snapshotState: "modified", runtimeState: "not_projected", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-review", snapshotState: "missing", runtimeState: "legacy_split_active", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-editorial-review", snapshotState: "missing", runtimeState: "upstream_shim_unprojected", capabilityId: null, decision: null },
  { upstreamSkill: "bmad-editorial-review-prose", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-editorial-review-prose", decision: null },
  { upstreamSkill: "bmad-editorial-review-structure", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-editorial-review-structure", decision: null },
  { upstreamSkill: "bmad-review-adversarial-general", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-review-adversarial-general", decision: null },
  { upstreamSkill: "bmad-review-edge-case-hunter", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-review-edge-case-hunter", decision: null },
  { upstreamSkill: "bmad-review-verification-gap", snapshotState: "relocated_or_legacy", runtimeState: "compatibility_shim", capabilityId: "bmm:bmad-review-verification-gap", decision: null },
];

const BY_UPSTREAM_SKILL: ReadonlyMap<string, BmadCoverageRow> = new Map(
  BMAD_COVERAGE.map((row) => [row.upstreamSkill, row]),
);

/**
 * The recorded row for an upstream skill name, or null when there is none.
 *
 * Null means *unrecorded*, and callers must keep it that way. Reading a missing
 * row as "not covered, therefore unsupported" would turn every Forge-native
 * capability into a false negative; reading it as "fine" would be the F-21
 * failure this data exists to prevent.
 */
export function coverageForUpstreamSkill(upstreamSkill: string): BmadCoverageRow | null {
  return BY_UPSTREAM_SKILL.get(upstreamSkill) ?? null;
}

/**
 * The rows that route to a capability id, in matrix order.
 *
 * Many-to-one on purpose: `bmm:bmad-prd` is named by its own
 * `active_projection` row and by three v6 shims. A caller wanting one answer
 * for a capability wants `capabilityIsProjected`, not the first row it finds.
 */
export function coverageForCapability(capabilityId: string): readonly BmadCoverageRow[] {
  return BMAD_COVERAGE.filter((row) => row.capabilityId === capabilityId);
}

/**
 * Whether the matrix records this build as projecting the capability.
 *
 * True needs one row that reaches it — `active_projection` (the checker proves
 * the capability is *active* in the closure ledger) or `compatibility_shim` (a
 * legacy name forwards to it). Null, not false, when no row names it: the
 * matrix covers upstream skills, so silence about a capability is silence, and
 * a `false` here would read as a denial the data never made.
 */
export function capabilityIsProjected(capabilityId: string): boolean | null {
  const rows = coverageForCapability(capabilityId);
  if (rows.length === 0) return null;
  return rows.some(
    (row) => row.runtimeState === "active_projection" || row.runtimeState === "compatibility_shim",
  );
}
