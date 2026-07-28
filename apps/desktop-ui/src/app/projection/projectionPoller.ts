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

/* ------------------------------------------------------------------ */

/**
 * The bound runtime a subscription polls against.
 *
 * Both halves, because every decision the loop makes needs one or the other:
 * the client to call, the bootstrap for the sequence to start from.
 */
export interface BoundProjectionRuntime<Client> {
  readonly client: Client;
  readonly startSequence: number;
}

export interface ProjectionSubscriptionPorts<Client> {
  /** Null unless a live host binding exists. The loop does not run without one. */
  readonly runtime: BoundProjectionRuntime<Client> | null;
  readonly pollIntervalMs: number;
  /**
   * The binding generation, read on every tick.
   *
   * A binding replaced mid-flight makes every remaining step of that tick a
   * no-op: the results belong to a host that is no longer there.
   */
  readonly bindingGeneration: () => number;
  readonly fetchEvents: (
    client: Client,
    cursor: number,
  ) => Promise<readonly ProjectionEventLike[]>;
  readonly fetchSnapshot: (
    client: Client,
  ) => Promise<{ readonly bootMode: string; readonly sequence: number }>;
  readonly reBootstrap: (client: Client) => Promise<{
    readonly bootMode: string;
    readonly supportedCommands: readonly string[];
  }>;
  readonly refreshWorkspaces: (client: Client) => Promise<void>;
  readonly refreshMethodLibrary: (client: Client) => Promise<void>;
  readonly libraryWasRequested: () => boolean;
  readonly markReadOnlyRecovery: (client: Client, sequence: number) => void;
  readonly markHostUnavailable: (client: Client) => void;
  /** Adopts a runtime rebuilt after a cursor gap, keeping the active workspace. */
  readonly adoptReboundRuntime: (
    client: Client,
    bootstrap: { readonly bootMode: string; readonly supportedCommands: readonly string[] },
  ) => void;
}

/**
 * Runs the polling loop until it is torn down.
 *
 * Returned rather than started, so the caller owns the effect boundary and this
 * module owns nothing about React. The returned function stops the loop and is
 * safe to call more than once.
 */
export function startProjectionSubscription<Client>(
  ports: ProjectionSubscriptionPorts<Client>,
): () => void {
  const { runtime } = ports;
  if (runtime === null) return () => undefined;

  const boundGeneration = ports.bindingGeneration();
  let cancelled = false;
  let cursor = runtime.startSequence;
  let timerId: number | undefined;

  /*
   * Stale means one of two things and treats them identically: the effect was
   * torn down, or the host binding was replaced. Either way the results of this
   * tick describe a host the app is no longer talking to, so nothing is
   * written and the loop stops rescheduling itself.
   */
  const isStale = () => cancelled || boundGeneration !== ports.bindingGeneration();

  const reportFailure = (error: unknown) => {
    const failure = classifyProjectionFailure(error);
    if (failure.kind === "recovery") ports.markReadOnlyRecovery(runtime.client, cursor);
    // `retry` reschedules by doing nothing; `cursor_gap` is handled before
    // this is reached.
    else if (failure.kind === "unavailable") ports.markHostUnavailable(runtime.client);
  };

  /**
   * Rebuilds the renderer's authority after a cursor gap.
   *
   * The snapshot the host can offer is summary-only, so it cannot reconstruct
   * the session, approval, execution, evidence, connectivity and update events
   * that fell in the gap. Advancing past them would leave the renderer showing
   * a state it never derived, so this establishes a fresh binding instead.
   */
  const recoverFromCursorGap = async () => {
    const snapshot = await ports.fetchSnapshot(runtime.client);
    if (isStale()) return;
    if (snapshot.bootMode === "read_only_recovery") {
      cursor = snapshot.sequence;
      ports.markReadOnlyRecovery(runtime.client, cursor);
      return;
    }

    let rebound;
    try {
      rebound = await ports.reBootstrap(runtime.client);
    } catch {
      // A host that cannot re-bootstrap is a host that has stopped answering.
      if (!isStale()) ports.markHostUnavailable(runtime.client);
      return;
    }
    if (isStale()) return;

    const reloadLibrary = shouldReloadLibraryAfterRebind({
      bootMode: rebound.bootMode,
      libraryWasRequested: ports.libraryWasRequested(),
      supportedCommands: rebound.supportedCommands,
    });
    ports.adoptReboundRuntime(runtime.client, rebound);
    if (reloadLibrary) void ports.refreshMethodLibrary(runtime.client);
  };

  const poll = async () => {
    // Checked before the call, not only after it. A tick scheduled while the
    // binding was live can fire after it has been replaced, and asking a host
    // the app has stopped talking to costs a request whose answer is thrown
    // away by the very next line.
    if (isStale()) return;
    try {
      const events = await ports.fetchEvents(runtime.client, cursor);
      if (isStale()) return;
      const outcome = decideProjectionOutcome(events);
      if (outcome.cursor !== null) cursor = outcome.cursor;

      if (outcome.refreshWorkspaces) {
        await ports.refreshWorkspaces(runtime.client);
        if (isStale()) return;
      }
      // After the workspace refresh: a batch can carry both, and the recovery
      // transition ends the tick, so doing it first would drop the workspace
      // list the same batch delivered.
      if (outcome.enterReadOnlyRecovery) {
        ports.markReadOnlyRecovery(runtime.client, cursor);
        return;
      }
      if (outcome.refreshMethodLibrary && ports.libraryWasRequested()) {
        await ports.refreshMethodLibrary(runtime.client);
      }
    } catch (error) {
      if (isStale()) return;
      if (classifyProjectionFailure(error).kind !== "cursor_gap") {
        reportFailure(error);
        return;
      }
      try {
        await recoverFromCursorGap();
      } catch (snapshotError) {
        if (!isStale()) reportFailure(snapshotError);
      }
    } finally {
      // Rescheduled in `finally` so a thrown tick does not silently end the
      // subscription -- a poller that stops on one bad response looks exactly
      // like a host that went quiet.
      if (!isStale()) {
        timerId = window.setTimeout(() => void poll(), ports.pollIntervalMs);
      }
    }
  };

  timerId = window.setTimeout(() => void poll(), ports.pollIntervalMs);
  return () => {
    cancelled = true;
    if (timerId !== undefined) window.clearTimeout(timerId);
  };
}
