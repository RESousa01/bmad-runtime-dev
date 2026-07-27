import { describe, expect, it } from "vitest";

import {
  appendTask,
  autoTitleTask,
  closeTask,
  closedTasks,
  createTask,
  createTaskId,
  DEFAULT_TASK_TITLE,
  deleteTask,
  findTask,
  MAX_TASK_TITLE_CHARS,
  MAX_TASKS,
  nextSelectionAfter,
  openTasks,
  renameTask,
  reopenTask,
  taskTitleFromMessage,
  type TaskRecord,
} from "./taskList";

function listOf(...ids: string[]): readonly TaskRecord[] {
  return ids.reduce<readonly TaskRecord[]>(
    (tasks, id) => appendTask(tasks, createTask(id)),
    [],
  );
}

describe("task identity", () => {
  it("never mints the same id twice, so a new task cannot adopt a deleted transcript", () => {
    // The id is the host's `client_session_ref`, and the durable chat session
    // is derived from it. A recycled id is not a cosmetic collision: it hands
    // the new task the old task's persisted messages.
    const ids = new Set(Array.from({ length: 500 }, () => createTaskId()));
    expect(ids.size).toBe(500);
    for (const id of ids) {
      expect(id.startsWith("task-")).toBe(true);
      // The host rejects a client session ref longer than 128 bytes.
      expect(id.length).toBeLessThanOrEqual(128);
    }
  });
});

describe("titles", () => {
  it("titles a task from the first line of its first message", () => {
    expect(taskTitleFromMessage("Plan the release")).toBe("Plan the release");
    expect(taskTitleFromMessage("Fix the parser\nit crashes on empty input")).toBe(
      "Fix the parser",
    );
    expect(taskTitleFromMessage("  Review   my  changes  ")).toBe("Review my changes");
    expect(taskTitleFromMessage("\n\nSecond line is the first real one")).toBe(
      "Second line is the first real one",
    );
  });

  it("keeps the placeholder when a message holds nothing usable", () => {
    expect(taskTitleFromMessage("")).toBeNull();
    expect(taskTitleFromMessage("   \n\t  ")).toBeNull();
    expect(createTask("a", "   ").title).toBe(DEFAULT_TASK_TITLE);
    expect(createTask("a", "   ").titleSource).toBe("default");
  });

  it("bounds a long title so it stays readable as a tab label", () => {
    const title = taskTitleFromMessage("x".repeat(200));
    expect(title).toHaveLength(MAX_TASK_TITLE_CHARS);
    expect(title?.endsWith("…")).toBe(true);
  });

  it("auto-titles an unnamed task from its first message and then stops", () => {
    let tasks = listOf("a");
    expect(findTask(tasks, "a")?.title).toBe(DEFAULT_TASK_TITLE);

    tasks = autoTitleTask(tasks, "a", "Explain the workspace layout");
    expect(findTask(tasks, "a")?.title).toBe("Explain the workspace layout");
    expect(findTask(tasks, "a")?.titleSource).toBe("auto");

    // The name describes how the conversation opened, so later messages do not
    // rewrite it.
    tasks = autoTitleTask(tasks, "a", "Now do something completely different");
    expect(findTask(tasks, "a")?.title).toBe("Explain the workspace layout");
  });

  it("never lets an automatic title overwrite one the user typed", () => {
    let tasks = renameTask(listOf("a"), "a", "Release checklist");
    expect(findTask(tasks, "a")?.titleSource).toBe("manual");

    tasks = autoTitleTask(tasks, "a", "Plan the migration");
    expect(findTask(tasks, "a")?.title).toBe("Release checklist");
  });

  it("clears a name back to the placeholder and re-arms auto-titling", () => {
    let tasks = renameTask(listOf("a"), "a", "Release checklist");
    tasks = renameTask(tasks, "a", "   ");
    expect(findTask(tasks, "a")?.title).toBe(DEFAULT_TASK_TITLE);
    expect(findTask(tasks, "a")?.titleSource).toBe("default");

    tasks = autoTitleTask(tasks, "a", "Plan the migration");
    expect(findTask(tasks, "a")?.title).toBe("Plan the migration");
  });

  it("bounds and collapses a manually typed title", () => {
    const tasks = renameTask(listOf("a"), "a", `  many   spaces  ${"y".repeat(200)}`);
    expect(findTask(tasks, "a")?.title).toHaveLength(MAX_TASK_TITLE_CHARS);
  });

  it("leaves the list untouched when the task is gone", () => {
    const tasks = listOf("a");
    expect(renameTask(tasks, "missing", "x")).toBe(tasks);
    expect(autoTitleTask(tasks, "missing", "x")).toBe(tasks);
    expect(deleteTask(tasks, "missing")).toBe(tasks);
  });
});

