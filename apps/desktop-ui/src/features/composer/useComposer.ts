/**
 * The composer machine, bound to React.
 *
 * FE-14. The machine's guarantees are only real if every call site goes through
 * it, so this hook does not hand out a `dispatch` and hope. It exposes the
 * operations -- `send`, `openFileDialog` -- and runs the async work itself,
 * which is the only way the guard and the work it guards cannot come apart.
 *
 * Why a ref as well as state
 * --------------------------
 * `setState` is asynchronous. A handler reading the `state` from its closure
 * sees the value from the last *render*, and for a double-click that is the
 * value from before the first click -- which is precisely the case the guard
 * exists to reject. The ref is the synchronous copy the guard reads; the state
 * is the copy React renders from. Same reasoning, and the same `commit` shape,
 * as `useCapabilitySessions` and the FE-13 task session controller.
 */

import { useCallback, useRef, useState } from "react";

import { HostCommandError } from "../../lib/hostClient";
import type { AttachmentRejection, ComposerEvent } from "./composerEvents";
import {
  canOpenFileDialog as canOpenFileDialogIn,
  canRetry as canRetryIn,
  canSend as canSendIn,
  canStop as canStopIn,
  composerTransition,
  initialComposerState,
  type ComposerState,
} from "./composerMachine";

export interface UseComposerOptions {
  /** The draft this task already had. Read once, on mount. */
  readonly initialDraft?: string | undefined;
  /**
   * Whether the composer may be used at all.
   *
   * Deliberately *not* a phase. Recovery mode, an inert background task and a
   * BMAD request in flight are permissions withheld from outside; a phase is
   * something the composer is doing. Modelling a permission as a phase would
   * mean the machine could transition out of it on its own.
   */
  readonly locked?: boolean | undefined;
  /** Reports the draft upwards so the task can own it across a switch. */
  readonly onDraftChange?: ((draft: string) => void) | undefined;
}

export interface ComposerController {
  readonly state: ComposerState;
  readonly draft: string;
  readonly rejections: readonly AttachmentRejection[];
  readonly failure: ComposerState["failure"];
  /** True while a send is with the host, for the in-flight affordance. */
  readonly isSending: boolean;
  readonly canSend: boolean;
  readonly canOpenFileDialog: boolean;
  readonly canStop: boolean;
  readonly canRetry: boolean;
  readonly setDraft: (draft: string) => void;
  readonly dismissRejections: () => void;
  /**
   * Runs `open` under the dialog guard.
   *
   * A second press while one dialog is open is dropped here, not by the
   * button's disabled state: the button is re-rendered asynchronously and the
   * second click of a double-click lands before that.
   */
  readonly openFileDialog: (open: () => Promise<ImportOutcome | void>) => void;
  readonly send: (submit: (intent: string) => Promise<void>) => void;
  readonly stop: (abort: () => Promise<void>) => void;
  /** Clears everything -- a task switch, or a message the host consumed. */
  readonly reset: () => void;
}

/** What a file import resolved to. `undefined` means "nothing was chosen". */
export interface ImportOutcome {
  readonly attached: number;
  readonly rejected: readonly AttachmentRejection[];
}

/** The host's own classification, never the UI's guess. */
function describeFailure(error: unknown): { reason: string; retryable: boolean } {
  if (error instanceof HostCommandError) {
    return { reason: error.details.safeMessage, retryable: error.details.retryable };
  }
  return {
    reason: error instanceof Error && error.message.length > 0
      ? error.message
      : "That did not go through.",
    // An error the host did not classify is not known to be transient, and
    // offering a retry that cannot work is worse than offering none.
    retryable: false,
  };
}

