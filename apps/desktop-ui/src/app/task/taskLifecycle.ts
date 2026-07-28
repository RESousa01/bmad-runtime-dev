/**
 * The task lifecycle controller: open, select, rename, close, reopen, delete.
 *
 * FE-10 step 3. These twelve functions were plain closures over App's state,
 * so what a task action touches was implicit -- you learned it by reading the
 * body. The dependency set is real and it is large: the task list, the route,
 * drafts, chat lanes, capability runs, governed-changes flows, attached
 * context, the drawer, and the in-flight BMAD request. Naming it in one
 * interface does not make it smaller, but it makes it countable, and a new
 * dependency now has to be declared rather than captured.
 *
 * What the closures were hiding
 * -----------------------------
 * Two things this move fixes:
 *
 * - Dismissing and confirming a deletion each restored focus with their own
 *   copy of the same five lines. Two copies of "put focus back" is how one
 *   path quietly stops doing it.
 * - Both borrowed `workspaceReturnFocusRef` -- the *workspace* panel's slot --
 *   to remember where focus came from. Only one modal is ever open, so it did
 *   not collide, but a ref named for one feature and used by another is a
 *   collision waiting for the second modal.
 */

import { useRef } from "react";

import {
  appendTask,
  autoTitleTask,
  closeTask,
  createTask,
  deleteTask,
  findTask,
  nextSelectionAfter,
  renameTask,
  reopenTask,
  type TaskRecord,
} from "../../state/taskList";

export type PrimaryRoute =
  | { kind: "home" }
  | { kind: "task"; taskId: string | null };

export interface TaskLifecyclePorts {
  readonly tasks: readonly TaskRecord[];
  readonly setTasks: (
    update: (current: readonly TaskRecord[]) => readonly TaskRecord[],
  ) => void;
  /** The task the route points at, or null on Home. */
  readonly routedTaskId: string | null;
  /** The per-task store key, which is the routed task or the no-task sentinel. */
  readonly selectedSessionId: string;
  readonly setPrimaryRoute: (route: PrimaryRoute) => void;
  readonly closeDrawer: () => void;
  /**
   * Discards the transient state a task owns: its capability run, its
   * governed-changes flow, and its attachments. The transcript is the host's
   * and is untouched.
   *
   * There used to be a `clearContext` port called on four paths -- select,
   * reopen, close and new -- because attachments were app-global and the only
   * safe thing to do with state belonging to nobody was to destroy it on every
   * route change. Every one of those calls acted on the *active* task, which is
   * never the task the operation is about; with one global slot that was
   * invisible, and with per-task state all four were wrong.
   */
  readonly releaseRunState: (taskId: string) => void;
  /** New ids must never repeat: the host derives durable session identity from them. */
  readonly createTaskId: () => string;
  /** Blocks starting a task while a BMAD request is mid-flight. */
  readonly canStartTask: () => boolean;
  /** Resets the BMAD request state a new task starts from. */
  readonly resetGuidanceForNewTask: () => void;
  readonly drafts: {
    readonly set: (taskId: string, draft: string) => void;
    readonly drop: (taskId: string) => void;
  };
  /** Discards a conversation and supersedes it, so a reply in flight is dropped. */
  readonly dropChatSession: (taskId: string) => void;
  readonly setPendingDeleteTaskId: (taskId: string | null) => void;
  readonly openDeleteConfirmation: () => void;
  readonly closeModal: () => void;
}

export interface TaskLifecycle {
  readonly selectSession: (id: string) => void;
  readonly startNewSession: (
    intent?: string,
    preserveIntentAsDraft?: boolean,
  ) => string | null;
  readonly autoTitleFromMessage: (taskId: string, message: string) => void;
  readonly renameSelectedTask: (title: string) => void;
  readonly closeTaskById: (taskId: string) => void;
  readonly reopenTaskById: (taskId: string) => void;
  readonly requestTaskDeletion: (taskId: string) => void;
  readonly dismissTaskDeletion: () => void;
  readonly confirmTaskDeletion: (pendingDeleteTaskId: string | null) => void;
}

