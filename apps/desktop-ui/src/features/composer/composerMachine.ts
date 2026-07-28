/**
 * The composer, as one state machine.
 *
 * FE-14. Drafting, picking files, importing them, sending, streaming, stopping
 * and retrying were a set of booleans read at the point of use, and every
 * control decided for itself what a combination meant. Two failures follow
 * directly from that, and both are acceptance criteria here:
 *
 *   - a second Add file press while the native dialog is open stacks a second
 *     dialog, because nothing owns "a dialog is open";
 *   - a double-click on Send submits twice, because the guard lives on the
 *     button rather than on the operation.
 *
 * A transition table fixes both by construction: an event that is not valid for
 * the current state is ignored, and "ignored" is a returned state identical to
 * the one that came in, not a thrown error. The user pressing a key twice is
 * not an exception.
 *
 * What this machine does *not* own
 * -------------------------------
 * Model-context telemetry, which FE-14 names as read-only input. The number of
 * tokens a request will cost does not change what the composer may do next, and
 * wiring it in would make every measurement a transition.
 *
 * Per-file rejections are also outside the state: they are a list that survives
 * across states, because one refused file must not erase the draft or the files
 * that did attach. Making them a state would force exactly that.
 */

import type { AttachmentRejection, ComposerEvent } from "./composerEvents";

/**
 * Where the composer is.
 *
 * `idle` and `drafting` differ only by whether there is text, but the
 * distinction is what lets Send be refused without consulting the draft at
 * every call site.
 */
export type ComposerPhase =
  | "idle"
  | "drafting"
  /** The native dialog is open. A second request is refused while it is. */
  | "opening-file-dialog"
  /** The host is resolving a selection. The draft stays editable. */
  | "importing-attachments"
  /** The request is with the host and has not been accepted yet. */
  | "sending"
  /** The host accepted and is producing a reply. */
  | "streaming"
  /** Stop requested; waiting for the host to acknowledge. */
  | "stopping"
  /** The send failed. The draft and attachments are untouched. */
  | "failed";

export interface ComposerState {
  readonly phase: ComposerPhase;
  readonly draft: string;
  /**
   * Files the host refused, per file.
   *
   * Survives every transition except an explicit dismissal or a reset: a
   * rejection the user has not read is not made stale by them continuing to
   * type.
   */
  readonly rejections: readonly AttachmentRejection[];
  /** Why the last send failed, or null. */
  readonly failure: { readonly reason: string; readonly retryable: boolean } | null;
  /**
   * The file currently being attached again, or null.
   *
   * Named rather than counted, so the row that is retrying is the row that
   * shows it. A single boolean would put the spinner on every retryable row at
   * once, which says the app is doing four things when it is doing one.
   */
  readonly retryingPath: string | null;
  /**
   * Where the composer returns when the dialog or an import ends.
   *
   * Recorded on entry rather than inferred on exit, because "before the dialog"
   * is not derivable from the draft: an empty draft could mean `idle` or a
   * draft the user cleared while the dialog was open.
   */
  readonly resumePhase: "idle" | "drafting";
}

export const initialComposerState: ComposerState = {
  draft: "",
  failure: null,
  phase: "idle",
  rejections: [],
  resumePhase: "idle",
  retryingPath: null,
};

/** The phase a draft alone implies. */
function restingPhase(draft: string): "idle" | "drafting" {
  return draft.trim().length === 0 ? "idle" : "drafting";
}

/** Whether the composer is doing something the user must not start twice. */
export function isBusy(state: ComposerState): boolean {
  return state.phase === "opening-file-dialog"
    || state.phase === "sending"
    || state.phase === "streaming"
    || state.phase === "stopping";
}

/**
 * Whether the draft alone decides the phase.
 *
 * Deliberately *not* `!isBusy`. Those answer different questions: `isBusy` is
 * "may the user start this again", which importing does not block, and this is
 * "does anything else own the phase right now", which importing certainly
 * does. Conflating them meant a keystroke during an import moved the phase to
 * `drafting`, and `import_settled` is only accepted from
 * `importing-attachments` -- so the whole import was discarded in silence.
 */
