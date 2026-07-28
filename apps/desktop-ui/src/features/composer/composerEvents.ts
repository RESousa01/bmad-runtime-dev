/**
 * What can happen to a composer, as data.
 *
 * FE-14. The composer's behaviour used to be a set of booleans read at the
 * point of use -- `sending`, `suggesting`, `interactionDisabled` -- and every
 * control decided for itself what combination meant. That is how a second
 * native dialog opens on top of the first, and how a double-click sends the
 * same message twice.
 *
 * Naming the events is the half of the fix that makes the other half possible:
 * a transition table can only reject what it can name.
 */

/** One file the host refused, and why, in the user's terms. */
export interface AttachmentRejection {
  readonly relativePath: string;
  /**
   * Why this file is not attached.
   *
   * Per file, because "3 files were skipped" tells the user something went
   * wrong and nothing about which of their choices to revisit.
   */
  readonly reason: string;
  /**
   * Whether attaching this file again could plausibly work.
   *
   * FE-15 asks for per-file retry, and this is the half that decides where it
   * is offered. Most refusals are deterministic: a binary file will be binary
   * next time, a `.env` will still be sensitive, a file outside the workspace
   * will still be outside it. Only an I/O failure is worth another attempt.
   *
   * Offering retry on the rest would be a control that cannot succeed, which
   * is worse than no control -- the user presses it, nothing changes, and they
   * have no reason to look for the real cause.
   *
   * Absent means the same as false. It is optional so callers that have no
   * host classification to offer cannot accidentally claim one.
   */
  readonly retryable?: boolean | undefined;
}

export type ComposerEvent =
  /** The user typed. Carries the whole draft, not a delta. */
  | { readonly type: "draft_changed"; readonly draft: string }
  /** Add file / Add folder pressed. */
  | { readonly type: "file_dialog_requested" }
  /**
   * The dialog closed with nothing chosen.
   *
   * Not a failure. FE-14 states it outright: cancelling produces no toast and
   * no state mutation, because the user deciding not to attach anything is the
   * dialog working.
   */
  | { readonly type: "file_dialog_cancelled" }
  /** The dialog closed with a selection; the host is now resolving it. */
  | { readonly type: "import_started" }
  /** The host resolved the selection. Rejections may be non-empty on success. */
  | {
    readonly type: "import_settled";
    readonly attached: number;
    readonly rejected: readonly AttachmentRejection[];
  }
  /** The import itself failed -- no files were resolved at all. */
  | { readonly type: "import_failed"; readonly reason: string }
  /** The user dismissed the per-file rejection list. */
  | { readonly type: "rejections_dismissed" }
  | { readonly type: "send_requested" }
  /** The host accepted the request and is producing a reply. */
  | { readonly type: "stream_started" }
  | { readonly type: "stream_finished" }
  | { readonly type: "stop_requested" }
  | { readonly type: "send_failed"; readonly reason: string; readonly retryable: boolean }
  /** The task changed, or the workspace binding did. */
  | { readonly type: "reset" };

export type ComposerEventType = ComposerEvent["type"];
