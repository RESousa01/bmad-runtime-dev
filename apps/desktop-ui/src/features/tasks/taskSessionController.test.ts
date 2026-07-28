import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { adoptPreview, emptyTaskAttachments } from "../attachments/attachmentTypes";
import { measure, type ModelContextKey } from "../model-context/modelContextTypes";
import type { ContextPreviewProjection } from "../../lib/hostClient";
import {
  captureRouting,
  clearAllSessions,
  closeSession,
  emptyTaskSessionStore,
  reconcileModelContext,
  routeUpdate,
  routingIsCurrent,
  sessionFor,
  supersede,
  updateSession,
  useTaskSessions,
} from "./taskSessionController";

function preview(paths: readonly string[], hash = "m1"): ContextPreviewProjection {
  return {
    workspaceId: "w1",
    manifestHash: hash,
    totalBytes: 0,
    estimatedTokens: 0,
    modelTarget: null,
    items: paths.map((relativePath) => ({
      relativePath,
      startLine: 1,
      endLine: 10,
      reason: "Selected for this task",
      contentHash: `hash-${relativePath}`,
      classification: "source",
      redactions: [],
      byteCount: 100,
      estimatedTokens: 25,
      content: "",
    })),
  } as ContextPreviewProjection;
}

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

function snapshot(k: ModelContextKey, inputTokens = 1_000) {
  return { key: k, inputTokens, windowTokens: 100_000, measuredAt: 0 };
}

describe("task isolation", () => {
  /// FE-13 acceptance: switching tasks never changes another task's draft,
  /// attachments, selected route, model-context snapshot, or run state.
  it("keeps every field separate across two tasks", () => {
    let store = emptyTaskSessionStore;
    store = updateSession(store, "a", (s) => ({
      ...s,
      attachments: adoptPreview(preview(["src/a.ts"]), "manual"),
      draft: "draft for a",
      modelContext: measure(snapshot(key({ taskId: "a" }), 500)),
    }));
    store = updateSession(store, "b", (s) => ({
      ...s,
      attachments: adoptPreview(preview(["src/b.ts", "src/c.ts"]), "automatic"),
      draft: "draft for b",
      modelContext: measure(snapshot(key({ taskId: "b" }), 9_000)),
    }));

    const a = sessionFor(store, "a");
    const b = sessionFor(store, "b");
    expect(a.draft).toBe("draft for a");
    expect(b.draft).toBe("draft for b");
    expect(a.attachments.descriptors).toHaveLength(1);
    expect(b.attachments.descriptors).toHaveLength(2);
    expect(a.attachments.selectionMode).toBe("manual");
    expect(b.attachments.selectionMode).toBe("automatic");
  });

  /// The defect this replaces: attachments were app-global, so a task switch
  /// destroyed them. `clearContext()` on every switch was the only safe thing
  /// to do with state that belonged to nobody.
  it("leaves a task's attachments alone while another task attaches", () => {
    let store = updateSession(emptyTaskSessionStore, "a", (s) => ({
      ...s,
      attachments: adoptPreview(preview(["src/a.ts"]), "manual"),
    }));
    store = updateSession(store, "b", (s) => ({
      ...s,
      attachments: adoptPreview(preview(["src/b.ts"]), "manual"),
    }));
    expect(sessionFor(store, "a").attachments.descriptors[0]?.relativePath).toBe("src/a.ts");
  });

  it("returns an empty session for a task that has never been touched", () => {
    expect(sessionFor(emptyTaskSessionStore, "never")).toEqual({
      attachments: emptyTaskAttachments,
      draft: "",
      modelContext: { kind: "unmeasured" },
    });
  });
});

describe("routing a late result", () => {
  /// An auto-selection is a host round trip that outlives the user's
  /// attention. It must land on the task that asked or nowhere.
  it("applies a result to the task that requested it", () => {
    const store = updateSession(emptyTaskSessionStore, "a", (s) => s);
    const routing = captureRouting(store, "a");
    const next = routeUpdate(store, routing, (s) => ({ ...s, draft: "applied" }));
    expect(sessionFor(next, "a").draft).toBe("applied");
  });

  it("drops a result whose task was superseded", () => {
    let store = updateSession(emptyTaskSessionStore, "a", (s) => s);
    const routing = captureRouting(store, "a");
    store = supersede(store, "a");
    const next = routeUpdate(store, routing, (s) => ({ ...s, draft: "stale" }));
    expect(sessionFor(next, "a").draft).toBe("");
  });

  it("drops a result whose task was closed", () => {
    let store = updateSession(emptyTaskSessionStore, "a", (s) => s);
    const routing = captureRouting(store, "a");
    store = closeSession(store, "a");
    expect(routingIsCurrent(store, routing)).toBe(false);
  });

  it("keeps the generation after a close, so an old token stays stale", () => {
    // Deleting the counter would restart it at zero and make a token captured
    // before the close read as current again.
    let store = updateSession(emptyTaskSessionStore, "a", (s) => s);
    const routing = captureRouting(store, "a");
    store = closeSession(store, "a");
    store = updateSession(store, "a", (s) => s);
    expect(routingIsCurrent(store, routing)).toBe(false);
  });

  it("supersedes without disturbing what the task holds", () => {
    let store = updateSession(emptyTaskSessionStore, "a", (s) => ({ ...s, draft: "keep me" }));
    store = supersede(store, "a");
    expect(sessionFor(store, "a").draft).toBe("keep me");
  });
});

