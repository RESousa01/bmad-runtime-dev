import { describe, expect, it } from "vitest";
import {
  initialModelResponseLifecycleState,
  modelResponseIsRetryable,
  transitionModelResponseLifecycle,
} from "./modelResponseLifecycle";
import { createModelStreamTimeline } from "./modelStreamTimeline";

const subscription = {
  workspaceId: "workspace_1",
  workspaceGrantEpoch: 3,
  appEpoch: 5,
  taskId: "task_1",
  runId: "run_1",
};

describe("transitionModelResponseLifecycle", () => {
  it("treats FeatureDisabled as a first-class terminal state, not a crash", () => {
    const sending = transitionModelResponseLifecycle(initialModelResponseLifecycleState, {
      type: "send_started",
    });
    expect(sending).toEqual({ kind: "sending" });
    const disabled = transitionModelResponseLifecycle(sending, {
      type: "feature_disabled",
      message: "Connected model responses are not enabled in this build.",
    });
    expect(disabled).toEqual({
      kind: "feature_disabled",
      message: "Connected model responses are not enabled in this build.",
    });
    expect(modelResponseIsRetryable(disabled)).toBe(false);
  });

  it("binds a stream timeline once the response actually started", () => {
    const sending = transitionModelResponseLifecycle(initialModelResponseLifecycleState, {
      type: "send_started",
    });
    const started = {
      taskId: "task_1",
      runId: "run_1",
      appEpoch: 5,
      profileId: "balanced",
      variantId: "standard",
      catalogEpoch: 1,
      startedAt: 1_000,
    };
    const active = transitionModelResponseLifecycle(sending, {
      type: "send_outcome",
      outcome: { kind: "started", started },
      makeTimeline: () => createModelStreamTimeline(subscription),
    });
    expect(active.kind).toBe("active");
    expect(active.kind === "active" && active.started).toEqual(started);
  });

  it("surfaces requires_explicit_choice without inventing a replacement model", () => {
    const sending = transitionModelResponseLifecycle(initialModelResponseLifecycleState, {
      type: "send_started",
    });
    const selection = {
      schemaVersion: "sapphirus.model-selection-projection.v1",
      workspaceId: "workspace_1",
      workspaceGrantEpoch: 3,
      appEpoch: 5,
      catalogEpoch: 1,
      catalogVersion: "v1.abc",
      status: "requires_explicit_choice",
      previousProfileId: "balanced",
      previousVariantId: "standard",
      blocked: "profile_removed",
    } as const;
    const result = transitionModelResponseLifecycle(sending, {
      type: "send_outcome",
      outcome: { kind: "selection", selection },
      makeTimeline: () => createModelStreamTimeline(subscription),
    });
    expect(result).toEqual({ kind: "requires_explicit_choice", selection });
  });

  it("distinguishes a retryable provider failure from a non-retryable one", () => {
    const sending = transitionModelResponseLifecycle(initialModelResponseLifecycleState, {
      type: "send_started",
    });
    const retryable = transitionModelResponseLifecycle(sending, {
      type: "send_failed",
      message: "The provider timed out.",
      retryable: true,
    });
    expect(modelResponseIsRetryable(retryable)).toBe(true);

    const uncertain = transitionModelResponseLifecycle(sending, {
      type: "send_failed",
      message: "The request outcome could not be confirmed.",
      retryable: false,
    });
    expect(modelResponseIsRetryable(uncertain)).toBe(false);
  });

  it("returns to idle once a run stops", () => {
    const active = {
      kind: "active" as const,
      started: {
        taskId: "task_1",
        runId: "run_1",
        appEpoch: 5,
        profileId: "balanced",
        variantId: "standard",
        catalogEpoch: 1,
        startedAt: 1_000,
      },
      timeline: createModelStreamTimeline(subscription),
    };
    expect(transitionModelResponseLifecycle(active, { type: "stopped" })).toEqual({ kind: "idle" });
  });
});
