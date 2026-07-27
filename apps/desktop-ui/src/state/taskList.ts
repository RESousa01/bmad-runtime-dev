// The user-visible task list.
//
// A task is a conversation bound to a workspace. Until now the renderer held it
// as an anonymous `{id, title}` summary that was written once at creation and
// never touched again, which is why every task the user did not start from the
// Home composer stayed called "New task" forever. This module makes the list a
// real, mutable, testable thing: a task can be titled from its first message,
// renamed by hand, closed, and deleted.
//
// Two rules are load-bearing rather than cosmetic:
//
//  * A **manual** title is never overwritten by an automatic one. Auto-titling
//    exists because most tasks never get named by hand; the moment the user
//    does name one, the guess must stop competing with them.
//  * A task id is **never reused**. The host derives the durable chat session
//    identity from `installation_id + workspace_id + client_session_ref`, and
//    `client_session_ref` is exactly this id. A recycled id would silently
//    adopt a previous task's persisted transcript — including one the user
//    thought they had deleted — and the host would build its next request from
//    a conversation the user cannot see. `createTaskId` is therefore unique per
//    task per installation, not a per-launch counter.
//
// The store is pure and immutable: every mutator returns a new array, so the
// React shell can drive re-renders from it and the semantics stay testable
// without mounting React. It mirrors `taskSessions` and `chatSessions`, which
// are keyed by the same task id.

export const DEFAULT_TASK_TITLE = "New task";

/// Titles are a tab label first and a heading second, so they are bounded to
/// something that stays readable in a 220px tab.
export const MAX_TASK_TITLE_CHARS = 64;

/// Upper bound on retained tasks. The per-task chat and capability stores are
/// bounded the same way; without a cap here a long session grows a list (and a
/// tab strip) without limit.
export const MAX_TASKS = 20;

/// Where a task's current title came from.
///
/// `default` means it is still the placeholder and may be replaced by an
/// automatic title. `auto` was derived from the first message. `manual` was
/// typed by the user and is never replaced automatically.
export type TaskTitleSource = "default" | "auto" | "manual";

export interface TaskRecord {
  readonly id: string;
  readonly title: string;
  readonly titleSource: TaskTitleSource;
  /// A closed task leaves the open tab strip but keeps its transcript, so it
  /// can be reopened. Deleting is the destructive one.
  readonly closed: boolean;
  readonly updatedAt: string;
}

/// Mints an id that is unique across app launches and installations.
///
/// See the module note: this value becomes the host's `client_session_ref`, so
/// a collision is not a cosmetic bug — it hands a new task someone else's
/// durable transcript. `randomUUID` is used when the runtime offers it;
/// the fallback is only for environments that do not (older embedded
/// WebViews), and still mixes a time component with two random draws.
export function createTaskId(): string {
  const uuid = globalThis.crypto?.randomUUID?.();
  if (uuid !== undefined) {
    return `task-${uuid}`;
  }
  const time = Date.now().toString(36);
  const first = Math.random().toString(36).slice(2, 12);
  const second = Math.random().toString(36).slice(2, 12);
  return `task-${time}-${first}${second}`;
}

/// Derives a task title from a message the user sent.
///
/// Returns `null` when the message holds nothing usable, so the caller keeps
/// the placeholder rather than titling a task with an empty string. Only the
/// first line is considered: a pasted multi-line prompt should be labelled by
/// its opening sentence, not by its last line of code.
export function taskTitleFromMessage(message: string): string | null {
  const firstLine = message.split("\n").find((line) => line.trim().length > 0);
  if (firstLine === undefined) {
    return null;
  }
  const collapsed = firstLine.replace(/\s+/gu, " ").trim();
  if (collapsed.length === 0) {
    return null;
  }
  return collapsed.length <= MAX_TASK_TITLE_CHARS
    ? collapsed
    : `${collapsed.slice(0, MAX_TASK_TITLE_CHARS - 1).trimEnd()}…`;
}

/// Creates a task, titling it from `intent` when one was supplied.
export function createTask(id: string, intent?: string): TaskRecord {
  const title = intent === undefined ? null : taskTitleFromMessage(intent);
  return {
    id,
    title: title ?? DEFAULT_TASK_TITLE,
    titleSource: title === null ? "default" : "auto",
    closed: false,
    updatedAt: "Now",
  };
}

/// Appends a task, evicting the oldest to stay within `MAX_TASKS`.
///
/// Closed tasks are evicted before open ones: a closed task is the user's own
/// "I am done with this", so it is the safer thing to lose when the list is
/// full, and dropping an open tab out from under someone is not.
export function appendTask(
  tasks: readonly TaskRecord[],
  task: TaskRecord,
): readonly TaskRecord[] {
  const next = [...tasks.filter(({ id }) => id !== task.id), task];
  while (next.length > MAX_TASKS) {
    const oldestClosed = next.findIndex((candidate) => candidate.closed);
    next.splice(oldestClosed >= 0 ? oldestClosed : 0, 1);
  }
  return next;
}

