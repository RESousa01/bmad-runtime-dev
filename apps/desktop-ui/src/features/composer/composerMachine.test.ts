import { describe, expect, it } from "vitest";

import {
  canOpenFileDialog,
  canRetry,
  canRetryAttachment,
  canSend,
  canStop,
  composerReducer,
  composerTransition,
  initialComposerState,
  isBusy,
  type ComposerState,
} from "./composerMachine";
import type { ComposerEvent } from "./composerEvents";

/** Folds events onto the initial state. */
function after(...events: readonly ComposerEvent[]): ComposerState {
  return composerReducer(initialComposerState, events);
}

const typed: ComposerEvent = { type: "draft_changed", draft: "fix the login" };
const openDialog: ComposerEvent = { type: "file_dialog_requested" };
const startImport: ComposerEvent = { type: "import_started" };
const send: ComposerEvent = { type: "send_requested" };

describe("drafting", () => {
  it("starts idle with nothing to send", () => {
    expect(initialComposerState.phase).toBe("idle");
    expect(canSend(initialComposerState)).toBe(false);
  });

  it("becomes drafting once there is text", () => {
    expect(after(typed).phase).toBe("drafting");
    expect(canSend(after(typed))).toBe(true);
  });

  it("returns to idle when the text is cleared", () => {
    expect(after(typed, { type: "draft_changed", draft: "" }).phase).toBe("idle");
  });

  it("treats whitespace as nothing to send", () => {
    expect(canSend(after({ type: "draft_changed", draft: "   " }))).toBe(false);
  });

  /// Refusing a keystroke is how a composer feels broken. Typing during a
  /// request is the user starting their next message.
  it("accepts typing while a request is in flight, without leaving the phase", () => {
    const state = after(typed, send, { type: "draft_changed", draft: "next message" });
    expect(state.phase).toBe("sending");
    expect(state.draft).toBe("next message");
  });
});

describe("the native file dialog", () => {
  /// FE-14 acceptance: stacked native dialogs are impossible.
  it("refuses a second request while one is open", () => {
    const open = after(typed, openDialog);
    expect(open.phase).toBe("opening-file-dialog");
    expect(canOpenFileDialog(open)).toBe(false);
    expect(composerTransition(open, openDialog)).toBe(open);
  });

  it("refuses a dialog while sending or streaming", () => {
    expect(canOpenFileDialog(after(typed, send))).toBe(false);
    expect(canOpenFileDialog(after(typed, send, { type: "stream_started" }))).toBe(false);
  });

  /// FE-14 acceptance: cancelling produces no toast and no state mutation.
  it("returns to drafting on cancel, with nothing recorded", () => {
    const before = after(typed);
    const cancelled = composerReducer(before, [openDialog, { type: "file_dialog_cancelled" }]);
    expect(cancelled).toEqual(before);
  });

  it("returns to idle on cancel when there was no draft", () => {
    const cancelled = after(openDialog, { type: "file_dialog_cancelled" });
    expect(cancelled).toEqual(initialComposerState);
  });

  it("returns to the phase the draft implies, not the one before the dialog", () => {
    // The user can clear the draft while the dialog is open. Cancelling must
    // then land on idle, not on the drafting phase they left.
    const state = composerReducer(after(typed), [
      openDialog,
      { type: "draft_changed", draft: "" },
      { type: "file_dialog_cancelled" },
    ]);
    expect(state.phase).toBe("idle");
  });

  it("stays open while the user edits the draft behind it", () => {
    const state = composerReducer(after(typed), [
      openDialog,
      { type: "draft_changed", draft: "changed my mind" },
    ]);
    expect(state.phase).toBe("opening-file-dialog");
  });

  it("ignores a cancel that no dialog preceded", () => {
    const before = after(typed);
    expect(composerTransition(before, { type: "file_dialog_cancelled" })).toBe(before);
  });
});

