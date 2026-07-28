/**
 * Runs a governed-changes action against the task and the generation it started
 * under.
 *
 * FE-10 step 3. Propose, decide and undo each hand-wrote the same two captures:
 *
 *   `originTask` — the task the flow belongs to, taken *before* the await, so a
 *   review that arrives after the user has switched tabs is applied to the task
 *   that asked for it rather than the one on screen. A governed change is a
 *   diff the user is being asked to approve; showing it under the wrong task is
 *   asking them to approve edits they did not request.
 *
 *   `generation` — bumped on entry and compared after the await. It is the
 *   supersede mechanism: starting a second flow, switching workspaces, or
 *   rebinding the host all advance it, and every write from the older flow is
 *   then dropped. Without it a slow proposal lands on top of a review the user
 *   is already reading.
 *
 * Both were correct at all three sites. Three correct copies of an invariant is
 * still three places for a fourth to go wrong, and the failure mode is silent:
 * the diff simply belongs to something else.
 */

import { getSafeHostMessage } from "../../lib/hostClient";
import { isRecoveryFailure } from "../hostFailure";

export interface ChangesRunContext {
  /** The task this flow belongs to. Every write must be addressed to it. */
  readonly originTask: string;
  /** False once this run has been superseded; every write must check it. */
  readonly isCurrent: () => boolean;
}

export interface ChangesRunnerPorts {
  /** Bumped on entry to supersede any flow already running. */
  readonly nextGeneration: () => number;
  readonly currentGeneration: () => number;
  /** The task in view when the action starts. */
  readonly originTask: string;
  readonly setError: (message: string | null) => void;
  /**
   * The host asked for recovery, or failed integrity.
   *
   * Separate from `setError` because it is not an error to show -- it is the
   * host saying its own state is untrustworthy, and the app has to change mode
   * rather than print something.
   */
  readonly enterReadOnlyRecovery: () => void;
  /** Puts the flow back to a state the user can retry from, after a failure. */
  readonly resetFlow: (originTask: string) => void;
}

export type RunChangesAction = (
  action: (context: ChangesRunContext) => Promise<void>,
) => Promise<void>;

export function createChangesRunner(ports: ChangesRunnerPorts): RunChangesAction {
  return async function runChangesAction(action) {
    const generation = ports.nextGeneration();
    const { originTask } = ports;
    const isCurrent = () => generation === ports.currentGeneration();

    ports.setError(null);
    try {
      await action({ isCurrent, originTask });
    } catch (error) {
      // Guarded like every other write. A failure from a superseded flow must
      // not blank the review the current one just produced.
      if (!isCurrent()) return;
      // The host consumes a pending proposal on any decision attempt, so a
      // failed apply always requires a fresh review -- the flow goes back to
      // idle rather than pretending the review is still live.
      ports.resetFlow(originTask);
      if (isRecoveryFailure(error)) ports.enterReadOnlyRecovery();
      else ports.setError(getSafeHostMessage(error));
    }
  };
}
