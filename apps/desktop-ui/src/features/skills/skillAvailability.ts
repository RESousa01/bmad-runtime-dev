/**
 * Whether one BMAD skill can be started in this build, and what may be said
 * about its relationship to the pinned upstream.
 *
 * FE-25. The skills catalogue used to decide this inline, per row, from
 * `availability === "available" && capabilityId !== null`, and every surface
 * that listed a skill re-derived it. Two things went wrong with that.
 *
 * The first is silent omission. A skill that cannot run was filtered out of the
 * list it belonged in, so the catalogue read as complete while a third of it was
 * missing. A user cannot ask about something they were never shown, and the
 * failure only arrives at the moment they needed the thing to work.
 *
 * The second is the one this repo keeps re-committing: a surface stating
 * something it has no basis for. Appearing in a list is not evidence that a
 * skill runs, and `docs/bmad/skill-coverage.json` recording a row for it is not
 * evidence that anyone reviewed its compatibility — BM-03 has recorded no
 * decision, so *every* row is unreviewed. The three-way verdict below exists so
 * "we do not know" is a value this code can return rather than a case it has to
 * round to one of the other two.
 *
 * ## The precedence, which is the actual content
 *
 * Two independent sources speak about a skill, and they answer different
 * questions:
 *
 *   - **the host's projection** — can this be started *here, now, in this
 *     workspace*. Live, specific, and the only one with runtime authority.
 *   - **the coverage matrix** — what this *build* projects from pinned upstream.
 *     Static, checked against the capability closure, and about parity rather
 *     than about this workspace.
 *
 * The projection wins, because it is the more specific and more current claim.
 * The matrix qualifies it and may never promote it: a capability the matrix has
 * no row for is a capability the matrix was never about — `bmm:bmad-shard-doc`
 * and the `builder:` operations are Forge-native, not upstream skills — and
 * treating that silence as a denial would invent a verdict just as surely as
 * treating it as approval.
 *
 * The one thing the matrix may do to a live "available" is withdraw it. When
 * the host says a skill runs and the matrix says this build projects no
 * capability for it, one of the two is stale, and neither is then a basis for
 * telling a user it will work.
 */

import type { BmadAvailability, BmadBlockerCode } from "../../lib/bmadProjection";
import {
  type BmadCoverageDecision,
  type BmadCoverageRow,
  type BmadRuntimeState,
  type BmadSnapshotState,
  coverageForCapability,
  coverageForUpstreamSkill,
} from "./bmadCoverage";

/**
 * Whether a skill can be started, and if not, why not.
 *
 * `reason` is required on both non-available variants, so the compiler refuses
 * a verdict that withholds a control without saying what is missing. This is
 * the same shape, for the same reason, as `CommandAvailability` in
 * `app/commands/commandRegistry.ts`.
 *
 * `unknown` is not a softer `unavailable`. It is the state where nothing on
 * hand settles the question, and it is separate precisely so that guessing
 * requires writing a different variant rather than falling through to one.
 */
export type SkillAvailability =
  | {
    readonly kind: "available";
    /** What a start would run. Only reachable from an available verdict. */
    readonly capabilityId: string;
  }
  | {
    readonly kind: "unavailable";
    /**
     * Shown to the user verbatim. A sentence naming what is missing, never a
     * blocker identifier and never "not allowed".
     */
    readonly reason: string;
  }
  | {
    readonly kind: "unknown";
    /**
     * Shown to the user verbatim. Says what is not known and what would settle
     * it, so "unknown" reads as a described gap rather than a shrug.
     */
    readonly reason: string;
  };

/**
 * What the recorded coverage matrix says, and what it pointedly does not.
 *
 * Carried alongside every verdict, including available ones. A runnable skill
 * whose upstream copy was modified and whose compatibility nobody reviewed is
 * still runnable — and the user is entitled to know both halves.
 */
export type SkillCoverageNote =
  | {
    readonly kind: "recorded";
    readonly runtimeState: BmadRuntimeState;
    readonly snapshotState: BmadSnapshotState;
    /** Null until BM-03 records the ledger. Null means unreviewed, not fine. */
    readonly decision: BmadCoverageDecision | null;
    /** One sentence on what this build does with the skill. */
    readonly summary: string;
    /** One sentence on how this build's copy relates to pinned upstream. */
    readonly sourceNote: string;
  }
  | {
    readonly kind: "unrecorded";
    readonly summary: string;
  };

