import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { createTask, type TaskRecord } from "../../state/taskList";
import {
  useTaskLifecycle,
  type PrimaryRoute,
  type TaskLifecycle,
  type TaskLifecyclePorts,
} from "./taskLifecycle";

/** `lifecycle` and `rerender` are attached once the hook is rendered. */
type Harness = HarnessState & {
  lifecycle: TaskLifecycle;
  rerender: () => void;
};

interface HarnessState {
  tasks: readonly TaskRecord[];
  route: PrimaryRoute;
  drafts: Map<string, string>;
  drawerClosed: number;
  modalClosed: number;
  deleteConfirmationOpened: number;
  releasedRunState: string[];
  droppedChatSessions: string[];
  pendingDeleteTaskId: string | null;
  guidanceResets: number;
  canStart: boolean;
}

function harness(seed: readonly TaskRecord[] = [], routedTaskId: string | null = null): Harness {
  const state: HarnessState = {
    canStart: true,
    deleteConfirmationOpened: 0,
    drafts: new Map(),
    drawerClosed: 0,
    droppedChatSessions: [],
    guidanceResets: 0,
    modalClosed: 0,
    pendingDeleteTaskId: null,
    releasedRunState: [],
    route: routedTaskId === null
      ? { kind: "home" }
      : { kind: "task", taskId: routedTaskId },
    tasks: seed,
  };
  const h = state as Harness;

  let nextId = 0;
  const ports = (): TaskLifecyclePorts => ({
    canStartTask: () => h.canStart,
    closeDrawer: () => { h.drawerClosed += 1; },
    closeModal: () => { h.modalClosed += 1; },
    createTaskId: () => `generated-${++nextId}`,
    drafts: {
      drop: (taskId) => { h.drafts.delete(taskId); },
      set: (taskId, draft) => { h.drafts.set(taskId, draft); },
    },
    dropChatSession: (taskId) => { h.droppedChatSessions.push(taskId); },
    openDeleteConfirmation: () => { h.deleteConfirmationOpened += 1; },
    releaseRunState: (taskId) => { h.releasedRunState.push(taskId); },
    resetGuidanceForNewTask: () => { h.guidanceResets += 1; },
    routedTaskId: h.route.kind === "task" ? h.route.taskId : null,
    selectedSessionId: (h.route.kind === "task" ? h.route.taskId : null) ?? "task-none",
    setPendingDeleteTaskId: (taskId) => { h.pendingDeleteTaskId = taskId; },
    setPrimaryRoute: (route) => { h.route = route; },
    setTasks: (update) => { h.tasks = update(h.tasks); },
    tasks: h.tasks,
  });

  const { rerender, result } = renderHook(() => useTaskLifecycle(ports()));
  Object.defineProperty(h, "lifecycle", { get: () => result.current });
  h.rerender = rerender;
  return h;
}

/** Runs the rAF the focus restore schedules. */
function flushAnimationFrame(): void {
  vi.runAllTimers();
}

afterEach(() => { vi.useRealTimers(); });

describe("selectSession", () => {
  it("routes to the task", () => {
    const h = harness([createTask("a"), createTask("b")]);
    act(() => h.lifecycle.selectSession("b"));
    expect(h.route).toEqual({ kind: "task", taskId: "b" });
    expect(h.drawerClosed).toBe(1);
  });

  /*
   * FE-13 reversed this. Switching used to clear attachments, because they
   * lived in one app-global slot and the alternative was sending one task's
   * files with another task's message. Now that a task owns them, a switch
   * touches nothing -- and the release path is the only thing that discards.
   */
  it("does not release any task's state when switching between tasks", () => {
    const h = harness([createTask("a"), createTask("b")], "a");
    act(() => h.lifecycle.selectSession("b"));
    expect(h.releasedRunState).toEqual([]);
  });

  it("does not release state when re-selecting the task already open", () => {
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.selectSession("a"));
    expect(h.releasedRunState).toEqual([]);
  });

  it("does not release the previous task's state when a new one starts", () => {
    // The old `clearContext` ran here and acted on the *outgoing* task, which
    // is not what starting a task should touch.
    const h = harness([createTask("a")], "a");
    act(() => { h.lifecycle.startNewSession(); });
    expect(h.releasedRunState).toEqual([]);
  });

  it("does not release state when reopening a closed task", () => {
    // Its session was dropped when it closed, so a reopened task starts empty
    // by construction and there is nothing to clear.
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.closeTaskById("a"));
    h.rerender();
    h.releasedRunState.length = 0;
    act(() => h.lifecycle.reopenTaskById("a"));
    expect(h.releasedRunState).toEqual([]);
  });

  it("ignores a task that does not exist", () => {
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.selectSession("ghost"));
    expect(h.route).toEqual({ kind: "task", taskId: "a" });
  });
});

