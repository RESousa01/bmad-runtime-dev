/**
 * Runs a reviewed capability action against the task that started it.
 *
 * FE-10 step 3. Prepare, approve, cancel and submit each repeated one ordering
 * by hand, and that ordering is the only thing this module exists to enforce:
 *
 *   capture the routing token *before* awaiting the host,
 *   and apply the result through the token afterwards.
 *
 * A capability run belongs to the task it was started from, and the host call
 * outlives the user's attention -- they switch tabs while a run is preparing.
 * Reading the active session again after the await would apply one task's
 * review to whichever task happens to be on screen: a manifest the user never
 * asked for, offered for approval under someone else's work. Written out four
 * times, that ordering is four chances to get it wrong.
 *
 * The host client is deliberately *not* behind a port here. Each action calls a
 * different, differently-typed command, and a port wide enough to cover all
 * four would have to erase those types -- trading real type safety for the
 * appearance of a boundary. The action closure keeps the typed client; this
 * module supplies the token and the guard.
 */

import { getSafeHostMessage } from "../../lib/hostClient";
import type {
  ActiveCapabilityRunState,
  CapabilityRouting,
  CapabilityRunUpdate,
} from "../../state/taskSessions";

/** The workspace a run is bound to. The epoch is what the host checks. */
export interface CapabilityWorkspace {
  readonly workspaceId: string;
  readonly grantEpoch: number;
}

export interface CapabilitySessionPort {
  readonly captureActive: () => CapabilityRouting;
  readonly route: (token: CapabilityRouting, update: CapabilityRunUpdate) => boolean;
}

export interface CapabilityRunnerPorts {
  /** The run on screen, or null when none is open. */
  readonly run: ActiveCapabilityRunState | null;
  /** Null unless a live host and a bound workspace are both present. */
  readonly workspace: CapabilityWorkspace | null;
  readonly sessions: CapabilitySessionPort;
}

export interface CapabilityRunContext {
  readonly run: ActiveCapabilityRunState;
  readonly workspace: CapabilityWorkspace;
  /** Apply a result to the task that started the run, not the visible one. */
  readonly route: (update: CapabilityRunUpdate) => void;
}

export type CapabilityPhase = ActiveCapabilityRunState["phase"];

export type RunCapabilityAction = (
  phaseGuard: (phase: CapabilityPhase) => boolean,
  action: (context: CapabilityRunContext) => Promise<void>,
) => Promise<void>;

export function createCapabilityRunner(ports: CapabilityRunnerPorts): RunCapabilityAction {
  return async function runCapabilityAction(phaseGuard, action) {
    const { run, sessions, workspace } = ports;
    if (run === null || workspace === null) return;
    // Refuses an action the run is not in the right phase for -- approving
    // something not under review, submitting something never approved. Before
    // the token is captured, so a refused action leaves no trace.
    if (!phaseGuard(run.phase)) return;

    const routing = sessions.captureActive();
    const route = (update: CapabilityRunUpdate) => { sessions.route(routing, update); };
    try {
      await action({ route, run, workspace });
    } catch (error) {
      // Routed, not applied to the active run: a failure belongs to the task
      // that asked, exactly as a success does.
      route((current) => current === null
        ? null
        : { ...current, phase: { kind: "error", message: getSafeHostMessage(error) } });
    }
  };
}

/** Phase guards, named so the call sites read as the rule they encode. */
export const anyPhase = (): boolean => true;
export const underReview = (phase: CapabilityPhase): boolean => phase.kind === "review";
export const approved = (phase: CapabilityPhase): boolean => phase.kind === "approved";