describe("importing attachments", () => {
  it("keeps the draft editable during an import", () => {
    // FE-14: a non-destructive import must not lock the draft.
    const state = composerReducer(after(typed, openDialog, startImport), [
      { type: "draft_changed", draft: "still typing" },
    ]);
    expect(state.phase).toBe("importing-attachments");
    expect(state.draft).toBe("still typing");
  });

  /// FE-14 acceptance: a send during an import is allowed -- the files that
  /// have already resolved are valid.
  it("allows a send while an import is running", () => {
    expect(canSend(after(typed, openDialog, startImport))).toBe(true);
  });

  /// Regression. Typing used to move the phase back to `drafting`, and
  /// `import_settled` is only accepted from `importing-attachments` -- so one
  /// keystroke discarded the entire import with no notice at all.
  it("still accepts the result after the user types mid-import", () => {
    const state = composerReducer(after(typed, openDialog, startImport), [
      { type: "draft_changed", draft: "still typing" },
      {
        type: "import_settled",
        attached: 1,
        rejected: [{ relativePath: "a.bin", reason: "Not readable." }],
      },
    ]);
    expect(state.rejections).toHaveLength(1);
    expect(state.phase).toBe("drafting");
  });

  /// FE-14 acceptance: partial failure is visible per file and recoverable.
  it("reports each refused file rather than a count", () => {
    const state = composerReducer(after(typed, openDialog, startImport), [{
      type: "import_settled",
      attached: 2,
      rejected: [
        { relativePath: "vendor/blob.bin", reason: "Not a readable text file." },
        { relativePath: "../outside.ts", reason: "Outside the workspace." },
      ],
    }]);
    expect(state.rejections).toHaveLength(2);
    expect(state.rejections[0]?.relativePath).toBe("vendor/blob.bin");
    // And the files that did attach are unaffected: the phase returns to
    // drafting rather than to an error state.
    expect(state.phase).toBe("drafting");
  });

  it("keeps the draft when files are refused", () => {
    const state = composerReducer(after(typed, openDialog, startImport), [{
      type: "import_settled",
      attached: 0,
      rejected: [{ relativePath: "a.bin", reason: "Not readable." }],
    }]);
    expect(state.draft).toBe("fix the login");
  });

  it("replaces the previous pick's rejections rather than accumulating them", () => {
    // Carrying an older pick's forward would attribute them to files the user
    // did not choose this time.
    const first = composerReducer(after(typed, openDialog, startImport), [{
      type: "import_settled", attached: 0,
      rejected: [{ relativePath: "old.bin", reason: "Not readable." }],
    }]);
    const second = composerReducer(first, [openDialog, startImport, {
      type: "import_settled", attached: 1, rejected: [],
    }]);
    expect(second.rejections).toEqual([]);
  });

  it("keeps a rejection visible while the user carries on typing", () => {
    const state = composerReducer(after(typed, openDialog, startImport), [
      { type: "import_settled", attached: 0, rejected: [{ relativePath: "a.bin", reason: "No." }] },
      { type: "draft_changed", draft: "more text" },
    ]);
    expect(state.rejections).toHaveLength(1);
  });

  it("clears rejections only when dismissed", () => {
    const state = composerReducer(after(typed, openDialog, startImport), [
      { type: "import_settled", attached: 0, rejected: [{ relativePath: "a.bin", reason: "No." }] },
      { type: "rejections_dismissed" },
    ]);
    expect(state.rejections).toEqual([]);
  });

  it("reports a dialog that never opened", () => {
    // Reached without `import_started`, because no import ever began.
    const state = composerReducer(after(typed), [
      openDialog,
      { type: "import_failed", reason: "The file dialog could not open." },
    ]);
    expect(state.phase).toBe("drafting");
    expect(state.rejections[0]?.reason).toBe("The file dialog could not open.");
  });

  it("reports an import that failed outright", () => {
    const state = composerReducer(after(typed, openDialog, startImport), [
      { type: "import_failed", reason: "The workspace grant was revoked." },
    ]);
    expect(state.rejections[0]?.reason).toBe("The workspace grant was revoked.");
    expect(state.phase).toBe("drafting");
  });
});

