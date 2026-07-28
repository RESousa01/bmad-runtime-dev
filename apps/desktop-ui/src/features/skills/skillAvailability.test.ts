import { describe, expect, it } from "vitest";

import { BMAD_COVERAGE } from "./bmadCoverage";
import {
  type SkillVerdictInput,
  availabilityLabel,
  blockerExplanation,
  capabilityCompatibility,
  compatibilityDecisionNote,
  selectSkill,
  skillVerdict,
} from "./skillAvailability";

function input(overrides: Partial<SkillVerdictInput> = {}): SkillVerdictInput {
  return {
    upstreamSkill: "bmad-architecture",
    capabilityId: "bmm:bmad-architecture",
    projected: { availability: "available", blockerCodes: [] },
    ...overrides,
  };
}

describe("skillVerdict — what the host projects", () => {
  it("is available when the projection says so and a capability is bound", () => {
    const verdict = skillVerdict(input());
    expect(verdict.availability).toEqual({
      kind: "available",
      capabilityId: "bmm:bmad-architecture",
    });
  });

  it("is unavailable with the projection's reason when the host says no", () => {
    const verdict = skillVerdict(
      input({
        projected: {
          availability: "capability_disabled",
          blockerCodes: ["bmad_capability_disabled"],
        },
      }),
    );

    expect(verdict.availability.kind).toBe("unavailable");
    expect(verdict.availability).toHaveProperty(
      "reason",
      expect.stringContaining("not enabled in the sealed foundation"),
    );
  });

  /// Every non-available projection value gets its own words. A generic
  /// fallback would let a new blocker reach the user as a sentence that
  /// explains nothing, which is the state this whole surface exists to end.
  it("gives every unavailable projection value a distinct, non-empty reason", () => {
    const values = [
      "capability_disabled",
      "dependency_unavailable",
      "orphan_skill",
      "network_unavailable",
      "source_prompt_unavailable",
    ] as const;
    const reasons = values.map((availability) => {
      const verdict = skillVerdict(input({ projected: { availability, blockerCodes: [] } }));
      expect(verdict.availability.kind).toBe("unavailable");
      return verdict.availability.kind === "unavailable" ? verdict.availability.reason : "";
    });

    for (const reason of reasons) expect(reason.trim().length).toBeGreaterThan(20);
    expect(new Set(reasons).size).toBe(values.length);
  });

  /**
   * The old panel filtered these rows out on `capabilityId !== null`, turning a
   * projection defect into a skill that simply was not in the list.
   *
   * The verdict is `unknown`, not `unavailable`, and the difference is the
   * point. `unavailable` is a claim that the skill cannot run; here the catalog
   * says it can and nothing names what would run, so the two sources contradict
   * each other and neither is a basis for either answer. `selectSkill` refuses
   * both kinds identically, so the user is protected the same way -- but only
   * one of them is honest about why.
   */
  it("uses the coverage row's capability when the projection names none", () => {
    // Installed skills and source actions carry a module and a skill name and
    // no capability, so the matrix row is the only thing that knows what would
    // actually run. Falling back to it is why the case below needs *both*
    // sources empty to reach the refusal.
    expect(skillVerdict(input({ capabilityId: null })).availability).toEqual({
      kind: "available",
      capabilityId: "bmm:bmad-architecture",
    });
  });

  it("refuses an entry the catalog calls available while naming no capability", () => {
    // Neither side names one: no capability on the projection, and an upstream
    // skill the coverage matrix has no row for.
    const verdict = skillVerdict(input({
      capabilityId: null,
      upstreamSkill: "bmad-not-in-the-matrix",
    }));

    expect(verdict.availability.kind).toBe("unknown");
    expect(verdict.availability).toHaveProperty(
      "reason",
      expect.stringContaining("nothing here that could be started"),
    );
    // Refused either way: the honesty is in the reason, not in the guard.
    expect(selectSkill(verdict, "Any label").kind).toBe("refused");
  });
});