export interface SkillVerdict {
  readonly availability: SkillAvailability;
  readonly coverage: SkillCoverageNote;
}

/** What the host's projection says about one catalogue entry. */
export interface ProjectedAvailability {
  readonly availability: BmadAvailability;
  readonly blockerCodes: readonly BmadBlockerCode[];
}

export interface SkillVerdictInput {
  /**
   * Upstream skill directory name — `bmad-architecture`, `bmad-agent-pm`. The
   * matrix's key. Null when the surface has no upstream identity to offer,
   * which is itself a reason the matrix cannot be consulted.
   */
  readonly upstreamSkill: string | null;
  /** The capability the entry binds, or null when it binds none. */
  readonly capabilityId: string | null;
  /**
   * The host's own statement, or null when the surface has none. Null is not
   * "fine" — it is the input that produces `unknown`.
   */
  readonly projected: ProjectedAvailability | null;
}

/**
 * Why a projection reports something as not available.
 *
 * Total over `BmadAvailability` rather than a lookup with a fallback: a new
 * availability value must come here and be given words, instead of silently
 * inheriting a generic sentence that explains nothing.
 */
function projectedReason(availability: Exclude<BmadAvailability, "available">): string {
  switch (availability) {
    case "capability_disabled":
      return "This capability is not enabled in the sealed foundation, so it cannot be started here. Enabling it is a reviewed organization decision, not an error you caused.";
    case "dependency_unavailable":
      return "A skill this entry depends on is not part of the adopted foundation yet, so it stays disabled until that skill is adopted.";
    case "orphan_skill":
      return "This catalog row has no matching installed skill, so there is nothing for it to start.";
    case "network_unavailable":
      return "The source references network content that is not adopted into the offline foundation, so it cannot run here.";
    case "source_prompt_unavailable":
      return "The original source prompt has not been adopted into the sealed foundation, so nothing can be prepared from it.";
  }
}

/** Short chip text for a verdict. Sentence case; it is a label, not a sentence. */
export function availabilityLabel(availability: SkillAvailability): string {
  switch (availability.kind) {
    case "available":
      return "Runs here";
    case "unavailable":
      return "Cannot run here";
    case "unknown":
      return "Not known";
  }
}

/**
 * What to say about a blocker code this build has no wording for.
 *
 * The previous shape rendered the bare identifier and nothing else, which is
 * the least actionable thing a panel can show. The code is kept as evidence
 * underneath; it is not the answer.
 */
export const UNKNOWN_BLOCKER_EXPLANATION =
  "The sealed foundation reports this entry as unavailable and gives no further reason, so nothing here can be started. The identifier below is what to quote if you report it.";

const BLOCKER_EXPLANATIONS: Readonly<Record<BmadBlockerCode, string>> = {
  bmad_capability_disabled:
    "This capability is not enabled in the sealed foundation. Enabling it is a reviewed organization decision, not an error.",
  bmad_dependency_unavailable:
    "A skill this entry depends on is not part of the adopted foundation yet, so it stays disabled until adoption.",
  bmad_help_catalog_orphan:
    "This catalog row has no matching installed skill, so it cannot be started.",
  bmad_network_reference_unavailable:
    "The source references network content that is not adopted into the offline foundation.",
  bmad_source_prompt_unavailable:
    "The original source prompt has not been adopted into the sealed foundation, so nothing can be prepared from it.",
};

/** Prose for one blocker code. Falls back rather than rendering a bare code. */
export function blockerExplanation(code: string): string {
  return BLOCKER_EXPLANATIONS[code as BmadBlockerCode] ?? UNKNOWN_BLOCKER_EXPLANATION;
}

/**
 * The capability a matrix row records this build as reaching, or null.
 *
 * Returns the id rather than a boolean so the caller that needs it does not
 * have to assert past a check it already made.
 */
function rowCapability(row: BmadCoverageRow): string | null {
  return row.runtimeState === "active_projection" || row.runtimeState === "compatibility_shim"
    ? row.capabilityId
    : null;
}

