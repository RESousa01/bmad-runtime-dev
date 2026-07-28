import { describe, expect, it } from "vitest";

import { emptyChatLane } from "../../state/chatSessions";
import { initialBmadRequestState } from "../../lib/bmadModelProjection";
import type { BmadRequestState } from "../../lib/bmadModelProjection";
import type { ChatLaneState } from "../../state/chatSessions";
import { taskHasUnfinishedWork, taskStatus, type TaskStatusInput } from "./taskStatus";

function input(overrides: Partial<TaskStatusInput> = {}): TaskStatusInput {
  return {
    chatLane: emptyChatLane,
    guidance: initialBmadRequestState,
    readOnlyRecovery: false,
    selectingContext: false,
    ...overrides,
  };
}

function lane(overrides: Partial<ChatLaneState> = {}): ChatLaneState {
  return { ...emptyChatLane, ...overrides };
}

/// Enough of a review-required state to be recognised. The projections it
/// carries are not read here -- the status depends on the kind alone, which is
/// the point of testing it separately from the panels that render the rest.
const reviewRequired = {
  kind: "review_required",
  run: { runId: "run-1", sessionId: "session-1" },
  runProjection: null,
  review: {},
  authority: {},
} as unknown as BmadRequestState;

describe("the ordinary states", () => {
  it("reads as ready when nothing is happening", () => {
    const status = taskStatus(input());
    expect(status.kind).toBe("idle");
    expect(status.label).toBe("Ready");
    expect(status.announce).toBe(false);
  });

  it("reads as working while a send is in flight", () => {
    expect(taskStatus(input({ chatLane: lane({ sending: true }) })).kind).toBe("working");
  });

  it("reads as working while context is being selected", () => {
    // The user pressed something and the app is doing it. Reading as "Ready"
    // while a selection runs makes the button look like it did nothing.
    expect(taskStatus(input({ selectingContext: true })).kind).toBe("working");
  });

  it("does not interrupt a screen reader to say it is working", () => {
    // Announcing every transition turns the live region into noise, and then
    // the announcement that matters arrives inside it.
    expect(taskStatus(input({ chatLane: lane({ sending: true }) })).announce).toBe(false);
  });
});

describe("precedence", () => {
  /// The whole point of the module. These combinations are the common case,
  /// not the edge case, and which one is shown decides whether the user acts.
  it("shows a review over a failure", () => {
    const status = taskStatus(input({
      chatLane: lane({ error: "The provider rate-limited this request." }),
      guidance: reviewRequired,
    }));
    // A failed send is worth knowing. A decision waiting on the user is worth
    // interrupting them for, and only one of the two can occupy the header.
    expect(status.kind).toBe("awaiting_review");
  });

  it("shows a review over work in flight", () => {
    const status = taskStatus(input({
      chatLane: lane({ sending: true }),
      guidance: reviewRequired,
    }));
    expect(status.kind).toBe("awaiting_review");
  });

  it("shows a review over read-only recovery", () => {
    const status = taskStatus(input({ guidance: reviewRequired, readOnlyRecovery: true }));
    expect(status.kind).toBe("awaiting_review");
  });

  /// Recovery is a condition of the workspace; a failed send is something that
  /// happened to this task. The specific claim wins.
  it("shows a failure over read-only recovery", () => {
    const status = taskStatus(input({
      chatLane: lane({ error: "That did not go through." }),
      readOnlyRecovery: true,
    }));
    expect(status.kind).toBe("failed");
  });

  it("shows read-only recovery over work in flight", () => {
    const status = taskStatus(input({
      chatLane: lane({ sending: true }),
      readOnlyRecovery: true,
    }));
    expect(status.kind).toBe("read_only");
  });

  it("interrupts only for the two states that are about the user", () => {
    const announced = [
      taskStatus(input({ guidance: reviewRequired })),
      taskStatus(input({ chatLane: lane({ error: "no" }) })),
    ];
    const silent = [
      taskStatus(input()),
      taskStatus(input({ chatLane: lane({ sending: true }) })),
      taskStatus(input({ readOnlyRecovery: true })),
    ];
    expect(announced.every((status) => status.announce)).toBe(true);
    expect(silent.some((status) => status.announce)).toBe(false);
  });
});

describe("guidance states", () => {
  const kinds: readonly [string, string][] = [
    ["creating", "working"],
    ["approving", "working"],
    ["submitting", "working"],
    ["review_required", "awaiting_review"],
    ["approved", "awaiting_review"],
    ["unavailable", "failed"],
  ];

  for (const [kind, expected] of kinds) {
    it(`maps ${kind} to ${expected}`, () => {
      const guidance = { ...reviewRequired, kind } as unknown as BmadRequestState;
      expect(taskStatus(input({ guidance })).kind).toBe(expected);
    });
  }

  /// `approved` is the host saying yes and waiting to be told to submit. From
  /// the user's side that is still their move, and calling it "working" would
  /// leave a run parked with nothing driving it and no sign that it is.
  it("treats an approved run as the user's move, not the host's", () => {
    const guidance = { ...reviewRequired, kind: "approved" } as unknown as BmadRequestState;
    expect(taskStatus(input({ guidance })).kind).toBe("awaiting_review");
  });
});

describe("taskHasUnfinishedWork", () => {
  it("is false for a task that has settled", () => {
    expect(taskHasUnfinishedWork(input())).toBe(false);
  });

  it("is true while something is in flight", () => {
    expect(taskHasUnfinishedWork(input({ chatLane: lane({ sending: true }) }))).toBe(true);
  });

  it("is true while a decision is waiting", () => {
    expect(taskHasUnfinishedWork(input({ guidance: reviewRequired }))).toBe(true);
  });

  /// The state a failed send leaves behind: nothing is in flight, the status
  /// reads "Failed", and the question the user wrote is still standing with no
  /// answer. Closing that without a word discards their message.
  it("is true for a question left standing by a failed send", () => {
    const chatLane = lane({ error: "That did not go through.", pending: "explain this" });
    expect(taskStatus(input({ chatLane })).kind).toBe("failed");
    expect(taskHasUnfinishedWork(input({ chatLane }))).toBe(true);
  });

  it("is false once the answer has arrived", () => {
    expect(taskHasUnfinishedWork(input({ chatLane: lane({ pending: null }) }))).toBe(false);
  });
});
