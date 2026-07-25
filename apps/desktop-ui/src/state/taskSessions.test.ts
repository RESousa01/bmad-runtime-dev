import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  type ActiveCapabilityRunState,
  type CapabilityRouting,
  MAX_LANE_SESSIONS,
  captureRouting,
  clearAllRuns,
  closeSession,
  emptyCapabilitySessionStore,
  generationForSession,
  openRun,
  routeRun,
  runForSession,
  setRun,
  useCapabilitySessions,
} from "./taskSessions";

function run(capabilityId: string, kind: "selecting" | "preparing" = "selecting"): ActiveCapabilityRunState {
  return { capabilityId, label: capabilityId, phase: { kind } };
}

function errorRun(current: ActiveCapabilityRunState | null): ActiveCapabilityRunState | null {
  return current === null ? null : { ...current, phase: { kind: "error", message: "boom" } };
}

describe("capability session store", () => {
  it("openRun advances the task generation and sets its run", () => {
    const opened = openRun(emptyCapabilitySessionStore, "task-a", run("cap"));
    expect(opened.routing).toEqual({ sessionId: "task-a", generation: 1 });
    expect(runForSession(opened.store, "task-a")).toEqual(run("cap"));
    expect(generationForSession(opened.store, "task-a")).toBe(1);
  });

  it("routes a late result to its owning task and leaves other tasks untouched", () => {
    // Start a run under task A, then the user switches to task B (which has its
    // own run). Task A's async completion must land on A, not the visible B.
    let store = emptyCapabilitySessionStore;
    const a = openRun(store, "task-a", run("cap-a"));
    store = a.store;
    store = openRun(store, "task-b", run("cap-b")).store;

    const routed = routeRun(store, a.routing, (current) =>
      current === null ? null : { ...current, phase: { kind: "preparing" } },
    );
    expect(routed.applied).toBe(true);
    store = routed.store;

    expect(runForSession(store, "task-a")?.phase.kind).toBe("preparing");
    expect(runForSession(store, "task-b")).toEqual(run("cap-b"));
  });

  it("drops a result whose task was superseded by a newer run", () => {
    let store = emptyCapabilitySessionStore;
    const stale = openRun(store, "task-a", run("cap-a"));
    store = stale.store;
    // A newer run starts on the same task before the first completes.
    store = openRun(store, "task-a", run("cap-a2")).store;

    const routed = routeRun(store, stale.routing, errorRun);
    expect(routed.applied).toBe(false);
    expect(runForSession(routed.store, "task-a")).toEqual(run("cap-a2"));
  });

  it("drops a result whose task was closed", () => {
    let store = emptyCapabilitySessionStore;
    const token = openRun(store, "task-a", run("cap-a"));
    store = closeSession(token.store, "task-a");

    const routed = routeRun(store, token.routing, errorRun);
    expect(routed.applied).toBe(false);
    expect(runForSession(routed.store, "task-a")).toBeNull();
  });

  it("clearAllRuns drops every in-flight result and empties the runs", () => {
    let store = emptyCapabilitySessionStore;
    const a = openRun(store, "task-a", run("cap-a"));
    store = a.store;
    const b = openRun(store, "task-b", run("cap-b"));
    store = b.store;

    store = clearAllRuns(store);
    expect(runForSession(store, "task-a")).toBeNull();
    expect(runForSession(store, "task-b")).toBeNull();
    expect(routeRun(store, a.routing, errorRun).applied).toBe(false);
    expect(routeRun(store, b.routing, errorRun).applied).toBe(false);
  });

  it("caps retained sessions, evicting the oldest but never the newest", () => {
    let store = emptyCapabilitySessionStore;
    for (let i = 0; i < MAX_LANE_SESSIONS + 5; i += 1) {
      store = openRun(store, `task-${i}`, run(`cap-${i}`)).store;
    }
    expect(store.runs.size).toBe(MAX_LANE_SESSIONS);
    // The five oldest were evicted; the newest is retained.
    expect(runForSession(store, "task-0")).toBeNull();
    expect(runForSession(store, "task-4")).toBeNull();
    expect(runForSession(store, `task-${MAX_LANE_SESSIONS + 4}`)?.capabilityId).toBe(
      `cap-${MAX_LANE_SESSIONS + 4}`,
    );
  });

  it("setRun updates in place without advancing the generation", () => {
    const opened = openRun(emptyCapabilitySessionStore, "task-a", run("cap"));
    const captured = captureRouting(opened.store, "task-a");
    const updated = setRun(opened.store, "task-a", (current) =>
      current === null ? null : { ...current, phase: { kind: "preparing" } },
    );
    expect(runForSession(updated, "task-a")?.phase.kind).toBe("preparing");
    // Same generation: an in-flight request captured before still routes.
    expect(routeRun(updated, captured, errorRun).applied).toBe(true);
  });
});

describe("useCapabilitySessions", () => {
  it("routes a completion to the origin task even after the active tab changed", () => {
    // Mirrors the App wiring: capture a routing token while task A is active,
    // switch tabs to B, and let A's async completion arrive. It must land on A
    // (visible only when A is active again), never on the now-visible B.
    const { result, rerender } = renderHook(
      ({ active }: { active: string }) => useCapabilitySessions(active),
      { initialProps: { active: "task-a" } },
    );

    let captured!: CapabilityRouting;
    act(() => {
      result.current.openActiveCapabilityRun(run("cap-a"));
      captured = result.current.captureActive();
    });
    expect(result.current.capabilityRun?.capabilityId).toBe("cap-a");

    // The user switches to a second task with no run of its own.
    rerender({ active: "task-b" });
    expect(result.current.capabilityRun).toBeNull();

    // Task A's async completion resolves while task B is on screen.
    let applied = false;
    act(() => {
      applied = result.current.routeCapabilityRun(captured, (current) =>
        current === null ? null : { ...current, phase: { kind: "preparing" } },
      );
    });
    expect(applied).toBe(true);
    // Task B is untouched by task A's completion.
    expect(result.current.capabilityRun).toBeNull();

    // Returning to task A reveals its routed completion.
    rerender({ active: "task-a" });
    expect(result.current.capabilityRun?.phase.kind).toBe("preparing");
  });

  it("drops a completion whose origin task was superseded by a new run", () => {
    const { result } = renderHook(() => useCapabilitySessions("task-a"));

    let captured!: CapabilityRouting;
    act(() => {
      result.current.openActiveCapabilityRun(run("cap-a"));
      captured = result.current.captureActive();
      // A new run supersedes the first before the first completes.
      result.current.openActiveCapabilityRun(run("cap-a2"));
    });

    let applied = true;
    act(() => {
      applied = result.current.routeCapabilityRun(captured, errorRun);
    });
    expect(applied).toBe(false);
    expect(result.current.capabilityRun?.capabilityId).toBe("cap-a2");
    expect(result.current.capabilityRun?.phase.kind).toBe("selecting");
  });
});
