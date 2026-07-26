// Honest per-capability fidelity classification for the BMAD Method denominator.
//
// Every runnable Method capability executes through the generic capability lane:
// a single reviewed, one-shot model request that returns one document or change
// set. That is a legitimate Forge adaptation, but it is NOT the upstream BMAD
// workflow, several of which are ordered, scripted, or multi-turn/collaborative.
// The UI must therefore not present a runnable capability as if it were the full
// upstream workflow. This module supplies the label and note used to say so.
//
// Classification is grounded, not asserted-faithful:
//   - `ordered_steps`   — Architecture / CA, the one capability the host runs
//                         as an ordered state machine: nine host-selected
//                         steps with a stable decision log, distilled into one
//                         spine. Still not the upstream coaching dialogue.
//   - `inert_draft`     — Builder capabilities that produce an inert draft and
//                         cannot install, register, or execute anything.
//   - `adapted_one_shot`— every other active Method capability: one reviewed
//                         request, deliberately not asserted as full upstream
//                         fidelity.
//
// The human-readable matrix lives at
// docs/reviews/bmad-capability-fidelity-matrix.md.

export type BmadCapabilityFidelity = "adapted_one_shot" | "ordered_steps" | "inert_draft";

export interface BmadFidelityClassification {
  readonly level: BmadCapabilityFidelity;
  /** Short badge text. */
  readonly label: string;
  /** One-line explanation of how the runtime differs from upstream. */
  readonly note: string;
}

const GENERIC_ADAPTED_NOTE =
  "A single reviewed, one-shot model request — not the full upstream multi-step BMAD workflow.";

const ARCHITECTURE_NOTE =
  "Runs as nine host-validated steps with a stable decision log, distilled into one spine — still not the upstream coaching dialogue.";

/** Capabilities the audit calls out as materially divergent from upstream. */
const NAMED_NOTES: Readonly<Record<string, string>> = {
  "bmm:bmad-architecture": ARCHITECTURE_NOTE,
  "bmm:bmad-market-research":
    "Records external facts as open questions because live retrieval is unavailable — one reviewed request, not the upstream research workflow.",
  "bmm:bmad-dev-story":
    "Reduced to a schema-first one-shot change proposal, not the upstream ordered dev-story workflow.",
  "bmm:bmad-quick-dev":
    "Reduced to a schema-first one-shot change proposal, not the upstream scripted quick-dev workflow.",
};

export function capabilityFidelity(capabilityId: string): BmadFidelityClassification {
  if (capabilityId === "bmm:bmad-architecture") {
    return {
      level: "ordered_steps",
      label: "Stepwise",
      note: ARCHITECTURE_NOTE,
    };
  }
  if (capabilityId.startsWith("builder:")) {
    return {
      level: "inert_draft",
      label: "Inert draft",
      note: "Produces an inert draft only; it cannot install, register, or execute an agent or workflow.",
    };
  }
  return {
    level: "adapted_one_shot",
    label: "Adapted",
    note: NAMED_NOTES[capabilityId] ?? GENERIC_ADAPTED_NOTE,
  };
}
