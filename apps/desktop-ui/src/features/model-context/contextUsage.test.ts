import { describe, expect, it } from "vitest";

import { projectContextUsage } from "./contextUsage";
import {
  measure,
  reconcile,
  unmeasuredModelContext,
  type ModelContextKey,
  type ModelContextSnapshot,
} from "./modelContextTypes";

function key(overrides: Partial<ModelContextKey> = {}): ModelContextKey {
  return {
    taskId: "a",
    workspaceId: "w1",
    workspaceGrantEpoch: 1,
    modelRoute: "openai/gpt-5",
    promptEpoch: 1,
    ...overrides,
  };
}

function snapshot(overrides: Partial<ModelContextSnapshot> = {}): ModelContextSnapshot {
  return {
    key: key(),
    inputTokens: 12_000,
    windowTokens: 200_000,
    measuredAt: 1_700_000_000_000,
    ...overrides,
  };
}

describe("projectContextUsage", () => {
  /// FE-16: the budget must come from the model, and the host reports no model
  /// window today. Nothing derived from `workspaceReadLimits` may reach here.
  it("calls the window unavailable until a measurement carries one", () => {
    const usage = projectContextUsage(unmeasuredModelContext, 2_000);

    expect(usage.window.kind).toBe("unavailable");
    expect(usage.usedTokens).toBe(2_000);
    expect(usage.basis).toBe("estimated");
    expect(usage.stale).toBe(false);
  });

  it("labels an attachment total as an estimate, never as reported", () => {
    expect(projectContextUsage(unmeasuredModelContext, 0).basis).toBe("estimated");
    expect(projectContextUsage(unmeasuredModelContext, 9_999).basis).toBe("estimated");
  });

  it("reports the host's measurement and the window it was measured against", () => {
    const usage = projectContextUsage(measure(snapshot()), 2_000);

    expect(usage.basis).toBe("reported");
    expect(usage.usedTokens).toBe(12_000);
    expect(usage.window).toEqual({
      fraction: 0.06,
      kind: "known",
      windowTokens: 200_000,
    });
  });

  /// The attachments are already inside what the host assembled. Adding the
  /// local estimate on top would count the same files twice.
  it("supersedes the attachment estimate rather than adding to it", () => {
    const usage = projectContextUsage(measure(snapshot({ inputTokens: 500 })), 2_000);

    expect(usage.usedTokens).toBe(500);
  });

  it("keeps the window unavailable when the measurement did not carry one", () => {
    const usage = projectContextUsage(measure(snapshot({ windowTokens: null })), 2_000);

    expect(usage.window.kind).toBe("unavailable");
    // Still the host's number. An absent window makes the denominator unknown,
    // not the numerator.
    expect(usage.basis).toBe("reported");
    expect(usage.usedTokens).toBe(12_000);
  });

  /// A window of zero is a denominator that cannot be divided by, and one of
  /// -1 is a sentinel some provider will eventually send. Neither is a budget.
  it("refuses a non-positive window as a budget", () => {
    expect(projectContextUsage(measure(snapshot({ windowTokens: 0 })), 0).window.kind)
      .toBe("unavailable");
    expect(projectContextUsage(measure(snapshot({ windowTokens: -1 })), 0).window.kind)
      .toBe("unavailable");
  });

  it("does not clamp usage that exceeded the window", () => {
    const usage = projectContextUsage(
      measure(snapshot({ inputTokens: 250_000 })),
      0,
    );

    expect(usage.usedTokens).toBe(250_000);
    expect(usage.window).toEqual({
      fraction: 1.25,
      kind: "known",
      windowTokens: 200_000,
    });
  });

  /// A stale measurement keeps its numbers -- a figure with a stated age beats
  /// a blank panel -- but must never read as current.
  it("marks a measurement stale once its key has moved", () => {
    const measured = measure(snapshot());
    const rerouted = reconcile(measured, key({ modelRoute: "azure/gpt-5" }));

    expect(projectContextUsage(rerouted, 2_000)).toEqual({
      basis: "reported",
      stale: true,
      usedTokens: 12_000,
      window: { fraction: 0.06, kind: "known", windowTokens: 200_000 },
    });
    expect(projectContextUsage(measured, 2_000).stale).toBe(false);
  });
});