describe("sending", () => {
  /// FE-14 acceptance: double-submit is impossible.
  it("ignores a second send while one is in flight", () => {
    const sending = after(typed, send);
    expect(sending.phase).toBe("sending");
    expect(composerTransition(sending, send)).toBe(sending);
  });

  it("refuses a send with nothing to send", () => {
    expect(composerTransition(initialComposerState, send)).toBe(initialComposerState);
  });

  it("moves to streaming once the host accepts", () => {
    expect(after(typed, send, { type: "stream_started" }).phase).toBe("streaming");
  });

  it("returns to drafting when the stream finishes with text still typed", () => {
    // The draft is not cleared by the stream ending -- clearing belongs to the
    // send that was accepted, and the user may have typed their next message
    // while waiting.
    const state = after(typed, send, { type: "stream_started" },
      { type: "draft_changed", draft: "next one" }, { type: "stream_finished" });
    expect(state.phase).toBe("drafting");
    expect(state.draft).toBe("next one");
  });

  it("ignores a stream start that no send preceded", () => {
    const before = after(typed);
    expect(composerTransition(before, { type: "stream_started" })).toBe(before);
  });
});

describe("stopping", () => {
  /// FE-14 acceptance: stop has an explicit, testable state.
  it("can be stopped only while streaming", () => {
    expect(canStop(after(typed, send))).toBe(false);
    expect(canStop(after(typed, send, { type: "stream_started" }))).toBe(true);
  });

  it("waits for the host to acknowledge a stop", () => {
    const stopping = after(typed, send, { type: "stream_started" }, { type: "stop_requested" });
    expect(stopping.phase).toBe("stopping");
    // A second stop is not a second request.
    expect(composerTransition(stopping, { type: "stop_requested" })).toBe(stopping);
  });

  it("settles when the stopped stream finishes", () => {
    const state = after(typed, send, { type: "stream_started" },
      { type: "stop_requested" }, { type: "stream_finished" });
    expect(state.phase).toBe("drafting");
  });
});

describe("failure and retry", () => {
  const failed: ComposerEvent = {
    type: "send_failed", reason: "The provider rate-limited this request.", retryable: true,
  };

  /// FE-14 acceptance: a failed send preserves the draft and attachments
  /// unless the host confirms consumption.
  it("keeps the draft when a send fails", () => {
    const state = after(typed, send, failed);
    expect(state.phase).toBe("failed");
    expect(state.draft).toBe("fix the login");
  });

  it("keeps rejections through a failed send", () => {
    const state = composerReducer(after(typed, openDialog, startImport), [
      { type: "import_settled", attached: 1, rejected: [{ relativePath: "a.bin", reason: "No." }] },
      send,
      failed,
    ]);
    expect(state.rejections).toHaveLength(1);
  });

  /// FE-14 acceptance: retry has an explicit, testable state.
  it("offers a retry only when the host says the failure is retryable", () => {
    expect(canRetry(after(typed, send, failed))).toBe(true);
    expect(canRetry(after(typed, send, {
      type: "send_failed", reason: "That API key was rejected.", retryable: false,
    }))).toBe(false);
  });

  it("keeps the notice while the user edits what failed", () => {
    // Editing the message is the most likely response to a failure. The
    // explanation of why it failed has to survive it.
    const state = after(typed, send, failed, { type: "draft_changed", draft: "shorter" });
    expect(state.phase).toBe("failed");
    expect(state.failure?.reason).toBe("The provider rate-limited this request.");
  });

  it("allows another send from the failed state", () => {
    // Which is what "retry" is: the draft is still there.
    const state = after(typed, send, failed);
    expect(canSend(state)).toBe(true);
    expect(composerTransition(state, send).phase).toBe("sending");
  });

  it("clears the previous failure when a new send starts", () => {
    const state = after(typed, send, failed, send);
    expect(state.failure).toBeNull();
  });

  it("can fail from streaming, which is a host disconnect mid-reply", () => {
    const state = after(typed, send, { type: "stream_started" }, failed);
    expect(state.phase).toBe("failed");
    expect(state.draft).toBe("fix the login");
  });

  it("can fail while stopping", () => {
    const state = after(typed, send, { type: "stream_started" },
      { type: "stop_requested" }, failed);
    expect(state.phase).toBe("failed");
  });

  it("ignores a failure for a send that never started", () => {
    const before = after(typed);
    expect(composerTransition(before, failed)).toBe(before);
  });

  it("allows a file dialog from the failed state", () => {
    // The user may want to attach the file that would have made it work.
    expect(canOpenFileDialog(after(typed, send, failed))).toBe(true);
  });
});

