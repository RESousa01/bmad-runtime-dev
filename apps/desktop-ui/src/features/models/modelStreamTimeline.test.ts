import { describe, expect, it } from "vitest";
import type { ModelStreamEventProjection } from "../../lib/hostClient";
import {
  applyModelStreamEvent,
  createModelStreamTimeline,
  type ModelStreamSubscriptionBinding,
} from "./modelStreamTimeline";

function subscription(
  overrides: Partial<ModelStreamSubscriptionBinding> = {},
): ModelStreamSubscriptionBinding {
  return {
    workspaceId: "workspace_1",
    workspaceGrantEpoch: 3,
    appEpoch: 5,
    taskId: "task_1",
    runId: "run_1",
    ...overrides,
  };
}

function event(
  sequence: number,
  body: ModelStreamEventProjection["body"],
  overrides: Partial<ModelStreamEventProjection> = {},
): ModelStreamEventProjection {
  return {
    schemaVersion: "sapphirus.model-stream-event.v1",
    workspaceId: "workspace_1",
    workspaceGrantEpoch: 3,
    appEpoch: 5,
    taskId: "task_1",
    runId: "run_1",
    sequence,
    occurredAt: 2_000 + sequence,
    body,
    ...overrides,
  };
}

describe("applyModelStreamEvent", () => {
  it("builds started, text, tool-proposal, usage, and completed entries in order", () => {
    let state = createModelStreamTimeline(subscription());
    state = applyModelStreamEvent(state, event(1, {
      kind: "started",
      resolvedProfileId: "balanced",
      resolvedVariantId: "standard",
    }));
    state = applyModelStreamEvent(state, event(2, {
      kind: "output_progress",
      outputIndex: 0,
      textDelta: "The answer",
      deltaByteCount: 10,
    }));
    state = applyModelStreamEvent(state, event(3, {
      kind: "output_progress",
      outputIndex: 0,
      textDelta: " is 42.",
      deltaByteCount: 7,
    }));
    state = applyModelStreamEvent(state, event(4, {
      kind: "tool_call_started",
      toolCallId: "call_1",
      toolName: "search_docs",
    }));
    state = applyModelStreamEvent(state, event(5, {
      kind: "tool_call_arguments",
      toolCallId: "call_1",
      argumentsDelta: '{"query":',
      deltaByteCount: 9,
    }));
    state = applyModelStreamEvent(state, event(6, {
      kind: "tool_call_arguments",
      toolCallId: "call_1",
      argumentsDelta: '"docs"}',
      deltaByteCount: 7,
    }));
    state = applyModelStreamEvent(state, event(7, {
      kind: "tool_call_completed",
      toolCallId: "call_1",
      argumentsByteCount: 16,
    }));
    state = applyModelStreamEvent(state, event(8, {
      kind: "terminal",
      terminal: "completed",
      receipt: {
        receiptId: "receipt_1",
        eventCount: 8,
        outputByteCount: 17,
        truncated: false,
        recordedAt: 2_100,
      },
      cancellationSource: null,
      reasonCode: null,
      retryable: false,
    }));

    expect(state.entries).toEqual([
      { kind: "started", resolvedProfileId: "balanced", resolvedVariantId: "standard" },
      { kind: "text", outputIndex: 0, text: "The answer is 42.", outputByteCount: 17 },
      {
        kind: "tool_proposal",
        proposal: {
          toolCallId: "call_1",
          toolName: "search_docs",
          argumentsText: '{"query":"docs"}',
          argumentsByteCount: 16,
          status: "proposed",
        },
      },
      {
        kind: "usage",
        receipt: {
          receiptId: "receipt_1",
          eventCount: 8,
          outputByteCount: 17,
          truncated: false,
          recordedAt: 2_100,
        },
      },
      { kind: "completed" },
    ]);
    expect(state.terminated).toBe(true);
  });

  it("refuses an event from another run without corrupting the timeline", () => {
    const initial = createModelStreamTimeline(subscription());
    const started = applyModelStreamEvent(initial, event(1, {
      kind: "started",
      resolvedProfileId: "balanced",
      resolvedVariantId: "standard",
    }));
    const wrongRun = applyModelStreamEvent(started, event(2, {
      kind: "output_progress",
      outputIndex: 0,
      textDelta: "leaked",
      deltaByteCount: 6,
    }, { runId: "run_2" }));
    expect(wrongRun.entries).toEqual(started.entries);
    expect(wrongRun.lastRejection).toBe("wrong_run");
    expect(wrongRun.lastSequence).toBe(started.lastSequence);
  });

  it("refuses a repeated or regressed sequence and keeps the stream open", () => {
    let state = createModelStreamTimeline(subscription());
    state = applyModelStreamEvent(state, event(4, {
      kind: "started",
      resolvedProfileId: "balanced",
      resolvedVariantId: "standard",
    }));
    const repeated = applyModelStreamEvent(state, event(4, {
      kind: "started",
      resolvedProfileId: "balanced",
      resolvedVariantId: "standard",
    }));
    expect(repeated.lastRejection).toBe("sequence_out_of_order");
    const resumed = applyModelStreamEvent(repeated, event(5, {
      kind: "output_progress",
      outputIndex: 0,
      textDelta: "ok",
      deltaByteCount: 2,
    }));
    expect(resumed.lastRejection).toBeNull();
    expect(resumed.entries).toHaveLength(2);
  });

  it("accepts nothing after a terminal event", () => {
    let state = createModelStreamTimeline(subscription());
    state = applyModelStreamEvent(state, event(1, {
      kind: "terminal",
      terminal: "cancelled",
      receipt: null,
      cancellationSource: "user",
      reasonCode: null,
      retryable: false,
    }));
    expect(state.terminated).toBe(true);
    const after = applyModelStreamEvent(state, event(2, {
      kind: "output_progress",
      outputIndex: 0,
      textDelta: "too late",
      deltaByteCount: 8,
    }));
    expect(after.lastRejection).toBe("already_terminal");
    expect(after.entries).toEqual(state.entries);
  });

  it("drops an arguments fragment for a call this timeline never saw start", () => {
    let state = createModelStreamTimeline(subscription());
    state = applyModelStreamEvent(state, event(1, {
      kind: "tool_call_arguments",
      toolCallId: "call_never_started",
      argumentsDelta: "{}",
      deltaByteCount: 2,
    }));
    expect(state.entries).toEqual([]);
    expect(state.lastSequence).toBe(1);
  });

  it("carries a refused, a retryable failed, and a cancelled terminal with their own evidence", () => {
    const refused = applyModelStreamEvent(createModelStreamTimeline(subscription()), event(1, {
      kind: "terminal",
      terminal: "refused",
      receipt: null,
      cancellationSource: null,
      reasonCode: "policy_blocked",
      retryable: false,
    }));
    expect(refused.entries).toEqual([{ kind: "refused", reasonCode: "policy_blocked" }]);

    const failed = applyModelStreamEvent(createModelStreamTimeline(subscription()), event(1, {
      kind: "terminal",
      terminal: "failed",
      receipt: null,
      cancellationSource: null,
      reasonCode: "provider_timeout",
      retryable: true,
    }));
    expect(failed.entries).toEqual([
      { kind: "failed", reasonCode: "provider_timeout", retryable: true },
    ]);

    const cancelled = applyModelStreamEvent(createModelStreamTimeline(subscription()), event(1, {
      kind: "terminal",
      terminal: "cancelled",
      receipt: null,
      cancellationSource: "host",
      reasonCode: null,
      retryable: false,
    }));
    expect(cancelled.entries).toEqual([{ kind: "cancelled", cancellationSource: "host" }]);
  });
});