export function useComposer(options: UseComposerOptions = {}): ComposerController {
  const { locked = false, onDraftChange } = options;
  const initial: ComposerState = {
    ...initialComposerState,
    draft: options.initialDraft ?? "",
    phase: (options.initialDraft ?? "").trim().length > 0 ? "drafting" : "idle",
    resumePhase: (options.initialDraft ?? "").trim().length > 0 ? "drafting" : "idle",
  };
  const stateRef = useRef<ComposerState>(initial);
  const [state, setState] = useState<ComposerState>(initial);

  /// Writes both copies. Never called during render -- see the module note.
  const dispatch = useCallback((event: ComposerEvent): ComposerState => {
    const next = composerTransition(stateRef.current, event);
    if (next === stateRef.current) return next;
    stateRef.current = next;
    setState(next);
    return next;
  }, []);

  const setDraft = useCallback((draft: string) => {
    dispatch({ type: "draft_changed", draft });
    onDraftChange?.(draft);
  }, [dispatch, onDraftChange]);

  const openFileDialog = useCallback((open: () => Promise<ImportOutcome | void>) => {
    /*
     * Deliberately not gated on `locked`.
     *
     * `locked` is "a turn is with the host, or the shell withheld the
     * composer". Attaching a file needs neither the lane nor a model -- the
     * user gathering context while a reply is in flight is ordinary, and Add
     * file has always stayed live through it. The only thing that blocks a
     * dialog is another dialog.
     *
     * Read from the ref, not the render's state: the second click of a
     * double-click arrives before React has re-rendered with the first
     * click's phase.
     */
    if (!canOpenFileDialogIn(stateRef.current)) return;
    dispatch({ type: "file_dialog_requested" });
    void (async () => {
      let outcome: ImportOutcome | void;
      try {
        outcome = await open();
      } catch (error) {
        dispatch({ type: "import_failed", reason: describeFailure(error).reason });
        return;
      }
      if (outcome === undefined) {
        // Cancelled. No toast, no notice, nothing cleared -- FE-14 states it
        // outright, because the user deciding not to attach anything is the
        // dialog working.
        dispatch({ type: "file_dialog_cancelled" });
        return;
      }
      dispatch({ type: "import_started" });
      dispatch({ type: "import_settled", attached: outcome.attached, rejected: outcome.rejected });
    })();
  }, [dispatch]);

  const send = useCallback((submit: (intent: string) => Promise<void>) => {
    if (locked || !canSendIn(stateRef.current)) return;
    const intent = stateRef.current.draft.trim();
    dispatch({ type: "send_requested" });
    void (async () => {
      try {
        await submit(intent);
      } catch (error) {
        const { reason, retryable } = describeFailure(error);
        // The draft survives. FE-14: preserved unless the host confirms
        // consumption -- and "try again" is useless advice if the thing to
        // retry has been thrown away.
        dispatch({ type: "send_failed", reason, retryable });
        return;
      }
      /*
       * Straight to `reset`, with no synthesised stream events.
       *
       * `submit` resolving means the host took the message; it is not a stream
       * boundary, because this surface never sees one. Dispatching
       * `stream_started` here would be the composer inventing an observation it
       * did not make -- and the `streaming` phase exists precisely so that a
       * caller which *does* observe the stream can report it truthfully.
       */
      onDraftChange?.("");
      dispatch({ type: "reset" });
    })();
  }, [dispatch, locked, onDraftChange]);

  const stop = useCallback((abort: () => Promise<void>) => {
    if (!canStopIn(stateRef.current)) return;
    dispatch({ type: "stop_requested" });
    void (async () => {
      try {
        await abort();
      } catch (error) {
        const { reason, retryable } = describeFailure(error);
        dispatch({ type: "send_failed", reason, retryable });
        return;
      }
      dispatch({ type: "stream_finished" });
    })();
  }, [dispatch]);

  const dismissRejections = useCallback(() => {
    dispatch({ type: "rejections_dismissed" });
  }, [dispatch]);

  const reset = useCallback(() => {
    dispatch({ type: "reset" });
  }, [dispatch]);

  return {
    canOpenFileDialog: canOpenFileDialogIn(state),
    canRetry: !locked && canRetryIn(state),
    canSend: !locked && canSendIn(state),
    canStop: !locked && canStopIn(state),
    dismissRejections,
    draft: state.draft,
    failure: state.failure,
    isSending: state.phase === "sending" || state.phase === "streaming",
    openFileDialog,
    rejections: state.rejections,
    reset,
    send,
    setDraft,
    state,
    stop,
  };
}
