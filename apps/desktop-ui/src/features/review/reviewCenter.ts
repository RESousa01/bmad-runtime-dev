/**
 * What is waiting to be reviewed, from both sources, in one order.
 *
 * FE-22. Git changes and governed proposals were two surfaces behind a mode
 * switch: the panel showed one or the other, and whichever was hidden shrank to
 * a 14px icon with a dot on it. A user with a proposal awaiting approval *and*
 * a dirty working tree saw one of those two facts, and the panel decided which.
 *
 * It decided badly in a way no boolean could fix. The switch fired from an
 * effect watching `proposalAttention` go from false to true, so a second
 * proposal arriving while the flag was already true moved nothing, and a user
 * who had navigated back to Git sat in front of an unanswered decision
 * represented by a dot. Nothing was broken; the decision was simply not on
 * screen.
 *
 * The fix is not a better switch. It is that these are different kinds of
 * thing, and only one of them is a question:
 *
 *   - a governed proposal and a prepared recovery are **decisions**: the app is
 *     stopped until the user answers;
 *   - git changes are **information**: true whether or not anyone looks.
 *
 * Information must never displace a question. That is the whole ordering rule,
 * and it is why this is a module with tests rather than a `useState` holding
 * `"git" | "proposals"`.
 */

/** One thing the Review Center has to show. */
export type ReviewDemand =
  /**
   * The local authority store is in recovery and a review is prepared.
   *
   * Ranked above a change proposal deliberately. Approving changes writes to
   * the workspace on the strength of a store that has said it cannot be
   * trusted; asking the user to do that first would be asking them to decide
   * on a basis the app has already disowned.
   */
  | { readonly kind: "recovery" }
  /** A reviewed change proposal awaiting apply, revise or discard. */
  | { readonly kind: "proposal" }
  /** The working tree. Never a question. */
  | { readonly kind: "git" };

export interface ReviewCenterInput {
  readonly hasPreparedRecovery: boolean;
  readonly hasProposal: boolean;
  /**
   * Whether a git source is bound at all.
   *
   * Deliberately not a file count. `GitReviewSource` is a fetcher, and the
   * count is read inside the git panel -- so a container asking for one here
   * would either have to lift that read or invent a number. Whether the
   * section exists is what this layer knows, and the only thing it needs in
   * order to order the sections.
   */
  readonly hasGitSource: boolean;
}

export interface ReviewCenterState {
  /** Every demand, decisions first. */
  readonly demands: readonly ReviewDemand[];
  /**
   * Whether something needs an answer.
   *
   * Drives the tab marker. True for decisions only -- a dirty tree is not an
   * unanswered question, and marking it as one teaches the user to ignore the
   * marker that matters.
   */
  readonly needsAnswer: boolean;
}

export function reviewCenterState({
  hasGitSource,
  hasPreparedRecovery,
  hasProposal,
}: ReviewCenterInput): ReviewCenterState {
  const demands: ReviewDemand[] = [];
  if (hasPreparedRecovery) demands.push({ kind: "recovery" });
  if (hasProposal) demands.push({ kind: "proposal" });
  // Present even when the tree is clean: "no changes" is a thing the user
  // came to check, and a section that vanishes when it has nothing to report
  // is indistinguishable from one that failed to load.
  if (hasGitSource) demands.push({ kind: "git" });
  return { demands, needsAnswer: hasPreparedRecovery || hasProposal };
}

/**
 * Which demand the surface should open on.
 *
 * The first, which is the highest-ranked one present -- so a pending decision
 * is what the user lands on, every time, without an effect having to notice a
 * transition. The bug this replaces was entirely in the noticing.
 */
export function leadingDemand(state: ReviewCenterState): ReviewDemand | null {
  return state.demands[0] ?? null;
}

/*
 * There is no `isDismissible`. One existed, restating `needsAnswer` for a
 * single demand, and it had no caller once the panel pinned on `needsAnswer`
 * directly -- two names for one fact, which is the shape a disagreement
 * eventually grows in.
 */