function runtimeStateSummary(runtimeState: BmadRuntimeState): string {
  switch (runtimeState) {
    case "active_projection":
      return "This build projects it from the pinned upstream, and its capability is active in the closure ledger.";
    case "compatibility_shim":
      return "A legacy upstream name that forwards to another capability here, so it runs under a different identity than upstream's.";
    case "legacy_split_active":
      return "Upstream's single skill is split across two active paths in this build, so no one capability corresponds to it.";
    case "normalized_not_in_closure":
      return "Normalized into a package but never reached capability closure, so this build exposes nothing that can start it.";
    case "not_projected":
      return "Present in the pinned upstream and absent from this build.";
    case "upstream_shim_unprojected":
      return "Upstream ships this only as a forwarder, and this build does not project it.";
  }
}

function snapshotStateNote(snapshotState: BmadSnapshotState): string {
  switch (snapshotState) {
    case "identical":
      return "This build's copy of the source matches the pinned upstream.";
    case "modified":
      return "This build's copy of the source differs from the pinned upstream.";
    case "relocated_or_legacy":
      return "Upstream keeps this at a relocated or legacy path, so the source identity is not a direct match.";
    case "missing":
      return "The pinned upstream source is not present in this build's snapshot.";
  }
}

const UNRECORDED_SUMMARY =
  "The upstream coverage matrix has no row for this skill, so nothing here records how it compares with the pinned upstream. Absence from the matrix is not a finding either way — the matrix counts upstream BMAD skills, and this build also ships capabilities that were never upstream ones.";

/**
 * The one sentence every verdict carries about compatibility review.
 *
 * Exported because it is the claim the surface is not allowed to skip. A
 * runnable skill still has no reviewed compatibility position, and a panel that
 * showed only "Runs here" would be read as one.
 */
export function compatibilityDecisionNote(decision: BmadCoverageDecision | null): string {
  return decision === null
    ? "No compatibility decision has been recorded for this skill, so nothing here claims it behaves as upstream does."
    : `Recorded compatibility decision: ${decision}.`;
}

function coverageNote(row: BmadCoverageRow | null): SkillCoverageNote {
  if (row === null) return { kind: "unrecorded", summary: UNRECORDED_SUMMARY };
  return {
    kind: "recorded",
    runtimeState: row.runtimeState,
    snapshotState: row.snapshotState,
    decision: row.decision,
    summary: runtimeStateSummary(row.runtimeState),
    sourceNote: snapshotStateNote(row.snapshotState),
  };
}

/**
 * The per-skill verdict. Pure: same input, same answer, on every surface.
 *
 * See the module comment for why the projection outranks the matrix and why the
 * matrix can only ever withdraw an availability, never grant one.
 */
export function skillVerdict(input: SkillVerdictInput): SkillVerdict {
  const row = input.upstreamSkill === null
    ? null
    : coverageForUpstreamSkill(input.upstreamSkill);
  const coverage = coverageNote(row);

  if (input.projected !== null) {
    if (input.projected.availability !== "available") {
      return {
        availability: {
          kind: "unavailable",
          reason: projectedReason(input.projected.availability),
        },
        coverage,
      };
    }
    if (row !== null && rowCapability(row) === null) {
      return {
        availability: {
          kind: "unknown",
          reason: `The catalog reports this as available, but the recorded coverage matrix says this build projects no capability for it (${row.runtimeState}). One of the two is out of date, and neither is a basis for saying it will run.`,
        },
        coverage,
      };
    }
    // Several projections state availability without naming a capability --
    // installed skills and source actions carry a module and a skill name only.
    // The matrix's row is then the one thing that knows what would run.
    const bound = input.capabilityId ?? (row === null ? null : rowCapability(row));
    if (bound === null) {
      // Reported runnable while nothing on either side names what would run.
      // The old panel dropped rows like this by filtering on
      // `capabilityId !== null`, which turned a projection defect into a skill
      // that simply was not in the list.
      return {
        availability: {
          kind: "unknown",
          reason: "The catalog reports this as available, but neither the catalog nor the recorded coverage matrix names a capability for it, so there is nothing here that could be started.",
        },
        coverage,
      };
    }
    return { availability: { kind: "available", capabilityId: bound }, coverage };
  }

  if (row === null) {
    return {
      availability: {
        kind: "unknown",
        reason: input.upstreamSkill === null
          ? "This entry carries no upstream skill identity and the catalog states no availability for it, so nothing here can say whether it runs."
          : `Nothing here states whether "${input.upstreamSkill}" runs: the catalog reports no availability for it, and the recorded coverage matrix has no row for it.`,
      },
      coverage,
    };
  }
  const reached = rowCapability(row);
  if (reached === null) {
    return {
      availability: { kind: "unavailable", reason: runtimeStateSummary(row.runtimeState) },
      coverage,
    };
  }
  return { availability: { kind: "available", capabilityId: reached }, coverage };
}