function isResting(phase: ComposerPhase): boolean {
  return phase === "idle" || phase === "drafting";
}

/**
 * Whether Send may be pressed.
 *
 * The draft check is here rather than on the button so that Enter, the Send
 * control and the command registry cannot disagree -- which is the same
 * argument FE-09 makes about availability generally.
 */
export function canSend(state: ComposerState): boolean {
  if (state.draft.trim().length === 0) return false;
  return state.phase === "drafting"
    || state.phase === "failed"
    // Importing does not block sending: the attachments already resolved are
    // valid, and FE-14 asks that a non-destructive import leave the draft
    // usable.
    || state.phase === "importing-attachments";
}

/** Whether a native file dialog may be opened. */
export function canOpenFileDialog(state: ComposerState): boolean {
  return state.phase === "idle"
    || state.phase === "drafting"
    || state.phase === "failed";
}

/** Whether a stop may be requested. */
export function canStop(state: ComposerState): boolean {
  return state.phase === "streaming";
}

/** Whether the last failure invites another attempt. */
export function canRetry(state: ComposerState): boolean {
  return state.phase === "failed" && state.failure?.retryable === true;
}

/**
 * Whether one refused file may be attached again.
 *
 * Two conditions, and each rules out a control that could not work: the
 * composer must be at rest, and the file must be one the host classified as
 * worth another attempt. A binary file will be binary next time -- offering
 * the button there teaches the user only that pressing it changes nothing.
 *
 * "No second retry while one is running" is not a third condition. A running
 * retry *is* `importing-attachments`, which `isResting` already excludes, and
 * the two are set and cleared in the same transitions -- so a separate
 * `retryingPath !== null` test was a clause nothing could make false on its
 * own, which mutation testing duly found. `retryingPath` says *which* file, for
 * the row that shows it; the phase says whether anything is in flight.
 */
export function canRetryAttachment(state: ComposerState, relativePath: string): boolean {
  if (!isResting(state.phase)) return false;
  return state.rejections.some((rejection) =>
    rejection.relativePath === relativePath && rejection.retryable === true);
}

