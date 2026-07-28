/**
 * Keeping the renderer in step with the host's projection stream.
 *
 * FE-10 step 3. The single largest block left in App.tsx: 150 lines of polling,
 * gap recovery and failure classification inside one `useEffect`. What it does
 * is not complicated; what made it hard was that its three distinct jobs were
 * interleaved, and one of them -- deciding what a `conflict` means -- is the
 * most consequential thing in the file.
 *
 * The cursor gap
 * --------------
 * `conflict` from `projectionEvents` means the renderer's cursor is behind what
 * the host still holds: events happened that it can no longer replay. The
 * snapshot it can offer is summary-only, so it cannot reconstruct the session,
 * approval, execution, evidence, connectivity and update events that fell in
 * the gap. Advancing past them would leave the renderer showing a state it
 * never derived. So the response is to re-bootstrap -- establish a fresh
 * renderer authority -- rather than skip ahead.
 *
 * `decideProjectionOutcome` below is that reasoning, extracted as a pure
 * function over a batch of events, so what the poller *does* with a batch can
 * be read and tested without a timer.
 */

import { HostCommandError } from "../../lib/hostClient";
import { isRecoveryFailure } from "../hostFailure";

/** The projection events this shell reacts to. */
export interface ProjectionEventLike {
  readonly event:
    | { readonly type: "boot_state_changed"; readonly projection: { readonly mode: string } }
    | { readonly type: "workspace_changed" }
    | { readonly type: "bmad.projection_changed" }
    | { readonly type: string };
  readonly sequence: number;
}

/** What a batch of events asks the shell to do. */
export interface ProjectionOutcome {
  /** The host said it is in recovery. Everything else in the batch is moot. */
  readonly enterReadOnlyRecovery: boolean;
  readonly refreshWorkspaces: boolean;
  readonly refreshMethodLibrary: boolean;
  /** The last sequence seen, or null when the batch was empty. */
  readonly cursor: number | null;
}

export function decideProjectionOutcome(
  events: readonly ProjectionEventLike[],
): ProjectionOutcome {
  let enterReadOnlyRecovery = false;
  let refreshWorkspaces = false;
  let refreshMethodLibrary = false;
  for (const { event } of events) {
    if (event.type === "boot_state_changed"
      && "projection" in event
      && event.projection.mode === "read_only_recovery") {
      enterReadOnlyRecovery = true;
    }
    if (event.type === "workspace_changed") refreshWorkspaces = true;
    if (event.type === "bmad.projection_changed") refreshMethodLibrary = true;
  }
  return {
    // The cursor only advances on a non-empty batch. Advancing on an empty one
    // would move past a sequence the host never sent.
    cursor: events.length > 0 ? events.at(-1)!.sequence : null,
    enterReadOnlyRecovery,
    refreshMethodLibrary,
    refreshWorkspaces,
  };
}

/** How a poll failure should be handled. */
export type ProjectionFailure =
  /** The host distrusts its own state. */
  | { readonly kind: "recovery" }
  /** The cursor is behind what the host can replay; re-establish authority. */
  | { readonly kind: "cursor_gap" }
  /** Worth trying again on the next tick. */
  | { readonly kind: "retry" }
  /** The host stopped answering. */
  | { readonly kind: "unavailable" };

/**
 * Classifies a poll failure.
 *
 * `conflict` is singled out before the recovery check because it is not a
 * failure of the host at all -- it is this renderer having fallen behind, and
 * the response is to catch up rather than to tear anything down.
 */
export function classifyProjectionFailure(error: unknown): ProjectionFailure {
  if (error instanceof HostCommandError && error.details.code === "conflict") {
    return { kind: "cursor_gap" };
  }
  if (isRecoveryFailure(error)) return { kind: "recovery" };
  if (error instanceof HostCommandError && error.details.retryable) {
    return { kind: "retry" };
  }
  return { kind: "unavailable" };
}

/**
 * Whether a re-bootstrap should reload the method library.
 *
 * Only if it had been asked for before the gap: re-establishing authority is
 * not a reason to start fetching something the user never opened.
 */
export function shouldReloadLibraryAfterRebind(input: {
  readonly libraryWasRequested: boolean;
  readonly bootMode: string;
  readonly supportedCommands: readonly string[];
}): boolean {
  return input.libraryWasRequested
    && input.bootMode === "ready"
    && input.supportedCommands.includes("bmad.library.snapshot");
}