describe("startNewSession", () => {
  it("creates, routes to, and resets guidance for a new task", () => {
    const h = harness();
    let created: string | null = null;
    act(() => { created = h.lifecycle.startNewSession(); });
    expect(created).toBe("generated-1");
    expect(h.tasks.map((t) => t.id)).toEqual(["generated-1"]);
    expect(h.route).toEqual({ kind: "task", taskId: "generated-1" });
    expect(h.guidanceResets).toBe(1);
  });

  it("refuses while a guidance request is mid-flight", () => {
    // A task started mid-request would leave the in-flight approval pointing at
    // a task the user is no longer looking at.
    const h = harness();
    h.canStart = false;
    let created: string | null = "unset";
    act(() => { created = h.lifecycle.startNewSession(); });
    expect(created).toBeNull();
    expect(h.tasks).toEqual([]);
  });

  it("keeps the intent as a draft when asked", () => {
    const h = harness();
    act(() => { h.lifecycle.startNewSession("  fix the login  ", true); });
    expect(h.drafts.get("generated-1")).toBe("fix the login");
  });

  it("does not store a blank intent as a draft", () => {
    const h = harness();
    act(() => { h.lifecycle.startNewSession("   ", true); });
    expect(h.drafts.size).toBe(0);
  });

  it("never reuses an id", () => {
    // The id becomes the host's `client_session_ref`. A repeat means a fresh
    // task silently adopts an older task's persisted transcript.
    const h = harness();
    act(() => { h.lifecycle.startNewSession(); });
    h.rerender();
    act(() => { h.lifecycle.startNewSession(); });
    expect(h.tasks.map((t) => t.id)).toEqual(["generated-1", "generated-2"]);
  });
});

describe("closeTaskById", () => {
  it("keeps the transcript and the draft", () => {
    // Closing is non-destructive, which is why it has no confirmation:
    // reopening from Home restores the conversation.
    const h = harness([createTask("a")], "a");
    h.drafts.set("a", "unsent");
    act(() => h.lifecycle.closeTaskById("a"));
    expect(h.drafts.get("a")).toBe("unsent");
    expect(h.droppedChatSessions).toEqual([]);
    expect(h.tasks.find((t) => t.id === "a")?.closed).toBe(true);
  });

  it("releases the in-progress run state", () => {
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.closeTaskById("a"));
    expect(h.releasedRunState).toEqual(["a"]);
  });

  it("lands on Home when the last open task closes", () => {
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.closeTaskById("a"));
    expect(h.route).toEqual({ kind: "home" });
  });

  it("moves to another open task rather than Home when one remains", () => {
    const h = harness([createTask("a"), createTask("b")], "a");
    act(() => h.lifecycle.closeTaskById("a"));
    expect(h.route).toEqual({ kind: "task", taskId: "b" });
  });

  it("leaves the route alone when closing a task that is not open on screen", () => {
    const h = harness([createTask("a"), createTask("b")], "a");
    act(() => h.lifecycle.closeTaskById("b"));
    expect(h.route).toEqual({ kind: "task", taskId: "a" });
  });
});