export function composerTransition(
  state: ComposerState,
  event: ComposerEvent,
): ComposerState {
  switch (event.type) {
    case "draft_changed": {
      /*
       * Always accepted, in every phase. Typing while a request is in flight is
       * the user starting their next message, and refusing the keystroke is how
       * a composer feels broken.
       *
       * The phase only follows the draft when nothing else owns it. During a
       * send it stays `sending`; during an import it stays
       * `importing-attachments`; after a failure it stays `failed`, so the
       * notice does not disappear the moment the user edits what failed.
       */
      const resting = restingPhase(event.draft);
      return {
        ...state,
        draft: event.draft,
        phase: isResting(state.phase) ? resting : state.phase,
        // Always tracks the latest draft, even mid-operation: the user can
        // clear the draft while the native dialog is open, and cancelling must
        // then land on `idle` rather than on the phase they left.
        resumePhase: resting,
      };
    }

    case "file_dialog_requested":
      // The guard *is* the fix for stacked dialogs. Refusing returns the same
      // state rather than throwing: pressing a button twice is not an error.
      if (!canOpenFileDialog(state)) return state;
      return {
        ...state,
        phase: "opening-file-dialog",
        resumePhase: restingPhase(state.draft),
      };

    case "file_dialog_cancelled":
      // No error, no notice, nothing cleared. The user deciding not to attach
      // anything is the dialog working.
      if (state.phase !== "opening-file-dialog") return state;
      return { ...state, phase: state.resumePhase };

    case "import_started":
      if (state.phase !== "opening-file-dialog") return state;
      return { ...state, phase: "importing-attachments" };

    case "import_settled":
      if (state.phase !== "importing-attachments") return state;
      return {
        ...state,
        phase: state.resumePhase,
        // Replaced, not appended: these are the rejections of the pick the user
        // just made, and carrying an older pick's forward would attribute them
        // to files they did not choose this time.
        rejections: event.rejected,
      };

    case "import_failed":
      // Accepted from the dialog phase too. A dialog that fails to open is an
      // import that failed, and forcing the caller through `import_started`
      // just to reach this arm would make it dispatch an event that did not
      // happen.
      if (state.phase !== "importing-attachments"
        && state.phase !== "opening-file-dialog") {
        return state;
      }
      return {
        ...state,
        phase: state.resumePhase,
        rejections: [{ relativePath: "", reason: event.reason }],
      };

    case "rejections_dismissed":
      return { ...state, rejections: [] };

    case "retry_requested": {
      /*
       * Only from rest, and only for a file that is actually refused.
       *
       * The second half matters as much as the first: a retry for a path with
       * no rejection would put the composer into `importing-attachments` with
       * nothing on screen explaining why, and `retry_settled` would then push a
       * refusal for a file the user never saw refused.
       *
       * The resting check is also what stops a second retry starting while one
       * is running, because a running retry is exactly this phase. A separate
       * `retryingPath !== null` test lived here and was removed: the two are
       * set and cleared together, so nothing could make it false on its own.
       */
      if (!isResting(state.phase)) return state;
      const refused = state.rejections
        .some((rejection) => rejection.relativePath === event.relativePath);
      if (!refused) return state;
      return {
        ...state,
        phase: "importing-attachments",
        resumePhase: restingPhase(state.draft),
        retryingPath: event.relativePath,
      };
    }

    case "retry_settled": {
      if (state.retryingPath !== event.relativePath) return state;
      /*
       * This file's old refusal goes whatever the outcome, and only this
       * file's. Every other refusal on screen describes a file nobody
       * retried, and clearing those would tell the user their problems were
       * fixed by an action that never touched them.
       */
      const others = state.rejections
        .filter((rejection) => rejection.relativePath !== event.relativePath);
      return {
        ...state,
        phase: state.resumePhase,
        rejections: event.rejection === null ? others : [...others, event.rejection],
        retryingPath: null,
      };
    }

    case "send_requested":
      // The double-submit guard. A second press while `sending` is ignored,
      // and so is a send with nothing to send.
      if (!canSend(state)) return state;
      return { ...state, failure: null, phase: "sending" };

    case "stream_started":
      if (state.phase !== "sending") return state;
      return { ...state, phase: "streaming" };

    case "stream_finished":
      if (state.phase !== "streaming" && state.phase !== "stopping") return state;
      /*
       * The draft is *not* cleared here.
       *
       * Clearing belongs to the send that was accepted, which the caller does
       * by dispatching `reset` once the host has confirmed consumption. A
       * machine that cleared on stream end would discard whatever the user
       * typed while waiting.
       */
      return { ...state, phase: restingPhase(state.draft) };

    case "stop_requested":
      if (!canStop(state)) return state;
      return { ...state, phase: "stopping" };

    case "send_failed":
      /*
       * The draft and the attachments both survive.
       *
       * FE-14 states it: a failed send preserves them unless the host confirms
       * consumption. The host classifies provider failures, so "try again" is
       * often the right advice -- and it is useless advice if the thing to
       * retry has been thrown away.
       */
      if (state.phase !== "sending" && state.phase !== "streaming"
        && state.phase !== "stopping") {
        return state;
      }
      return {
        ...state,
        failure: { reason: event.reason, retryable: event.retryable },
        phase: "failed",
      };

    case "reset":
      // Everything the composer held for the previous task or the consumed
      // message. Rejections go too: they described a pick that is no longer on
      // screen.
      return initialComposerState;
  }
}

/** Folds a sequence of events, for tests and for replaying a batch. */
export function composerReducer(
  state: ComposerState,
  events: readonly ComposerEvent[],
): ComposerState {
  return events.reduce(composerTransition, state);
}
