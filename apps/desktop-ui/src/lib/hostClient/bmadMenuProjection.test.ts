import { describe, expect, it } from "vitest";
import { parseBmadAgentMenu } from "./bmadProtocol";

const reviewedMenu = {
  code: "MR",
  description: "Research the reviewed market.",
  targetKind: "skill_target",
  displayLabel: "Market Research",
  capabilityId: "bmm:bmad-market-research",
  availability: "available",
  availabilityReason: null,
};

describe("BMAD menu capability projection", () => {
  it("carries the host-validated capability identity to the renderer", () => {
    expect(parseBmadAgentMenu(reviewedMenu).capabilityId).toBe(
      "bmm:bmad-market-research",
    );
  });

  it("rejects availability and capability identities that drift apart", () => {
    expect(() =>
      parseBmadAgentMenu({ ...reviewedMenu, capabilityId: null }),
    ).toThrow();
    expect(() =>
      parseBmadAgentMenu({
        ...reviewedMenu,
        availability: "capability_disabled",
        availabilityReason: "bmad_capability_disabled",
      }),
    ).toThrow();
  });

  it("rejects malformed capability identities", () => {
    expect(() =>
      parseBmadAgentMenu({
        ...reviewedMenu,
        capabilityId: "bmm:bmad-market-research:swapped",
      }),
    ).toThrow();
  });
});