/// Tasks shown in the open tab strip, in creation order.
export function openTasks(tasks: readonly TaskRecord[]): readonly TaskRecord[] {
  return tasks.filter((task) => !task.closed);
}

/// Tasks the user closed. Newest first: this is a "pick one back up" list.
export function closedTasks(tasks: readonly TaskRecord[]): readonly TaskRecord[] {
  return tasks.filter((task) => task.closed).toReversed();
}

export function findTask(
  tasks: readonly TaskRecord[],
  id: string | null,
): TaskRecord | null {
  if (id === null) return null;
  return tasks.find((task) => task.id === id) ?? null;
}

function replace(
  tasks: readonly TaskRecord[],
  id: string,
  update: (task: TaskRecord) => TaskRecord,
): readonly TaskRecord[] {
  let changed = false;
  const next = tasks.map((task) => {
    if (task.id !== id) return task;
    changed = true;
    return update(task);
  });
  return changed ? next : tasks;
}

/// Renames a task by hand, pinning the title against later auto-titling.
///
/// A blank title is not stored as an empty label: it clears the name back to
/// the placeholder and re-arms auto-titling, which is the only sensible reading
/// of "the user erased the name".
export function renameTask(
  tasks: readonly TaskRecord[],
  id: string,
  title: string,
): readonly TaskRecord[] {
  const collapsed = title.replace(/\s+/gu, " ").trim();
  if (collapsed.length === 0) {
    return replace(tasks, id, (task) => ({
      ...task,
      title: DEFAULT_TASK_TITLE,
      titleSource: "default",
    }));
  }
  const bounded = collapsed.length <= MAX_TASK_TITLE_CHARS
    ? collapsed
    : `${collapsed.slice(0, MAX_TASK_TITLE_CHARS - 1).trimEnd()}…`;
  return replace(tasks, id, (task) => ({
    ...task,
    title: bounded,
    titleSource: "manual",
  }));
}

/// Titles a task from its first message, if it is still unnamed.
///
/// Deliberately a no-op once the title is `auto` or `manual`: the name should
/// describe how the conversation started, and a hand-picked name must survive
/// everything sent afterwards.
export function autoTitleTask(
  tasks: readonly TaskRecord[],
  id: string,
  message: string,
): readonly TaskRecord[] {
  const task = findTask(tasks, id);
  if (task === null || task.titleSource !== "default") {
    return tasks;
  }
  const title = taskTitleFromMessage(message);
  if (title === null) {
    return tasks;
  }
  return replace(tasks, id, (current) => ({ ...current, title, titleSource: "auto" }));
}

/// Hides a task from the open list while keeping its transcript.
export function closeTask(
  tasks: readonly TaskRecord[],
  id: string,
): readonly TaskRecord[] {
  return replace(tasks, id, (task) => ({ ...task, closed: true }));
}

export function reopenTask(
  tasks: readonly TaskRecord[],
  id: string,
): readonly TaskRecord[] {
  return replace(tasks, id, (task) => ({ ...task, closed: false }));
}

/// Destroys a task. Irreversible, which is why the caller confirms first.
export function deleteTask(
  tasks: readonly TaskRecord[],
  id: string,
): readonly TaskRecord[] {
  const next = tasks.filter((task) => task.id !== id);
  return next.length === tasks.length ? tasks : next;
}

/// Which task to show once `removedId` leaves the open list.
///
/// Takes the list *before* the removal, because that is the only version that
/// still knows where the departing tab sat. Callers compute the next selection
/// first, then apply `closeTask`/`deleteTask`.
///
/// Only matters when the departing task was the selected one. Prefers the tab
/// to its left — the one the user was most likely on before — then the tab to
/// its right, then `null`, which the shell reads as "go Home". `null` is the
/// answer for the last task, and that is the point: there is no phantom task to
/// fall back onto.
export function nextSelectionAfter(
  tasksBeforeRemoval: readonly TaskRecord[],
  removedId: string,
  selectedId: string | null,
): string | null {
  const open = openTasks(tasksBeforeRemoval);
  if (selectedId !== removedId) {
    const stillOpen = open.some(
      (task) => task.id === selectedId && task.id !== removedId,
    );
    return stillOpen ? selectedId : null;
  }
  const index = open.findIndex((task) => task.id === removedId);
  if (index < 0) {
    return null;
  }
  return open[index - 1]?.id ?? open[index + 1]?.id ?? null;
}
