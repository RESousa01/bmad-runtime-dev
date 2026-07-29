/**
 * FND-053: the task timeline built from normalized model stream events.
 *
 * A pure reducer over `ModelStreamEventProjection`, deliberately fed events
 * directly rather than driven by a live subscription -- `send_model_response`
 * in this build always returns `FeatureDisabled` (the HTTPS transport is
 * FND-045's job, which runs after this one), so there is no live call to
 * drive one with. Every rule below is instead proven against fabricated
 * envelopes, exactly the way `ModelStreamPublisher`'s own Rust tests prove the
 * projection layer.
 *
 * Ownership is re-checked on every event, mirroring
 * `ModelStreamSubscription::authorize` on the host: a subscriber is bound to
 * one workspace grant, one app epoch, one task and one run, and refuses
 * anything that disagrees with any of them, out-of-order sequences included.
 * This is defence in depth -- the host already refuses to *emit* a
 * mismatched envelope -- but a renderer that trusted transport framing to
 * enforce that would have no test of its own for the day the assumption
 * breaks.
 */

import type {
  ModelReceiptSummaryProjection,
  ModelStreamCancellationSource,
  ModelStreamEventProjection,
} from "../../lib/hostClient";

export interface ModelStreamSubscriptionBinding {
  readonly workspaceId: string;
  readonly workspaceGrantEpoch: number;
  readonly appEpoch: number;
  readonly taskId: string;
  readonly runId: string;
}

export interface ModelTimelineToolProposal {
  readonly toolCallId: string;
  readonly toolName: string;
  /** The reviewable typed proposal taking shape, not an authority to act --
   * see `ModelStreamEventInput::ToolCallArguments` on the host. */
  readonly argumentsText: string;
  readonly argumentsByteCount: number;
  readonly status: "assembling" | "proposed";
}

export type ModelTimelineEntry =
  | { readonly kind: "started"; readonly resolvedProfileId: string; readonly resolvedVariantId: string }
  | { readonly kind: "text"; readonly outputIndex: number; readonly text: string; readonly outputByteCount: number }
  | { readonly kind: "tool_proposal"; readonly proposal: ModelTimelineToolProposal }
  | { readonly kind: "usage"; readonly receipt: ModelReceiptSummaryProjection }
  | { readonly kind: "completed" }
  | { readonly kind: "refused"; readonly reasonCode: string }
  | { readonly kind: "failed"; readonly reasonCode: string | null; readonly retryable: boolean }
  | { readonly kind: "cancelled"; readonly cancellationSource: ModelStreamCancellationSource };

export type ModelStreamRejection =
  | "wrong_run"
  | "wrong_task"
  | "wrong_workspace"
  | "stale_app_epoch"
  | "sequence_out_of_order"
  | "already_terminal";

export interface ModelStreamTimelineState {
  readonly subscription: ModelStreamSubscriptionBinding;
  readonly entries: readonly ModelTimelineEntry[];
  readonly lastSequence: number | null;
  readonly terminated: boolean;
  /** The most recent event this timeline refused, if any. Kept for the UI to
   * surface a diagnostic; it never blocks later, valid events. */
  readonly lastRejection: ModelStreamRejection | null;
}

export function createModelStreamTimeline(
  subscription: ModelStreamSubscriptionBinding,
): ModelStreamTimelineState {
  return { subscription, entries: [], lastSequence: null, terminated: false, lastRejection: null };
}

function authorize(
  subscription: ModelStreamSubscriptionBinding,
  event: ModelStreamEventProjection,
): ModelStreamRejection | null {
  if (
    subscription.workspaceId !== event.workspaceId
    || subscription.workspaceGrantEpoch !== event.workspaceGrantEpoch
  ) {
    return "wrong_workspace";
  }
  if (subscription.appEpoch !== event.appEpoch) return "stale_app_epoch";
  if (subscription.taskId !== event.taskId) return "wrong_task";
  if (subscription.runId !== event.runId) return "wrong_run";
  return null;
}

function appendText(
  entries: readonly ModelTimelineEntry[],
  outputIndex: number,
  textDelta: string,
  deltaByteCount: number,
): readonly ModelTimelineEntry[] {
  const lastIndex = entries.length - 1;
  const last = entries[lastIndex];
  if (last !== undefined && last.kind === "text" && last.outputIndex === outputIndex) {
    const updated: ModelTimelineEntry = {
      kind: "text",
      outputIndex,
      text: last.text + textDelta,
      outputByteCount: last.outputByteCount + deltaByteCount,
    };
    return [...entries.slice(0, lastIndex), updated];
  }
  return [...entries, { kind: "text", outputIndex, text: textDelta, outputByteCount: deltaByteCount }];
}

