/**
 * What one task is doing, as a single value.
 *
 * FE-20. The task header derived this inline from four independent signals --
 * `chatLane.sending`, `chatLane.error`, the BMAD request state, and whether the
 * binding had dropped to read-only recovery -- and each control that wanted to
 * describe the task re-derived it in its own way. That is how a header comes to
 * say "Ready" beside a transcript showing a failure, and how a task waiting on
 * an approval reads as idle.
 *
 * The content of this module is not the list of statuses. It is the
 * **precedence**: several of these are true at once far more often than not,
 * and which one the user is shown decides whether they act. A task that has
 * failed *and* is awaiting review must read as awaiting review, because that is
 * the one with something for them to do.
 */

import type { BmadRequestState } from "../../lib/bmadModelProjection";
import type { ChatLaneState } from "../../state/chatSessions";

/**
 * The states a task can be in, most urgent first.
 *
 * Ordered deliberately: `taskStatus` returns the first that applies, so the
 * declaration order *is* the precedence and the two cannot drift apart.
 */
export type TaskStatusKind =
  /** A governed decision is waiting on the user. Nothing outranks this. */
  | "awaiting_review"
  /** The last operation failed and has not been superseded. */
  | "failed"
  /** The binding is in recovery: the task is visible but cannot be worked. */
  | "read_only"
  /** A request is with the host. */
  | "working"
  /** Nothing in flight. */
  | "idle";

/**
 * How loudly to draw it.
 *
 * Three levels, not five. A status dot that can be any of five colours is a
 * legend the user has to learn; the only distinctions that change behaviour are
 * "you must act", "something is wrong", and "carry on".
 */
export type TaskStatusTone = "action" | "warning" | "neutral";

export interface TaskStatus {
  readonly kind: TaskStatusKind;
  /** Shown in the header. Sentence case, no trailing period -- it is a label. */
  readonly label: string;
  readonly tone: TaskStatusTone;
  /**
   * Whether a screen reader should be interrupted.
   *
   * True only for the two states that are about the user rather than about the
   * app. Announcing every transition to "Working" turns the live region into
   * noise, and then the announcement that matters arrives inside it.
   */
  readonly announce: boolean;
}

export interface TaskStatusInput {
  readonly chatLane: ChatLaneState;
  readonly guidance: BmadRequestState;
  /** True when the workspace binding has dropped to read-only recovery. */
  readonly readOnlyRecovery: boolean;
  /** True while an automatic context selection is running for this task. */
  readonly selectingContext: boolean;
}

/** The guidance states that are waiting on a person, not on the host. */
function guidanceAwaitsUser(guidance: BmadRequestState): boolean {
  // `approved` is included on purpose. The host has approved and is waiting to
  // be told to submit -- from the user's side that is still their move, and
  // reading it as "working" would leave a run parked with nothing driving it.
  return guidance.kind === "review_required" || guidance.kind === "approved";
}

/** The guidance states where the host is doing something. */
function guidanceIsWorking(guidance: BmadRequestState): boolean {
  return guidance.kind === "creating"
    || guidance.kind === "approving"
    || guidance.kind === "submitting";
}

export function taskStatus({
  chatLane,
  guidance,
  readOnlyRecovery,
  selectingContext,
}: TaskStatusInput): TaskStatus {
  if (guidanceAwaitsUser(guidance)) {
    return {
      announce: true,
      kind: "awaiting_review",
      label: "Awaiting your review",
      tone: "action",
    };
  }
  /*
   * Failure outranks recovery, and both outrank work in flight.
   *
   * Recovery is a condition of the workspace; a failed send is something that
   * happened to *this* task, and it is the more specific claim. Showing
   * "Read only" over it would leave the user reading a generic condition while
   * the thing that just went wrong went unnamed.
   */
  if (chatLane.error !== null || guidance.kind === "unavailable") {
    return { announce: true, kind: "failed", label: "Failed", tone: "warning" };
  }
  if (readOnlyRecovery) {
    return { announce: false, kind: "read_only", label: "Read only", tone: "warning" };
  }
  if (chatLane.sending || guidanceIsWorking(guidance) || selectingContext) {
    return { announce: false, kind: "working", label: "Working", tone: "neutral" };
  }
  return { announce: false, kind: "idle", label: "Ready", tone: "neutral" };
}

/**
 * Whether the task has work the user could still lose.
 *
 * Used by the close and delete paths. A question standing with no answer is
 * unfinished whether or not anything is in flight -- that is precisely the
 * state a failed send leaves behind, and it is the one worth confirming before
 * throwing away.
 */
export function taskHasUnfinishedWork(input: TaskStatusInput): boolean {
  const status = taskStatus(input);
  return status.kind === "awaiting_review"
    || status.kind === "working"
    || input.chatLane.pending !== null;
}
