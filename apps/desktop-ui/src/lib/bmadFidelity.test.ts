import { describe, expect, it } from "vitest";

import { capabilityFidelity } from "./bmadFidelity";

describe("capabilityFidelity", () => {
  it("marks Architecture as the ordered stepwise capability", () => {
    const fidelity = capabilityFidelity("bmm:bmad-architecture");
    expect(fidelity.level).toBe("ordered_steps");
    expect(fidelity.label).toBe("Stepwise");
    // The count must match `BmadArchitectureStep::ORDERED`, which gained a
    // ninth step (`Deferred`). This assertion previously pinned "eight", so it
    // held the badge's wrong number in place instead of catching the drift.
    expect(fidelity.note).toContain("nine host-validated steps");
  });

  it("still refuses to claim upstream fidelity for the stepwise capability", () => {
    // The ordered lane is a real state machine, not the upstream coaching
    // dialogue. Overclaiming here is the failure this module exists to prevent.
    expect(capabilityFidelity("bmm:bmad-architecture").note).toContain("not the upstream");
  });

  it("marks Builder capabilities as inert drafts", () => {
    const fidelity = capabilityFidelity("builder:agent.edit");
    expect(fidelity.level).toBe("inert_draft");
    expect(fidelity.label).toBe("Inert draft");
  });

  it("marks a runnable Method capability as an adapted one-shot", () => {
    const fidelity = capabilityFidelity("bmm:bmad-product-brief");
    expect(fidelity.level).toBe("adapted_one_shot");
    expect(fidelity.label).toBe("Adapted");
    expect(fidelity.note).toContain("one-shot");
  });

  it("uses audit-specific notes for materially divergent capabilities", () => {
    expect(capabilityFidelity("bmm:bmad-dev-story").note).toContain("schema-first one-shot");
    expect(capabilityFidelity("bmm:bmad-quick-dev").note).toContain("schema-first one-shot");
    expect(capabilityFidelity("bmm:bmad-market-research").note).toContain("open questions");
  });
});