export function useTaskLifecycle(ports: TaskLifecyclePorts): TaskLifecycle {
  /**
   * Where focus came from when the delete confirmation opened.
   *
   * Owned here rather than borrowed from the workspace panel. Only one modal is
   * open at a time so sharing a slot did not collide in practice, but a ref
   * named for one feature and written by another is a collision waiting for the
   * second modal.
   */
  const deleteReturnFocusRef = useRef<HTMLElement | null>(null);

  /**
   * Puts focus back where the confirmation took it from.
   *
   * One copy, used by both the dismiss and the confirm path. Two copies of
   * "put focus back" is how one of them quietly stops doing it.
   *
   * The `rAF` matters: the modal is still mounted when this is called, and
   * focusing an element the modal's own teardown is about to move focus away
   * from loses the race. The `isConnected` and `[inert]` checks are for the
   * confirm path, where the element focus came from can be the row of the task
   * that was just deleted.
   */
  function restoreFocusAfterModal(): void {
    const target = deleteReturnFocusRef.current;
    deleteReturnFocusRef.current = null;
    window.requestAnimationFrame(() => {
      if (target?.isConnected && !target.closest("[inert]")) target.focus();
    });
  }

  /** Moves the route off a task that is leaving the open list. */
  function reselectAfterRemoval(taskId: string): void {
    const next = nextSelectionAfter(ports.tasks, taskId, ports.routedTaskId);
    if (ports.routedTaskId !== taskId) return;
    ports.closeDrawer();
    // No phantom task to land on: with nothing open, Home *is* the empty state.
    ports.setPrimaryRoute(next === null ? { kind: "home" } : { kind: "task", taskId: next });
  }

  return {
    autoTitleFromMessage(taskId: string, message: string) {
      // `autoTitleTask` is a no-op once the task has any title, which is what
      // keeps a name the user typed from being overwritten by their next
      // message.
      ports.setTasks((current) => autoTitleTask(current, taskId, message));
    },

    closeTaskById(taskId: string) {
      // Non-destructive: the task leaves the tab strip but keeps its transcript
      // and its unsent draft, so reopening it from Home restores the
      // conversation. Hence no confirmation.
      if (findTask(ports.tasks, taskId) === null) return;
      reselectAfterRemoval(taskId);
      ports.releaseRunState(taskId);
      ports.setTasks((current) => closeTask(current, taskId));
    },

    confirmTaskDeletion(pendingDeleteTaskId: string | null) {
      /*
       * Irreversible from Forge's side, which is why it is only reachable
       * through the confirmation dialog.
       *
       * This drops the renderer's record and its conversation lane. The host's
       * durable transcript is *not* deleted: the command catalog exposes no
       * per-session chat delete, and the store offers no removal at all.
       * Because task ids are never reused, the orphaned row can no longer be
       * addressed by any task; erasing it from disk is Settings -> Local data.
       * `DeleteTaskDialog` says exactly this rather than promising an erase
       * that does not happen.
       */
      ports.setPendingDeleteTaskId(null);
      ports.closeModal();
      if (pendingDeleteTaskId === null
        || findTask(ports.tasks, pendingDeleteTaskId) === null) {
        // Still restore focus: the dialog was open and is now closed, however
        // it got into a state where the task had already gone.
        restoreFocusAfterModal();
        return;
      }
      reselectAfterRemoval(pendingDeleteTaskId);
      ports.releaseRunState(pendingDeleteTaskId);
      ports.drafts.drop(pendingDeleteTaskId);
      ports.dropChatSession(pendingDeleteTaskId);
      ports.setTasks((current) => deleteTask(current, pendingDeleteTaskId));
      restoreFocusAfterModal();
    },

    dismissTaskDeletion() {
      ports.setPendingDeleteTaskId(null);
      ports.closeModal();
      restoreFocusAfterModal();
    },

    renameSelectedTask(title: string) {
      if (ports.routedTaskId === null) return;
      ports.setTasks((current) => renameTask(current, ports.routedTaskId!, title));
    },

    reopenTaskById(taskId: string) {
      if (findTask(ports.tasks, taskId) === null) return;
      // Nothing to clear: the session was dropped when the task closed, so a
      // reopened task starts empty by construction.
      ports.setTasks((current) => reopenTask(current, taskId));
      ports.closeDrawer();
      ports.setPrimaryRoute({ kind: "task", taskId });
    },

    requestTaskDeletion(taskId: string) {
      if (findTask(ports.tasks, taskId) === null) return;
      deleteReturnFocusRef.current = document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
      ports.setPendingDeleteTaskId(taskId);
      ports.closeDrawer();
      ports.openDeleteConfirmation();
    },

    selectSession(id: string) {
      if (findTask(ports.tasks, id) === null) return;
      // FE-13: nothing is cleared on a switch. A task keeps its attachments
      // while the user works elsewhere, which is the whole point of owning
      // them.
      ports.setPrimaryRoute({ kind: "task", taskId: id });
      ports.closeDrawer();
    },

    startNewSession(intent?: string, preserveIntentAsDraft = false): string | null {
      if (!ports.canStartTask()) return null;
      // The id becomes the host's `client_session_ref`, from which the durable
      // chat session identity is derived. It must never repeat, or a fresh task
      // silently adopts an older task's persisted transcript -- including one
      // the user deleted.
      const session = createTask(ports.createTaskId(), intent);
      ports.resetGuidanceForNewTask();
      if (preserveIntentAsDraft && intent?.trim()) {
        ports.drafts.set(session.id, intent.trim());
      }
      ports.setTasks((current) => appendTask(current, session));
      ports.setPrimaryRoute({ kind: "task", taskId: session.id });
      ports.closeDrawer();
      // The new task starts with no capability run of its own; other tasks keep
      // theirs (per-task isolation), so no global capability reset here.
      return session.id;
    },
  };
}