/**
 * The upstream skill a method-graph step key belongs to.
 *
 * Upstream's `module-help.csv` keys a single-action skill by its own name and a
 * multi-action one as `skill:action`, so `bmad-create-story:create` and
 * `bmad-create-story:validate` are two steps of one skill. The coverage matrix
 * is keyed by skill, so the action has to come off before it is consulted —
 * otherwise every multi-action step looks like a skill nobody recorded.
 */
export function upstreamSkillFromStepKey(stepKey: string): string {
  const separator = stepKey.indexOf(":");
  return separator === -1 ? stepKey : stepKey.slice(0, separator);
}

/**
 * What the coverage matrix records about one capability, for a surface that
 * only has the capability id — a run in flight, not a catalogue row.
 *
 * Reported as the set of upstream skills that reach it, because that is what
 * the matrix actually holds: `bmm:bmad-prd` is reached by upstream's own
 * `bmad-prd` and by three v6 shim names, and a surface that named only one of
 * them would be describing a different skill than the one it ran.
 */
export interface CapabilityCompatibility {
  /** Upstream skills the matrix records as reaching this capability. */
  readonly upstreamSkills: readonly string[];
  /** One sentence, shown verbatim. */
  readonly summary: string;
  /** True once BM-03 records a decision for any of the rows involved. */
  readonly decisionRecorded: boolean;
}

export function capabilityCompatibility(capabilityId: string): CapabilityCompatibility {
  const rows = coverageForCapability(capabilityId);
  if (rows.length === 0) {
    return {
      upstreamSkills: [],
      summary: `The recorded upstream coverage matrix has no row for ${capabilityId}, so nothing here says how it compares with the pinned upstream method.`,
      decisionRecorded: false,
    };
  }
  const canonical = rows.find((row) => row.runtimeState === "active_projection") ?? null;
  const aliases = rows
    .filter((row) => row !== canonical)
    .map((row) => row.upstreamSkill);
  // The alias clause is written twice rather than interpolated once, because a
  // singular subject needs a singular verb: "the legacy name X forwards to it",
  // not "forward to it". Sentences shown verbatim to a user have to read as
  // sentences, or the surface looks assembled rather than written.
  const aliasClause = aliases.length === 1
    ? `the legacy name ${aliases[0]} forwards to it`
    : `the legacy names ${aliases.join(", ")} forward to it`;
  const summary = canonical === null
    ? `Reached only through the legacy upstream ${aliases.length === 1 ? "name" : "names"} ${aliases.join(", ")}, so it runs under a different identity than upstream's own.`
    : aliases.length === 0
      ? `Projected from the upstream skill ${canonical.upstreamSkill}.`
      : `Projected from the upstream skill ${canonical.upstreamSkill}; ${aliasClause}.`;
  return {
    upstreamSkills: rows.map((row) => row.upstreamSkill),
    summary,
    decisionRecorded: rows.some((row) => row.decision !== null),
  };
}

/**
 * What happens when the user picks a skill.
 *
 * `start` is the only carrier of a capability id a caller can act on, so a
 * surface cannot issue a run for something it should have refused — the id is
 * simply not in scope on the other branch. That is the point: this repo prefers
 * an unrepresentable illegal state over a second runtime check that a later
 * edit can drift away from.
 */
export type SkillSelection =
  | { readonly kind: "start"; readonly capabilityId: string; readonly label: string }
  | { readonly kind: "refused"; readonly reason: string };

/**
 * Refuses at the point of selection rather than at the point of use.
 *
 * `unknown` is refused alongside `unavailable`, and the wording differs. A
 * start spends the user's reviewed context on one irreversible send; doing that
 * on a verdict nothing supports is how "it just failed and nothing warned me"
 * happens. The refusal for an unknown says it is unknown, not that it is
 * broken.
 */
export function selectSkill(verdict: SkillVerdict, label: string): SkillSelection {
  switch (verdict.availability.kind) {
    case "available":
      return { kind: "start", capabilityId: verdict.availability.capabilityId, label };
    case "unavailable":
    case "unknown":
      return { kind: "refused", reason: verdict.availability.reason };
  }
}