describe("close and delete", () => {
  it("closing keeps the task and its transcript, only hiding it from the open list", () => {
    const tasks = closeTask(listOf("a", "b"), "a");
    expect(openTasks(tasks).map(({ id }) => id)).toEqual(["b"]);
    expect(closedTasks(tasks).map(({ id }) => id)).toEqual(["a"]);
    // Still a record, so it can come back with its name intact.
    expect(findTask(tasks, "a")).not.toBeNull();
  });

  it("reopens a closed task in its original position", () => {
    let tasks = closeTask(listOf("a", "b", "c"), "b");
    tasks = reopenTask(tasks, "b");
    expect(openTasks(tasks).map(({ id }) => id)).toEqual(["a", "b", "c"]);
    expect(closedTasks(tasks)).toEqual([]);
  });

  it("deleting destroys the record, unlike closing", () => {
    const tasks = deleteTask(listOf("a", "b"), "a");
    expect(findTask(tasks, "a")).toBeNull();
    expect(openTasks(tasks).map(({ id }) => id)).toEqual(["b"]);
    expect(closedTasks(tasks)).toEqual([]);
  });

  it("deletes a closed task too", () => {
    const tasks = deleteTask(closeTask(listOf("a", "b"), "a"), "a");
    expect(findTask(tasks, "a")).toBeNull();
    expect(openTasks(tasks).map(({ id }) => id)).toEqual(["b"]);
  });
});

describe("selection after a task leaves", () => {
  it("prefers the tab to the left", () => {
    expect(nextSelectionAfter(listOf("a", "b", "c"), "b", "b")).toBe("a");
  });

  it("falls back to the tab on the right for the first task", () => {
    expect(nextSelectionAfter(listOf("a", "b", "c"), "a", "a")).toBe("b");
  });

  it("returns Home when the last task is removed", () => {
    // The edge case the old shell could not express: there was always a
    // phantom "New task" tab to fall back onto.
    const tasks = listOf("only");
    expect(nextSelectionAfter(tasks, "only", "only")).toBeNull();
    expect(deleteTask(tasks, "only")).toEqual([]);
  });

  it("returns Home when the last open task is closed but kept", () => {
    const tasks = listOf("only");
    expect(nextSelectionAfter(tasks, "only", "only")).toBeNull();
    expect(closedTasks(closeTask(tasks, "only")).map(({ id }) => id)).toEqual(["only"]);
  });

  it("keeps the current task selected when a different one leaves", () => {
    expect(nextSelectionAfter(listOf("a", "b", "c"), "c", "a")).toBe("a");
  });

  it("skips a task that is already closed when picking a neighbour", () => {
    const tasks = closeTask(listOf("a", "b", "c"), "a");
    expect(nextSelectionAfter(tasks, "b", "b")).toBe("c");
  });
});

describe("bounded growth", () => {
  it("evicts the oldest closed task before any open one", () => {
    let tasks: readonly TaskRecord[] = [];
    for (let index = 0; index < MAX_TASKS; index += 1) {
      tasks = appendTask(tasks, createTask(`t${index}`));
    }
    tasks = closeTask(tasks, "t3");
    tasks = appendTask(tasks, createTask("overflow"));

    expect(tasks).toHaveLength(MAX_TASKS);
    // The closed one went; the oldest open task survived.
    expect(findTask(tasks, "t3")).toBeNull();
    expect(findTask(tasks, "t0")).not.toBeNull();
    expect(findTask(tasks, "overflow")).not.toBeNull();
  });

  it("evicts the oldest task when every task is open", () => {
    let tasks: readonly TaskRecord[] = [];
    for (let index = 0; index <= MAX_TASKS; index += 1) {
      tasks = appendTask(tasks, createTask(`t${index}`));
    }
    expect(tasks).toHaveLength(MAX_TASKS);
    expect(findTask(tasks, "t0")).toBeNull();
  });
});
