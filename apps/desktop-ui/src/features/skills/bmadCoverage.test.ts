import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  BMAD_COVERAGE,
  COVERAGE_SOURCE,
  PINNED_UPSTREAM,
  capabilityIsProjected,
  coverageForCapability,
  coverageForUpstreamSkill,
} from "./bmadCoverage";

/// The authoritative matrix, read off disk. The embedded table is a
/// transcription of it, and a transcription nobody checks is a second source of
/// truth waiting to disagree with the first.
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../../..");

interface RawRow {
  readonly upstreamSkill: string;
  readonly snapshotState: string;
  readonly runtimeState: string;
  readonly capabilityId: string | null;
  readonly decision: string | null;
}

const matrix = JSON.parse(
  readFileSync(path.join(repoRoot, COVERAGE_SOURCE), "utf8"),
) as {
  readonly pinnedUpstream: string;
  readonly totals: { readonly upstreamSkills: number };
  readonly skills: readonly RawRow[];
};

describe("BMAD coverage transcription", () => {
  it("is the same 48 rows as the checked matrix, field for field", () => {
    expect(matrix.pinnedUpstream).toBe(PINNED_UPSTREAM);
    // 48 is BM-02's denominator: the pinned upstream discovery result, not
    // however many rows someone wrote. A coverage figure over the wrong
    // denominator is worse than none.
    expect(matrix.totals.upstreamSkills).toBe(48);
    expect(BMAD_COVERAGE).toHaveLength(matrix.totals.upstreamSkills);
    expect(BMAD_COVERAGE).toEqual(
      matrix.skills.map((row) => ({
        upstreamSkill: row.upstreamSkill,
        snapshotState: row.snapshotState,
        runtimeState: row.runtimeState,
        capabilityId: row.capabilityId,
        decision: row.decision,
      })),
    );
  });

  /// The honesty the whole surface rests on: not one of the 48 rows carries a
  /// governed BM-03 decision, so nothing built on this data may claim upstream
  /// compatibility. If this ever goes green-with-decisions, the surface's
  /// wording has to change with it.
  it("carries no recorded compatibility decision on any row", () => {
    expect(BMAD_COVERAGE.filter((row) => row.decision !== null)).toEqual([]);
  });

  it("names each upstream skill exactly once", () => {
    const names = BMAD_COVERAGE.map((row) => row.upstreamSkill);
    expect(new Set(names).size).toBe(names.length);
  });
});

describe("coverageForUpstreamSkill", () => {
  it("finds a recorded row", () => {
    expect(coverageForUpstreamSkill("bmad-architecture")).toEqual({
      upstreamSkill: "bmad-architecture",
      snapshotState: "modified",
      runtimeState: "active_projection",
      capabilityId: "bmm:bmad-architecture",
      decision: null,
    });
  });

  it("returns null for a skill the matrix never recorded", () => {
    expect(coverageForUpstreamSkill("bmad-shard-doc")).toBeNull();
    expect(coverageForUpstreamSkill("")).toBeNull();
  });
});

describe("coverageForCapability", () => {
  /// `bmm:bmad-prd` is named by its own active row and by three v6 shims. A
  /// lookup that returned the first match would report a shim's semantics for
  /// the canonical skill, or the reverse, depending on row order.
  it("returns every row routing to one capability", () => {
    const rows = coverageForCapability("bmm:bmad-prd");
    expect(rows.map((row) => row.upstreamSkill)).toEqual([
      "bmad-prd",
      "bmad-create-prd",
      "bmad-edit-prd",
      "bmad-validate-prd",
    ]);
    expect(rows.filter((row) => row.runtimeState === "compatibility_shim")).toHaveLength(3);
  });

  it("returns nothing for a capability no row names", () => {
    expect(coverageForCapability("bmm:bmad-shard-doc")).toEqual([]);
  });
});

describe("capabilityIsProjected", () => {
  it("is true when a row reaches the capability", () => {
    expect(capabilityIsProjected("bmm:bmad-architecture")).toBe(true);
    // Reached only through shim rows, which still reach it.
    expect(capabilityIsProjected("bmm:bmad-editorial-review-prose")).toBe(true);
  });

  /// Null rather than false. The matrix counts upstream BMAD skills; this build
  /// also ships capabilities that were never upstream ones, and reporting those
  /// as "not projected" would be a denial the data never made.
  it("is null, not false, when no row names the capability", () => {
    expect(capabilityIsProjected("bmm:bmad-shard-doc")).toBeNull();
    expect(capabilityIsProjected("builder:agent.analyze")).toBeNull();
  });
});