/**
 * FE-15's per-file retry.
 *
 * A retry describes one file. The settle that follows a pick describes a whole
 * selection and replaces the refusal list, so folding a retry into it would
 * make attaching one file quietly clear the warnings about every other.
 */
describe("retrying one refused file", () => {
  const refused: ComposerEvent = {
    type: "import_settled",
    attached: 1,
    rejected: [
      { relativePath: "src/locked.ts", reason: "Could not be read.", retryable: true },
      { relativePath: "src/image.bin", reason: "Not a readable text file.", retryable: false },
      { relativePath: "", reason: "1 file was outside this workspace." },
    ],
  };
  const withRefusals = () => composerReducer(after(typed), [openDialog, startImport, refused]);

  it("offers a retry only for the file the host said could go differently", () => {
    const state = withRefusals();
    expect(canRetryAttachment(state, "src/locked.ts")).toBe(true);
    expect(canRetryAttachment(state, "src/image.bin")).toBe(false);
  });

  it("offers no retry for a refusal the host would not name", () => {
    // Retrying needs a file to name, and this is the one it has no name for.
    expect(canRetryAttachment(withRefusals(), "")).toBe(false);
  });

  it("refuses a retry for a file that was never refused", () => {
    // Otherwise the composer enters an import with nothing on screen saying
    // why, and the settle pushes a refusal the user never saw raised.
    const state = withRefusals();
    expect(canRetryAttachment(state, "src/never-picked.ts")).toBe(false);
    expect(composerTransition(state, {
      type: "retry_requested", relativePath: "src/never-picked.ts",
    })).toBe(state);
  });

  it("names the file it is retrying, so one row can show it", () => {
    const state = composerTransition(withRefusals(), {
      type: "retry_requested", relativePath: "src/locked.ts",
    });
    expect(state.phase).toBe("importing-attachments");
    expect(state.retryingPath).toBe("src/locked.ts");
  });

  it("refuses a second retry while one is running", () => {
    const running = composerTransition(withRefusals(), {
      type: "retry_requested", relativePath: "src/locked.ts",
    });
    expect(canRetryAttachment(running, "src/locked.ts")).toBe(false);
    expect(composerTransition(running, {
      type: "retry_requested", relativePath: "src/image.bin",
    })).toBe(running);
  });

  it("clears the row when the file attaches", () => {
    const state = composerReducer(withRefusals(), [
      { type: "retry_requested", relativePath: "src/locked.ts" },
      { type: "retry_settled", relativePath: "src/locked.ts", rejection: null },
    ]);
    expect(state.rejections.map((r) => r.relativePath)).toEqual(["src/image.bin", ""]);
    expect(state.retryingPath).toBeNull();
    expect(state.phase).toBe("drafting");
  });

  /// The reason this event exists at all.
  it("leaves every other file's refusal standing", () => {
    const state = composerReducer(withRefusals(), [
      { type: "retry_requested", relativePath: "src/locked.ts" },
      { type: "retry_settled", relativePath: "src/locked.ts", rejection: null },
    ]);
    expect(state.rejections).toHaveLength(2);
  });

  it("keeps the row when the file is still refused", () => {
    const state = composerReducer(withRefusals(), [
      { type: "retry_requested", relativePath: "src/locked.ts" },
      {
        type: "retry_settled",
        relativePath: "src/locked.ts",
        rejection: {
          relativePath: "src/locked.ts", reason: "Still could not be read.", retryable: true,
        },
      },
    ]);
    expect(state.rejections).toHaveLength(3);
    expect(canRetryAttachment(state, "src/locked.ts")).toBe(true);
  });

  it("ignores a settle for a file it is not retrying", () => {
    // A settle arriving for a superseded retry must not clear a row the user
    // is still looking at.
    const running = composerTransition(withRefusals(), {
      type: "retry_requested", relativePath: "src/locked.ts",
    });
    expect(composerTransition(running, {
      type: "retry_settled", relativePath: "src/image.bin", rejection: null,
    })).toBe(running);
  });

  it("keeps the draft through a retry", () => {
    const state = composerReducer(withRefusals(), [
      { type: "retry_requested", relativePath: "src/locked.ts" },
      { type: "retry_settled", relativePath: "src/locked.ts", rejection: null },
    ]);
    expect(state.draft).toBe("fix the login");
  });

  it("forgets an in-flight retry on reset", () => {
    const state = composerReducer(withRefusals(), [
      { type: "retry_requested", relativePath: "src/locked.ts" },
      { type: "reset" },
    ]);
    expect(state.retryingPath).toBeNull();
  });
});