describe("deleting a task", () => {
  it("drops the record, the draft and the conversation", () => {
    const h = harness([createTask("a")], "a");
    h.drafts.set("a", "unsent");
    act(() => h.lifecycle.confirmTaskDeletion("a"));
    expect(h.tasks).toEqual([]);
    expect(h.drafts.has("a")).toBe(false);
    // Supersedes the lane, so a reply already in flight is dropped rather than
    // resurrecting the transcript.
    expect(h.droppedChatSessions).toEqual(["a"]);
  });

  it("does nothing to a task that has already gone", () => {
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.confirmTaskDeletion("ghost"));
    expect(h.tasks.map((t) => t.id)).toEqual(["a"]);
    expect(h.droppedChatSessions).toEqual([]);
  });

  it("clears the pending id and closes the dialog on both paths", () => {
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.requestTaskDeletion("a"));
    expect(h.pendingDeleteTaskId).toBe("a");
    act(() => h.lifecycle.dismissTaskDeletion());
    expect(h.pendingDeleteTaskId).toBeNull();
    expect(h.modalClosed).toBe(1);
  });
});

describe("focus after the delete confirmation closes", () => {
  function withOpener(): HTMLButtonElement {
    const opener = document.createElement("button");
    document.body.append(opener);
    opener.focus();
    return opener;
  }

  it("returns focus when the deletion is dismissed", () => {
    vi.useFakeTimers();
    const opener = withOpener();
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.requestTaskDeletion("a"));
    opener.blur();
    act(() => h.lifecycle.dismissTaskDeletion());
    flushAnimationFrame();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });

  it("returns focus when the deletion is confirmed", () => {
    vi.useFakeTimers();
    const opener = withOpener();
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.requestTaskDeletion("a"));
    opener.blur();
    act(() => h.lifecycle.confirmTaskDeletion("a"));
    flushAnimationFrame();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });

  /// Regression. The confirm path used to return early when the task had
  /// already gone, closing the dialog and leaving focus on `<body>` -- with no
  /// sensible place for the next keystroke to land.
  it("returns focus even when the task has already gone", () => {
    vi.useFakeTimers();
    const opener = withOpener();
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.requestTaskDeletion("a"));
    opener.blur();
    act(() => h.lifecycle.confirmTaskDeletion("ghost"));
    flushAnimationFrame();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });

  it("does not focus an element that has left the document", () => {
    // On the confirm path the element focus came from is often the row of the
    // task that was just deleted.
    vi.useFakeTimers();
    const opener = withOpener();
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.requestTaskDeletion("a"));
    opener.remove();
    expect(() => {
      act(() => h.lifecycle.confirmTaskDeletion("a"));
      flushAnimationFrame();
    }).not.toThrow();
  });
});

describe("renaming and auto-titling", () => {
  it("renames the routed task", () => {
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.renameSelectedTask("Fix login"));
    expect(h.tasks[0]?.title).toBe("Fix login");
  });

  it("does nothing on Home", () => {
    const h = harness([createTask("a")]);
    act(() => h.lifecycle.renameSelectedTask("Fix login"));
    expect(h.tasks[0]?.title).not.toBe("Fix login");
  });

  it("never overwrites a title the user typed", () => {
    // Most tasks are never named by hand, so the opening message is the only
    // honest label -- but it must not replace one that was.
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.renameSelectedTask("Chosen by hand"));
    act(() => h.lifecycle.autoTitleFromMessage("a", "some later message"));
    expect(h.tasks[0]?.title).toBe("Chosen by hand");
  });
});

describe("reopenTaskById", () => {
  it("reopens and routes to the task", () => {
    const h = harness([createTask("a")], "a");
    act(() => h.lifecycle.closeTaskById("a"));
    h.rerender();
    act(() => h.lifecycle.reopenTaskById("a"));
    expect(h.tasks[0]?.closed).toBe(false);
    expect(h.route).toEqual({ kind: "task", taskId: "a" });
  });
});
