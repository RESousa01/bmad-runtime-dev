import { describe, expect, it } from "vitest";

import { matchMenuIntent } from "./menuIntent";
import type { BmadAgentMenuProjection } from "./bmadProjection";

function menu(
  code: string,
  displayLabel: string,
  overrides: Partial<BmadAgentMenuProjection> = {},
): BmadAgentMenuProjection {
  return {
    code,
    description: displayLabel,
    targetKind: "skill_target",
    displayLabel,
    capabilityId: `bmm:${displayLabel.toLowerCase().replace(/\s+/gu, "-")}`,
    availability: "available",
    availabilityReason: null,
    ...overrides,
  };
}

const architect = [menu("CA", "Create Architecture"), menu("IR", "Check Implementation Readiness")];

describe("matchMenuIntent", () => {
  it("dispatches on an explicit menu code", () => {
    const match = matchMenuIntent("run CA on this repo", architect);
    expect(match?.menu.code).toBe("CA");
    expect(match?.reason).toBe("menu_code");
  });

  it("dispatches when every significant label word is named", () => {
    const match = matchMenuIntent("let's create the architecture for this", architect);
    expect(match?.menu.code).toBe("CA");
    expect(match?.reason).toBe("label");
  });

  it("continues the conversation when nothing fits", () => {
    // The source is explicit: an unmatched message is just chat.
    expect(matchMenuIntent("what does this workspace do?", architect)).toBeNull();
    expect(matchMenuIntent("", architect)).toBeNull();
  });

  it("continues the conversation when the intent is ambiguous", () => {
    const ambiguous = [menu("CA", "Create Architecture"), menu("XA", "Create Architecture")];
    expect(matchMenuIntent("create the architecture", ambiguous)).toBeNull();
    // Two codes named at once is equally ambiguous.
    expect(matchMenuIntent("do CA and IR", architect)).toBeNull();
  });

  it("matches a code only as a whole word", () => {
    // "CA" must not match inside an unrelated word.
    expect(matchMenuIntent("the cascade layout broke", architect)).toBeNull();
  });

  it("ignores menus the host did not make runnable", () => {
    const unavailable = [
      menu("CA", "Create Architecture", { capabilityId: null }),
      menu("IR", "Check Implementation Readiness", { availability: "capability_disabled" }),
    ];
    expect(matchMenuIntent("run CA", unavailable)).toBeNull();
    expect(matchMenuIntent("check implementation readiness", unavailable)).toBeNull();
  });

  it("does not match on filler words alone", () => {
    // "let's create a new thing" shares only stop-words with the label.
    expect(matchMenuIntent("let's create a new thing", architect)).toBeNull();
  });
});