function withToolProposal(
  entries: readonly ModelTimelineEntry[],
  toolCallId: string,
  update: (proposal: ModelTimelineToolProposal) => ModelTimelineToolProposal,
): readonly ModelTimelineEntry[] | null {
  const index = entries.findIndex(
    (entry) => entry.kind === "tool_proposal" && entry.proposal.toolCallId === toolCallId,
  );
  if (index === -1) return null;
  const entry = entries[index];
  if (entry === undefined || entry.kind !== "tool_proposal") return null;
  const updated: ModelTimelineEntry = { kind: "tool_proposal", proposal: update(entry.proposal) };
  return [...entries.slice(0, index), updated, ...entries.slice(index + 1)];
}

/**
 * Applies one authorized, in-order stream event to a timeline.
 *
 * An event that fails authorization, repeats or regresses the sequence, or
 * arrives after a terminal event is not an error to throw -- it is recorded
 * as `lastRejection` and the timeline is otherwise returned unchanged, the
 * same "refuse without corrupting state" shape `ModelStreamPublisher::publish`
 * uses on the host.
 */
export function applyModelStreamEvent(
  state: ModelStreamTimelineState,
  event: ModelStreamEventProjection,
): ModelStreamTimelineState {
  const rejection = authorize(state.subscription, event);
  if (rejection !== null) {
    return { ...state, lastRejection: rejection };
  }
  if (state.terminated) {
    return { ...state, lastRejection: "already_terminal" };
  }
  if (state.lastSequence !== null && event.sequence <= state.lastSequence) {
    return { ...state, lastRejection: "sequence_out_of_order" };
  }

  const { body } = event;
  let entries: readonly ModelTimelineEntry[] = state.entries;
  let terminated = false;

  switch (body.kind) {
    case "started":
      entries = [
        ...entries,
        {
          kind: "started",
          resolvedProfileId: body.resolvedProfileId,
          resolvedVariantId: body.resolvedVariantId,
        },
      ];
      break;
    case "output_progress":
      entries = appendText(entries, body.outputIndex, body.textDelta, body.deltaByteCount);
      break;
    case "tool_call_started":
      entries = [
        ...entries,
        {
          kind: "tool_proposal",
          proposal: {
            toolCallId: body.toolCallId,
            toolName: body.toolName,
            argumentsText: "",
            argumentsByteCount: 0,
            status: "assembling",
          },
        },
      ];
      break;
    case "tool_call_arguments": {
      const updated = withToolProposal(entries, body.toolCallId, (proposal) => ({
        ...proposal,
        argumentsText: proposal.argumentsText + body.argumentsDelta,
        argumentsByteCount: proposal.argumentsByteCount + body.deltaByteCount,
      }));
      // A fragment for a call this timeline never saw start is not
      // representable -- there is nowhere in the shape to attach it -- so it
      // is dropped rather than starting a phantom proposal with no name.
      entries = updated ?? entries;
      break;
    }
    case "tool_call_completed": {
      const updated = withToolProposal(entries, body.toolCallId, (proposal) => ({
        ...proposal,
        // The completed byte count is authoritative over the sum of deltas
        // this timeline happened to see -- it is what the support plane
        // validated the arguments against, not a client-side estimate.
        argumentsByteCount: body.argumentsByteCount,
        status: "proposed",
      }));
      entries = updated ?? entries;
      break;
    }
    case "terminal": {
      terminated = true;
      if (body.receipt !== null) {
        entries = [...entries, { kind: "usage", receipt: body.receipt }];
      }
      switch (body.terminal) {
        case "completed":
          entries = [...entries, { kind: "completed" }];
          break;
        case "refused":
          entries = [...entries, { kind: "refused", reasonCode: body.reasonCode }];
          break;
        case "failed":
          entries = [...entries, { kind: "failed", reasonCode: body.reasonCode, retryable: body.retryable }];
          break;
        case "cancelled":
          entries = [...entries, { kind: "cancelled", cancellationSource: body.cancellationSource }];
          break;
      }
      break;
    }
  }

  return {
    subscription: state.subscription,
    entries,
    lastSequence: event.sequence,
    terminated,
    lastRejection: null,
  };
}
