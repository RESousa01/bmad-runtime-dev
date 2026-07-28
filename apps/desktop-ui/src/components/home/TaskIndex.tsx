import { OpenCodeIcon } from "../OpenCodeIcon";
import type { HomeEmptyState } from "./homeEmptyState";

export interface TaskIndexEntry {
  readonly id: string;
  readonly title: string;
  readonly updatedAt: string;
}

export interface TaskIndexProps {
  /// Tasks the user closed. They keep their transcripts, so this is where they
  /// come back from — without the section, closing would be indistinguishable
  /// from deleting.
  readonly closedTasks: readonly TaskIndexEntry[];
  /// Why there is nothing to list, or `null` when there is. Decided by
  /// `homeEmptyState` rather than here, so the reason is stated once.
  readonly emptyState: HomeEmptyState | null;
  readonly onNewTask: () => void;
  readonly onOpenWorkspace: () => void;
  readonly onReopenTask: (taskId: string) => void;
  readonly onSelectTask: (taskId: string) => void;
  readonly recentTasks: readonly TaskIndexEntry[];
}

/**
 * What the user was doing, newest first, one press from being back in it.
 *
 * Rows are ordered by the caller and never re-sorted here. `updatedAt` is a
 * display string the shell chooses, not a timestamp, so sorting on it would
 * order the index by the alphabet and quietly break "most recent first" the
 * first time the wording changed.
 *
 * Every row carries an explicit accessible name. Left to its contents a row
 * announces as "Trace the boot sequence Now", which reads as a fragment rather
 * than as the action pressing it performs — and the closed rows next to it,
 * which look identical, do something else entirely.
 */
export function TaskIndex({
  closedTasks,
  emptyState,
  onNewTask,
  onOpenWorkspace,
  onReopenTask,
  onSelectTask,
  recentTasks,
}: TaskIndexProps) {
  // An empty index and a reason for it are two separate facts, and only their
  // conjunction is a notice. A caller that hands over rows *and* a reason gets
  // the rows: the list is the evidence, the reason is the explanation, and an
  // explanation is not allowed to hide the thing it would be explaining.
  const notice = recentTasks.length === 0 ? emptyState : null;

  return (
    <>
      <header className="home-view__section-header home-view__section-header--tasks">
        <h2>Recent tasks</h2>
        <button onClick={onNewTask} type="button">
          <OpenCodeIcon name="edit" size={14} />
          New task
        </button>
      </header>
      {notice === null ? (
        <ul aria-label="Recent tasks" className="home-view__task-list">
          {recentTasks.map((task) => (
            <li key={task.id}>
              <button
                aria-label={`Resume task ${task.title}`}
                onClick={() => onSelectTask(task.id)}
                type="button"
              >
                <span>{task.title}</span>
                <time>{task.updatedAt}</time>
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <div className="home-index__empty">
          <p className="home-view__task-empty">{notice.message}</p>
          {/* The action is offered only where it can succeed. A host that
              cannot list projects cannot open one either, so the folder picker
              appears for the one reason that a folder actually fixes. */}
          {notice.reason === "no-project" ? (
            <button className="home-view__empty-action" onClick={onOpenWorkspace} type="button">
              <OpenCodeIcon name="folder" size={14} />
              Open a local workspace
            </button>
          ) : null}
        </div>
      )}

      {closedTasks.length === 0 ? null : (
        <>
          <header className="home-view__section-header home-view__section-header--tasks">
            <h2>Closed tasks</h2>
          </header>
          <ul
            aria-label="Closed tasks"
            className="home-view__task-list home-view__task-list--closed"
          >
            {closedTasks.map((task) => (
              <li key={task.id}>
                <button
                  aria-label={`Reopen task ${task.title}`}
                  onClick={() => onReopenTask(task.id)}
                  type="button"
                >
                  <span>{task.title}</span>
                  <time>Closed</time>
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </>
  );
}
