import { describe, expect, it } from "vitest";

import {
  beginDecision,
  canDecide,
  decisionInFlight,
  decisionLabel,
  settleDecision,
  undecided,
  type ApprovalChoice,
} from "./approvalDecision";

const choices: readonly ApprovalChoice[] = ["apply", "revise", "discard"];

describe("deciding once", () => {
  it("allows a decision from an undecided proposal", () => {
    expect(canDecide(undecided)).toBe(true);
    expect(beginDecision(undecided, "apply")).toEqual({ kind: "deciding", choice: "apply" });
  });

  it("refuses a second press of the same control", () => {
    const deciding = beginDecision(undecided, "apply");
    // Identity, not equality: the caller tells "this did nothing" by reference.
    expect(beginDecision(deciding, "apply")).toBe(deciding);
  });

  /**
   * The case a per-button guard misses entirely.
   *
   * Apply and Discard are different controls, so disabling the one that was
   * pressed leaves the other live for the render it takes `busy` to arrive.
   * The outcome is a proposal both applied and thrown away.
   */
  it("refuses a different decision after one is taken", () => {
    const deciding = beginDecision(undecided, "apply");
    for (const choice of choices) {
      expect(beginDecision(deciding, choice)).toBe(deciding);
    }
  });

  it("refuses every decision once one has settled", () => {
    // The proposal is spent -- applied, revised into a new one, or discarded.
    const settled = settleDecision(beginDecision(undecided, "apply"));
    for (const choice of choices) {
      expect(beginDecision(settled, choice)).toBe(settled);
    }
    expect(canDecide(settled)).toBe(false);
  });

  it("refuses a decision the shell has withheld", () => {
    // A host that cannot accept the command, or a workspace in recovery.
    expect(canDecide(undecided, true)).toBe(false);
    expect(beginDecision(undecided, "apply", true)).toBe(undecided);
  });

  it("does not let the lock be escaped by deciding twice", () => {
    // The lock is a permission from outside, so no sequence of transitions
    // may end in a decision it forbade.
    const first = beginDecision(undecided, "apply", true);
    expect(beginDecision(first, "discard", true)).toBe(undecided);
  });

  it("carries every choice through", () => {
    for (const choice of choices) {
      expect(decisionInFlight(beginDecision(undecided, choice))).toBe(choice);
    }
  });
});

describe("settling", () => {
  it("keeps the choice that was made", () => {
    expect(settleDecision(beginDecision(undecided, "discard")))
      .toEqual({ kind: "decided", choice: "discard" });
  });

  it("ignores a settle for a decision that never started", () => {
    expect(settleDecision(undecided)).toBe(undecided);
  });

  it("ignores a second settle", () => {
    const settled = settleDecision(beginDecision(undecided, "apply"));
    expect(settleDecision(settled)).toBe(settled);
  });

  it("reports nothing in flight once settled", () => {
    expect(decisionInFlight(settleDecision(beginDecision(undecided, "apply")))).toBeNull();
  });
});

describe("what the controls say", () => {
  /// A user who has just committed to writing files is owed a sentence about
  /// which thing is happening, not three disabled buttons.
  it("names the decision that is running", () => {
    expect(decisionLabel("apply", "apply")).toBe("Applying…");
    expect(decisionLabel("discard", "discard")).toBe("Discarding…");
    expect(decisionLabel("revise", "revise")).toBe("Revising…");
  });

  it("leaves the other controls alone", () => {
    // Relabelling all three would say three things are happening.
    expect(decisionLabel("revise", "apply")).toBe("Revise");
    expect(decisionLabel("discard", "apply")).toBe("Discard");
  });

  it("uses the settled label when nothing is running", () => {
    for (const choice of choices) {
      expect(decisionLabel(choice, null)).not.toContain("…");
    }
  });
});