describe("closing and clearing", () => {
  it("drops a closed task's unsent attachments", () => {
    // They were selected against a workspace grant for a message that was
    // never sent; re-arming them on reopen would attach files the user last
    // thought about in a different context.
    let store = updateSession(emptyTaskSessionStore, "a", (s) => ({
      ...s,
      attachments: adoptPreview(preview(["src/a.ts"]), "manual"),
      draft: "unsent",
    }));
    store = closeSession(store, "a");
    expect(sessionFor(store, "a")).toEqual({
      attachments: emptyTaskAttachments,
      draft: "",
      modelContext: { kind: "unmeasured" },
    });
  });

  it("leaves other tasks alone when one closes", () => {
    let store = updateSession(emptyTaskSessionStore, "a", (s) => ({ ...s, draft: "a" }));
    store = updateSession(store, "b", (s) => ({ ...s, draft: "b" }));
    store = closeSession(store, "a");
    expect(sessionFor(store, "b").draft).toBe("b");
  });

  it("drops everything on a workspace change", () => {
    // Attachments resolve against a workspace grant, so none of them survive.
    let store = updateSession(emptyTaskSessionStore, "a", (s) => ({ ...s, draft: "a" }));
    store = updateSession(store, "b", (s) => ({ ...s, draft: "b" }));
    store = clearAllSessions(store);
    expect(store.sessions.size).toBe(0);
  });

  it("supersedes in-flight work when everything is cleared", () => {
    const store = updateSession(emptyTaskSessionStore, "a", (s) => s);
    const routing = captureRouting(store, "a");
    expect(routingIsCurrent(clearAllSessions(store), routing)).toBe(false);
  });
});

describe("model-context reconciliation", () => {
  /// A snapshot measures one request under one authority. Showing a stale
  /// figure is worse than showing none, because the user budgets against it.
  it("keeps a snapshot whose key is unchanged", () => {
    const store = updateSession(emptyTaskSessionStore, "a", (s) => ({
      ...s,
      modelContext: measure(snapshot(key())),
    }));
    const next = reconcileModelContext(store, () => key());
    expect(sessionFor(next, "a").modelContext.kind).toBe("measured");
  });

  it("marks a snapshot stale when the model route changes", () => {
    const store = updateSession(emptyTaskSessionStore, "a", (s) => ({
      ...s,
      modelContext: measure(snapshot(key())),
    }));
    const next = reconcileModelContext(store, () => key({ modelRoute: "azure/gpt-5" }));
    expect(sessionFor(next, "a").modelContext.kind).toBe("stale");
  });

  it("marks a snapshot stale when the prompt projection changes", () => {
    // A persona edit invalidates without the renderer needing to understand
    // personas.
    const store = updateSession(emptyTaskSessionStore, "a", (s) => ({
      ...s,
      modelContext: measure(snapshot(key())),
    }));
    const next = reconcileModelContext(store, () => key({ promptEpoch: 2 }));
    expect(sessionFor(next, "a").modelContext.kind).toBe("stale");
  });

  it("keeps the figure when marking it stale, so the panel can date it", () => {
    // A number with a stated age is more use than a blank panel.
    const store = updateSession(emptyTaskSessionStore, "a", (s) => ({
      ...s,
      modelContext: measure(snapshot(key(), 4_321)),
    }));
    const next = reconcileModelContext(store, () => key({ promptEpoch: 9 }));
    const state = sessionFor(next, "a").modelContext;
    expect(state.kind === "stale" && state.snapshot.inputTokens).toBe(4_321);
  });

  it("reconciles each task against its own key", () => {
    let store = updateSession(emptyTaskSessionStore, "a", (s) => ({
      ...s,
      modelContext: measure(snapshot(key({ taskId: "a" }))),
    }));
    store = updateSession(store, "b", (s) => ({
      ...s,
      modelContext: measure(snapshot(key({ taskId: "b" }))),
    }));
    const next = reconcileModelContext(store, (taskId) =>
      key({ taskId, promptEpoch: taskId === "b" ? 2 : 1 }));
    expect(sessionFor(next, "a").modelContext.kind).toBe("measured");
    expect(sessionFor(next, "b").modelContext.kind).toBe("stale");
  });

  it("returns the same store when nothing changed", () => {
    // Identity matters: a new store on every render would re-run every
    // effect that depends on it.
    const store = updateSession(emptyTaskSessionStore, "a", (s) => ({
      ...s,
      modelContext: measure(snapshot(key())),
    }));
    expect(reconcileModelContext(store, () => key())).toBe(store);
  });
});

