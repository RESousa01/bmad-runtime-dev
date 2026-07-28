/**
 * One decision per reviewed proposal.
 *
 * FE-23. The governed changes panel offered Apply, Revise and Discard, each
 * disabled on `state.busy` -- and `busy` is host state that arrives a render
 * late. Between the press and that render the buttons are all still live, so
 * the window that let a double-click send a chat message twice (FE-14) exists
 * here too, over an action that writes to the user's files.
 *
 * The guard is not "no second Apply". It is **no second decision of any kind**.
 * Apply followed quickly by Discard is the case that matters: two different
 * controls, so a per-button guard misses it entirely, and the outcome is a
 * proposal that was both applied and thrown away. `RecoveryReview` already
 * guards its own decisions this way; this is the same rule for the surface that
 * did not.
 *
 * The host is not the safety net here. It refuses a second consume of the same
 * proposal, which is what makes the *result* safe -- but a renderer that fires
 * two commands and shows whichever error came back second is not a surface
 * anyone can read. Deciding once is the renderer's job.
 */

/** What the user can do with a reviewed proposal. */
export type ApprovalChoice = "apply" | "revise" | "discard";

export type ApprovalDecisionState =
  /** Nothing decided. Every choice is available. */
  | { readonly kind: "undecided" }
  /**
   * A choice has been made and is with the host.
   *
   * Named rather than a boolean, so the surface can say "Applying…" instead of
   * disabling three buttons and explaining nothing. A user who has just
   * committed to writing files is owed a sentence about which thing is
   * happening.
   */
  | { readonly kind: "deciding"; readonly choice: ApprovalChoice }
  /**
   * The host answered. Still terminal.
   *
   * A settled decision does not re-open the choice, because the proposal it
   * referred to is spent -- applied, revised into a new one, or discarded. The
   * panel moves to a different state entirely; this exists so a late second
   * press during that transition is refused rather than racing it.
   */
  | { readonly kind: "decided"; readonly choice: ApprovalChoice };

export const undecided: ApprovalDecisionState = { kind: "undecided" };

/**
 * Whether any decision may still be made.
 *
 * `locked` is the shell's business -- a host that cannot accept the command, a
 * workspace in recovery. It is passed in rather than folded into the state
 * because it is a permission withheld from outside, not something this decision
 * did, and a state machine must not be able to transition its way out of it.
 */
export function canDecide(state: ApprovalDecisionState, locked = false): boolean {
  return !locked && state.kind === "undecided";
}

/** The choice currently with the host, or null. */
export function decisionInFlight(state: ApprovalDecisionState): ApprovalChoice | null {
  return state.kind === "deciding" ? state.choice : null;
}

/**
 * Records a decision, or refuses it.
 *
 * Returns the same object when refused, so a caller can tell "this did nothing"
 * by identity and does not have to compare fields. Refusing rather than
 * throwing: pressing a button twice is not an exception, and the second press
 * of a double-click is the ordinary case this exists for.
 */
export function beginDecision(
  state: ApprovalDecisionState,
  choice: ApprovalChoice,
  locked = false,
): ApprovalDecisionState {
  if (!canDecide(state, locked)) return state;
  return { choice, kind: "deciding" };
}

/** Records that the host answered, whatever the answer was. */
export function settleDecision(state: ApprovalDecisionState): ApprovalDecisionState {
  if (state.kind !== "deciding") return state;
  return { choice: state.choice, kind: "decided" };
}

/**
 * What the control should say while its decision is in flight.
 *
 * Only the chosen control changes; the others keep their labels and go
 * disabled. Relabelling all three would suggest three things are happening.
 */
export function decisionLabel(choice: ApprovalChoice, inFlight: ApprovalChoice | null): string {
  const settled: Record<ApprovalChoice, string> = {
    apply: "Apply changes",
    discard: "Discard",
    revise: "Revise",
  };
  if (inFlight !== choice) return settled[choice];
  const active: Record<ApprovalChoice, string> = {
    apply: "Applying…",
    discard: "Discarding…",
    revise: "Revising…",
  };
  return active[choice];
}
