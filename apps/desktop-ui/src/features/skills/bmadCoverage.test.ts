import { describe, expect, it } from "vitest";

import { bmadCapabilities } from "../../generated/bmadCapabilities";
import {
  BMAD_COVERAGE,
  COVERAGE_SOURCE,
  capabilityIsProjected,
  coverageForCapability,
  coverageForUpstreamSkill,
} from "./bmadCoverage";

describe("generated BMAD support projection", () => {
  it("is projected from every generated Method catalog row", () => {
    const methodRows = bmadCapabilities.filter((row) => row.source.id === "method");
    expect(COVERAGE_SOURCE).toBe("packages/bmad-foundation/capability-catalog.json");
    expect(BMAD_COVERAGE).toHaveLength(methodRows.length);
    expect(BMAD_COVERAGE).toHaveLength(48);
    expect(new Set(BMAD_COVERAGE.map((row) => row.upstreamSkill)).size).toBe(48);
  });

  it("makes canonical PRD runtime support and its aliases one generated truth", () => {
    expect(coverageForUpstreamSkill("bmad-prd")).toMatchObject({
      runtimeState: "active_projection",
      capabilityId: "bmm:bmad-prd",
      decision: "adopt",
    });
    expect(coverageForCapability("bmm:bmad-prd").map((row) => row.upstreamSkill)).toEqual([
      "bmad-prd",
      "bmad-create-prd",
      "bmad-edit-prd",
      "bmad-validate-prd",
    ]);
    expect(
      coverageForCapability("bmm:bmad-prd").filter(
        (row) => row.runtimeState === "compatibility_shim",
      ),
    ).toHaveLength(3);
    expect(capabilityIsProjected("bmm:bmad-prd")).toBe(true);
  });

  it("does not invent support for a capability absent from the generated rows", () => {
    expect(coverageForUpstreamSkill("bmad-invented")).toBeNull();
    expect(coverageForCapability("bmm:bmad-invented")).toEqual([]);
    expect(capabilityIsProjected("bmm:bmad-invented")).toBeNull();
  });
});