describe("reset", () => {
  /// A task switch, or the host confirming the message was consumed.
  it("drops everything the previous task held", () => {
    const state = composerReducer(after(typed, openDialog, startImport), [
      { type: "import_settled", attached: 1, rejected: [{ relativePath: "a.bin", reason: "No." }] },
      { type: "reset" },
    ]);
    expect(state).toEqual(initialComposerState);
  });

  it("resets from any phase", () => {
    for (const state of [
      after(typed, send),
      after(typed, send, { type: "stream_started" }),
      after(typed, openDialog),
      after(typed, send, { type: "send_failed", reason: "no", retryable: true }),
    ]) {
      expect(composerTransition(state, { type: "reset" })).toEqual(initialComposerState);
    }
  });
});

describe("isBusy", () => {
  it("covers exactly the phases the user must not start work from twice", () => {
    expect(isBusy(after(typed, openDialog))).toBe(true);
    expect(isBusy(after(typed, send))).toBe(true);
    expect(isBusy(after(typed, send, { type: "stream_started" }))).toBe(true);
    expect(isBusy(after(typed))).toBe(false);
    // Importing is deliberately not busy: the draft stays usable.
    expect(isBusy(after(typed, openDialog, startImport))).toBe(false);
  });
});

describe("every event is safe in every phase", () => {
  /// A machine that throws on an unexpected event moves the problem to every
  /// call site. Ignoring is a returned state, and it must never be corrupt.
  it("never produces an invalid state, whatever the order", () => {
    const events: readonly ComposerEvent[] = [
      typed, openDialog, startImport,
      { type: "import_settled", attached: 1, rejected: [] },
      { type: "import_failed", reason: "x" },
      { type: "file_dialog_cancelled" },
      send, { type: "stream_started" }, { type: "stop_requested" },
      { type: "stream_finished" },
      { type: "send_failed", reason: "x", retryable: true },
      { type: "rejections_dismissed" }, { type: "reset" },
    ];
    const phases = new Set<string>();
    for (const a of events) {
      for (const b of events) {
        const state = composerReducer(initialComposerState, [a, b]);
        phases.add(state.phase);
        expect(typeof state.draft).toBe("string");
        expect(Array.isArray(state.rejections)).toBe(true);
      }
    }
    expect(phases.size).toBeGreaterThan(1);
  });
});
