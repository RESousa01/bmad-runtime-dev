import { describe, expect, it } from "vitest";

import {
  leadingDemand,
  reviewCenterState,
  type ReviewCenterInput,
} from "./reviewCenter";

function input(overrides: Partial<ReviewCenterInput> = {}): ReviewCenterInput {
  return {
    hasGitSource: true,
    hasPreparedRecovery: false,
    hasProposal: false,
    ...overrides,
  };
}

describe("what is waiting", () => {
  it("shows git changes when nothing is waiting on an answer", () => {
    const state = reviewCenterState(input());
    expect(state.demands).toEqual([{ kind: "git" }]);
    expect(state.needsAnswer).toBe(false);
  });

  /**
   * The failure this module replaces.
   *
   * The panel showed git *or* proposals, so a user with both saw one and the
   * panel chose which. Both are present now, and the ordering below decides
   * which is in front rather than which exists.
   */
  it("shows both when there is a proposal and a dirty tree", () => {
    const state = reviewCenterState(input({ hasProposal: true }));
    expect(state.demands.map((demand) => demand.kind)).toEqual(["proposal", "git"]);
  });

  it("never lets information displace a question", () => {
    // A dirty tree is true whether or not anyone looks. A proposal is the app
    // stopped, waiting.
    for (const hasGitSource of [true, false]) {
      const state = reviewCenterState(input({ hasGitSource, hasProposal: true }));
      expect(leadingDemand(state)).toEqual({ kind: "proposal" });
    }
  });

  /**
   * Recovery outranks a proposal.
   *
   * Approving changes writes to the workspace on the strength of a store that
   * has said it cannot be trusted. Putting that question first asks the user to
   * decide on a basis the app has already disowned.
   */
  it("puts recovery ahead of a proposal", () => {
    const state = reviewCenterState(input({
      hasPreparedRecovery: true,
      hasProposal: true,
    }));
    expect(state.demands.map((demand) => demand.kind)).toEqual(["recovery", "proposal", "git"]);
    expect(leadingDemand(state)).toEqual({ kind: "recovery" });
  });

  it("says nothing is waiting when there is no source and no decision", () => {
    const empty = reviewCenterState(input({ hasGitSource: false }));
    expect(empty.demands).toEqual([]);
    expect(leadingDemand(empty)).toBeNull();
  });

  /// A section that vanishes when it has nothing to report is
  /// indistinguishable from one that failed to load, so a bound source always
  /// gets its section -- "no changes" is a thing the user came to check.
  it("keeps the git section for a bound source with a clean tree", () => {
    expect(reviewCenterState(input()).demands).toEqual([{ kind: "git" }]);
  });
});

describe("needsAnswer", () => {
  /// The tab marker. Marking a dirty tree as unanswered teaches the user to
  /// ignore the marker that matters.
  it("is true for decisions and false for changes", () => {
    expect(reviewCenterState(input({ hasProposal: true })).needsAnswer).toBe(true);
    expect(reviewCenterState(input({ hasPreparedRecovery: true })).needsAnswer).toBe(true);
    expect(reviewCenterState(input()).needsAnswer).toBe(false);
  });

  it("stays true while either decision stands", () => {
    // A recovery answered with a proposal still pending is not "nothing left".
    expect(reviewCenterState(input({ hasPreparedRecovery: true, hasProposal: true })).needsAnswer)
      .toBe(true);
  });
});