describe("the hook", () => {
  it("exposes the active task's session", () => {
    const { rerender, result } = renderHook(
      ({ id }) => useTaskSessions(id),
      { initialProps: { id: "a" } },
    );
    act(() => result.current.update("a", (s) => ({ ...s, draft: "for a" })));
    act(() => result.current.update("b", (s) => ({ ...s, draft: "for b" })));
    expect(result.current.session.draft).toBe("for a");

    rerender({ id: "b" });
    expect(result.current.session.draft).toBe("for b");
  });

  it("captures routing for the active task by default", () => {
    const { result } = renderHook(() => useTaskSessions("a"));
    act(() => result.current.update("a", (s) => s));
    expect(result.current.captureRouting().taskId).toBe("a");
  });

  /// The trap this API exists to remove. `setStore` is asynchronous and the
  /// ref behind `captureRouting` only re-syncs on render, so superseding and
  /// then capturing in the same handler returns the pre-bump generation -- a
  /// token that is stale the instant the bump lands, silently dropping every
  /// result. Two integration tests failed on exactly this.
  it("returns a token that is still current, in one call", () => {
    const { result } = renderHook(() => useTaskSessions("a"));
    let routing = { generation: -1, taskId: "" };
    act(() => { routing = result.current.beginRequest(); });
    expect(result.current.isCurrent(routing)).toBe(true);
  });

  it("supersedes the previous request when a new one begins", () => {
    const { result } = renderHook(() => useTaskSessions("a"));
    let first = { generation: -1, taskId: "" };
    let second = { generation: -1, taskId: "" };
    act(() => { first = result.current.beginRequest(); });
    act(() => { second = result.current.beginRequest(); });
    expect(result.current.isCurrent(first)).toBe(false);
    expect(result.current.isCurrent(second)).toBe(true);
  });

  it("supersedes correctly when two requests begin in one tick", () => {
    // The second must see the bump the first made, which is why the ref is
    // advanced by `beginRequest` and not only by the next render.
    const { result } = renderHook(() => useTaskSessions("a"));
    let first = { generation: -1, taskId: "" };
    let second = { generation: -1, taskId: "" };
    act(() => {
      first = result.current.beginRequest();
      second = result.current.beginRequest();
    });
    expect(first.generation).not.toBe(second.generation);
    expect(result.current.isCurrent(first)).toBe(false);
    expect(result.current.isCurrent(second)).toBe(true);
  });

  it("begins a request for a named task rather than the active one", () => {
    const { result } = renderHook(() => useTaskSessions("a"));
    let routing = { generation: -1, taskId: "" };
    act(() => { routing = result.current.beginRequest("b"); });
    expect(routing.taskId).toBe("b");
    expect(result.current.isCurrent(routing)).toBe(true);
  });

  /*
   * Only the callbacks that are named as effect dependencies need stable
   * identity, and `clearAll` is the one that is -- the workspace and host
   * invalidators both list it, and a new function on every task switch would
   * re-run them constantly.
   *
   * An earlier version of this test asserted `captureRouting` was stable too.
   * That was holding the whole API to a constraint one member needs, and it
   * cost a ref written during render, which React discards a render but still
   * performs. `captureRouting` now closes over the active id and nothing lists
   * it as a dependency.
   */
  it("keeps the invalidation callback stable across a task switch", () => {
    const { rerender, result } = renderHook(
      ({ id }) => useTaskSessions(id),
      { initialProps: { id: "a" } },
    );
    const before = result.current.clearAll;
    rerender({ id: "b" });
    expect(result.current.clearAll).toBe(before);
  });

  it("targets the new task after a switch", () => {
    // The other side of that trade: a callback closing over the active id has
    // to actually follow it.
    const { rerender, result } = renderHook(
      ({ id }) => useTaskSessions(id),
      { initialProps: { id: "a" } },
    );
    rerender({ id: "b" });
    act(() => result.current.updateActive((s) => ({ ...s, draft: "for b" })));
    expect(result.current.sessionFor("b").draft).toBe("for b");
    expect(result.current.sessionFor("a").draft).toBe("");
  });
});