describe("skillVerdict — what the coverage matrix adds", () => {
  /// The mutation this file exists for. A skill the matrix has never heard of
  /// must not come back as a settled verdict just because nothing said no.
  it("does not know about a skill with no coverage entry and no projection", () => {
    const verdict = skillVerdict({
      upstreamSkill: "bmad-invented-by-nobody",
      capabilityId: null,
      projected: null,
    });

    expect(verdict.availability.kind).toBe("unknown");
    expect(verdict.availability).toHaveProperty(
      "reason",
      expect.stringContaining("bmad-invented-by-nobody"),
    );
    expect(verdict.coverage.kind).toBe("unrecorded");
  });

  it("does not know when the entry has no upstream identity at all", () => {
    const verdict = skillVerdict({
      upstreamSkill: null,
      capabilityId: "bmm:bmad-architecture",
      projected: null,
    });

    expect(verdict.availability.kind).toBe("unknown");
    expect(verdict.availability).toHaveProperty(
      "reason",
      expect.stringContaining("no upstream skill identity"),
    );
  });

  /// The matrix alone is enough to say no: it is checked against the capability
  /// closure, so "this build projects nothing for it" is a fact, not a guess.
  it("is unavailable for an upstream skill this build does not project", () => {
    const verdict = skillVerdict({
      upstreamSkill: "bmad-sprint-status",
      capabilityId: null,
      projected: null,
    });

    expect(verdict.availability).toEqual({
      kind: "unavailable",
      reason: "Present in the pinned upstream and absent from this build.",
    });
  });

  it("is available from the matrix alone when a row reaches a capability", () => {
    expect(
      skillVerdict({ upstreamSkill: "bmad-ux", capabilityId: null, projected: null }).availability,
    ).toEqual({ kind: "available", capabilityId: "bmm:bmad-ux" });
    // A shim row reaches a capability too, under upstream's legacy name.
    expect(
      skillVerdict({ upstreamSkill: "bmad-create-prd", capabilityId: null, projected: null })
        .availability,
    ).toEqual({ kind: "available", capabilityId: "bmm:bmad-prd" });
  });

  /// Two recorded sources disagreeing is not a tie to be broken by whichever is
  /// checked first. Neither is then a basis for telling a user it will run.
  it("withdraws to unknown when the catalog and the matrix contradict each other", () => {
    const verdict = skillVerdict({
      upstreamSkill: "bmad-sprint-status",
      capabilityId: "bmm:bmad-sprint-status",
      projected: { availability: "available", blockerCodes: [] },
    });

    expect(verdict.availability.kind).toBe("unknown");
    expect(verdict.availability).toHaveProperty(
      "reason",
      expect.stringContaining("not_projected"),
    );
  });

  /// A capability that was never an upstream BMAD skill is not made unavailable
  /// by the matrix's silence. `bmm:bmad-shard-doc` is a real, active core skill
  /// with no row in a matrix whose denominator is upstream directories.
  it("leaves a Forge-native capability runnable and says the matrix is silent", () => {
    const verdict = skillVerdict({
      upstreamSkill: "bmad-shard-doc",
      capabilityId: "bmm:bmad-shard-doc",
      projected: { availability: "available", blockerCodes: [] },
    });

    expect(verdict.availability).toEqual({
      kind: "available",
      capabilityId: "bmm:bmad-shard-doc",
    });
    expect(verdict.coverage.kind).toBe("unrecorded");
    expect(verdict.coverage.summary).toContain("no row for this skill");
  });

  it("carries the matrix's runtime and source state on an available verdict", () => {
    const verdict = skillVerdict(input());

    expect(verdict.coverage).toEqual({
      kind: "recorded",
      runtimeState: "active_projection",
      snapshotState: "modified",
      decision: null,
      summary: expect.stringContaining("active in the closure ledger"),
      sourceNote: "This build's copy of the source differs from the pinned upstream.",
    });
  });

  /// Every runtime and snapshot state must have words. A state with none would
  /// reach the panel as an empty sentence beside a skill the user is deciding
  /// whether to run.
  it("gives every state recorded in the matrix a non-empty sentence", () => {
    for (const row of BMAD_COVERAGE) {
      const coverage = skillVerdict({
        upstreamSkill: row.upstreamSkill,
        capabilityId: row.capabilityId,
        projected: null,
      }).coverage;
      expect(coverage.kind).toBe("recorded");
      expect(coverage.summary.trim().length).toBeGreaterThan(20);
      if (coverage.kind === "recorded") {
        expect(coverage.sourceNote.trim().length).toBeGreaterThan(20);
      }
    }
  });
});

describe("compatibilityDecisionNote", () => {
  /// BM-03 has recorded nothing, so a surface showing only "Runs here" would be
  /// read as a compatibility claim nobody has made.
  it("says plainly that nothing was reviewed when no decision is recorded", () => {
    expect(compatibilityDecisionNote(null)).toContain("No compatibility decision");
    expect(compatibilityDecisionNote(null)).toContain("nothing here claims it behaves as upstream");
  });

  it("names a decision once one is recorded", () => {
    expect(compatibilityDecisionNote("adapt")).toBe("Recorded compatibility decision: adapt.");
  });
});

