import { describe, expect, it } from "vitest";

import {
  beginDecision,
  canDecide,
  decisionInFlight,
  decisionLabel,
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

/*
 * There is no "settled" state to test, and that is deliberate.
 *
 * One existed and was removed. A decision ends when the proposal it referred
 * to is gone, which the caller already knows from the approval id changing --
 * so a settled state added an effect writing state on every host reply to
 * represent something already derivable. Nothing could observe it separately
 * from `deciding`, which means it was not a state.
 */

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