/// The surfaces that hold a capability id and no upstream name -- an agent
/// menu, a run in flight -- ask the matrix this way instead. Untested until
/// FE-25 wired them, and the first thing rendering it showed was a sentence
/// whose verb did not agree with its subject.
describe("capabilityCompatibility", () => {
  it("names the canonical upstream skill and the legacy names that forward to it", () => {
    const compatibility = capabilityCompatibility("bmm:bmad-architecture");

    expect(compatibility.summary).toBe(
      "Projected from the upstream skill bmad-architecture; the legacy name bmad-create-architecture forwards to it.",
    );
    expect(compatibility.upstreamSkills).toEqual([
      "bmad-architecture",
      "bmad-create-architecture",
    ]);
    expect(compatibility.decisionRecorded).toBe(false);
  });

  it("keeps the verb agreeing with the subject when several names forward", () => {
    expect(capabilityCompatibility("bmm:bmad-prd").summary).toBe(
      "Projected from the upstream skill bmad-prd; the legacy names bmad-create-prd, bmad-edit-prd, bmad-validate-prd forward to it.",
    );
  });

  /// Reported as a different identity rather than as upstream's own: nothing
  /// upstream carries this name, so calling it "projected from" it would be a
  /// parity claim the matrix does not make.
  it("says so when a capability is reached only through a legacy name", () => {
    expect(capabilityCompatibility("bmm:bmad-domain-research").summary).toContain(
      "Reached only through the legacy upstream name bmad-domain-research",
    );
  });

  /// Silence, not a denial. The matrix counts upstream directories, and this
  /// build also ships capabilities that were never upstream skills.
  it("reports silence for a capability the matrix has no row for", () => {
    const compatibility = capabilityCompatibility("builder:agent.analyze");

    expect(compatibility.upstreamSkills).toEqual([]);
    expect(compatibility.summary).toContain("no row for builder:agent.analyze");
    expect(compatibility.decisionRecorded).toBe(false);
  });
});

describe("blockerExplanation", () => {
  it("explains a known blocker code", () => {
    expect(blockerExplanation("bmad_help_catalog_orphan")).toContain(
      "no matching installed skill",
    );
  });

  /// Regression guard: an unrecognised blocker rendered as a bare identifier and
  /// nothing else.
  it("falls back to prose rather than a bare identifier", () => {
    expect(blockerExplanation("bmad_future_code")).toContain("gives no further reason");
  });
});

describe("availabilityLabel", () => {
  it("distinguishes cannot-run from not-known", () => {
    expect(availabilityLabel({ kind: "available", capabilityId: "x" })).toBe("Runs here");
    expect(availabilityLabel({ kind: "unavailable", reason: "r" })).toBe("Cannot run here");
    expect(availabilityLabel({ kind: "unknown", reason: "r" })).toBe("Not known");
  });
});

describe("selectSkill", () => {
  it("starts an available skill and carries the capability to run", () => {
    expect(selectSkill(skillVerdict(input()), "Architecture")).toEqual({
      kind: "start",
      capabilityId: "bmm:bmad-architecture",
      label: "Architecture",
    });
  });

  /// The point of selection, not the point of use. The old surface let the user
  /// pick something and discover at send time that it could not run.
  it("refuses an unavailable skill with the verdict's own reason", () => {
    const verdict = skillVerdict(
      input({
        projected: {
          availability: "source_prompt_unavailable",
          blockerCodes: ["bmad_source_prompt_unavailable"],
        },
      }),
    );
    const selection = selectSkill(verdict, "Architecture");

    expect(selection.kind).toBe("refused");
    expect(selection).toHaveProperty(
      "reason",
      expect.stringContaining("original source prompt has not been adopted"),
    );
  });

  it("refuses an unknown skill, saying it is unknown rather than broken", () => {
    const verdict = skillVerdict({
      upstreamSkill: "bmad-never-heard-of-it",
      capabilityId: null,
      projected: null,
    });
    const selection = selectSkill(verdict, "Mystery");

    expect(selection.kind).toBe("refused");
    expect(selection).toHaveProperty(
      "reason",
      expect.stringContaining("has no row for it"),
    );
  });

  /// Nothing may be refused without saying why, and "why" may not be blank.
  /// The type makes `reason` required; this is the other half — that it is
  /// carried through from the verdict rather than replaced by a placeholder.
  it("never refuses without a reason, for any state in the matrix", () => {
    for (const row of BMAD_COVERAGE) {
      const selection = selectSkill(
        skillVerdict({
          upstreamSkill: row.upstreamSkill,
          capabilityId: row.capabilityId,
          projected: null,
        }),
        row.upstreamSkill,
      );
      if (selection.kind === "refused") {
        expect(selection.reason.trim().length).toBeGreaterThan(20);
      }
    }
  });
});
