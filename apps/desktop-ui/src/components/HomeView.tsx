import { Button } from "@sapphirus/ui";
import { Send } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import type { WorkspaceProjection } from "../lib/hostClient";
import { OpenCodeIcon } from "./OpenCodeIcon";

export interface HomeTaskSummary {
  readonly id: string;
  readonly title: string;
  readonly updatedAt: string;
}

export interface HomeViewProps {
  readonly activeWorkspaceId: string | null;
  readonly canOpenFolder: boolean;
  readonly composerDisabled: boolean;
  readonly onActivateWorkspace: (workspaceId: string) => void;
  readonly onOpenWorkspaceManager: () => void;
  readonly onOpenWorkspace: () => void;
  readonly onSelectTask: (taskId: string) => void;
  readonly onSubmitIntent: (intent: string) => void;
  readonly onNewTask: () => void;
  readonly recentTasks: readonly HomeTaskSummary[];
  /// Tasks the user closed. They keep their transcripts, so Home is where they
  /// come back from — without this list, closing would be indistinguishable
  /// from deleting.
  readonly closedTasks: readonly HomeTaskSummary[];
  readonly onReopenTask: (taskId: string) => void;
  /// Re-runs host verification. Supplied only when the host is unavailable.
  /// Home is where a failed bootstrap always lands, so this is the retry the
  /// user can actually reach; without it `unavailable` is terminal until the
  /// app is restarted.
  readonly onRetryHost?: (() => void) | undefined;
  readonly statusHint: string;
  readonly workspaces: readonly WorkspaceProjection[];
  readonly workspaceStatusLabel: string;
}

/**
 * The landing view: projects on the left, a composer and recent tasks on the
 * right. Submitting routes into a fresh task.
 */
export function HomeView({
  activeWorkspaceId,
  canOpenFolder,
  composerDisabled,
  onActivateWorkspace,
  onOpenWorkspaceManager,
  onOpenWorkspace,
  onSelectTask,
  onSubmitIntent,
  onNewTask,
  recentTasks,
  closedTasks,
  onReopenTask,
  onRetryHost,
  statusHint,
  workspaces,
  workspaceStatusLabel,
}: HomeViewProps) {
  const [intent, setIntent] = useState("");
  const inputRef = useRef<HTMLInputElement | null>(null);
  const trimmed = intent.trim();

  useEffect(() => {
    // Land with the composer focused so typing starts a task immediately
    // (and stray activation keys from app launch are absorbed harmlessly).
    inputRef.current?.focus();
  }, []);

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!composerDisabled && trimmed.length > 0) {
      onSubmitIntent(trimmed);
      setIntent("");
    }
  };

  return (
    <main
      aria-label="Home"
      className="home-view home-view--opencode-v2"
    >
      <h1 className="sr-only">Forge workspace</h1>
      <div className="home-view__overview">
        <section className="home-view__projects">
          <header className="home-view__section-header">
            <h2>Projects</h2>
            <button
              aria-label="Open workspace"
              disabled={!canOpenFolder}
              onClick={onOpenWorkspace}
              type="button"
            >
              <OpenCodeIcon name="folder-add-left" size={14} />
            </button>
          </header>
          {workspaces.length === 0 ? (
            <button className="home-view__empty-action" onClick={onOpenWorkspace} type="button">
              <OpenCodeIcon name="folder" size={14} />
              Open a local workspace
            </button>
          ) : (
            <ul aria-label="Projects" className="home-view__project-list">
              {workspaces.map((workspace) => (
                <li key={workspace.workspaceId}>
                  <button
                    aria-current={workspace.workspaceId === activeWorkspaceId ? "page" : undefined}
                    onClick={() => onActivateWorkspace(workspace.workspaceId)}
                    type="button"
                  >
                    <OpenCodeIcon name="folder" size={14} />
                    <span>{workspace.displayName}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
          <div className="home-view__project-actions">
            <button onClick={onOpenWorkspaceManager} type="button">
              <OpenCodeIcon name="settings-gear" size={14} />
              Manage workspaces
            </button>
            {onRetryHost ? (
              <button onClick={onRetryHost} type="button">
                <OpenCodeIcon name="reset" size={14} />
                Retry host
              </button>
            ) : null}
            <span>{workspaceStatusLabel}</span>
          </div>
        </section>

        <section className="home-view__tasks">
          <form aria-label="Start a task" className="home-view__composer" onSubmit={submit}>
            <input
              aria-label="Describe your intent"
              autoComplete="off"
              className="home-view__input"
              disabled={composerDisabled}
              onChange={(event) => setIntent(event.target.value)}
              placeholder="Ask Forge to inspect, plan, or review…"
              ref={inputRef}
              spellCheck={false}
              type="text"
              value={intent}
            />
            <div className="home-view__composer-row">
              <span className="home-view__hint">{statusHint}</span>
              <Button
                aria-label="Start task"
                isDisabled={composerDisabled || trimmed.length === 0}
                size="small"
                type="submit"
                variant="primary"
              >
                <Send aria-hidden="true" size={14} />
                Start task
              </Button>
            </div>
          </form>

          <header className="home-view__section-header home-view__section-header--tasks">
            <h2>Recent tasks</h2>
            <button onClick={onNewTask} type="button">
              <OpenCodeIcon name="edit" size={14} />
              New task
            </button>
          </header>
          {recentTasks.length === 0 ? (
            <p className="home-view__task-empty">
              No tasks yet. Describe what you want above, or start an empty one
              with New task.
            </p>
          ) : (
            <ul aria-label="Recent tasks" className="home-view__task-list">
              {recentTasks.map((task) => (
                <li key={task.id}>
                  <button onClick={() => onSelectTask(task.id)} type="button">
                    <OpenCodeIcon name="edit" size={14} />
                    <span>{task.title}</span>
                    <time>{task.updatedAt}</time>
                  </button>
                </li>
              ))}
            </ul>
          )}

          {closedTasks.length === 0 ? null : (
            <>
              <header className="home-view__section-header home-view__section-header--tasks">
                <h2>Closed tasks</h2>
              </header>
              <ul aria-label="Closed tasks" className="home-view__task-list home-view__task-list--closed">
                {closedTasks.map((task) => (
                  <li key={task.id}>
                    <button
                      aria-label={`Reopen task ${task.title}`}
                      onClick={() => onReopenTask(task.id)}
                      type="button"
                    >
                      <OpenCodeIcon name="edit" size={14} />
                      <span>{task.title}</span>
                      <time>Closed</time>
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </section>
      </div>
    </main>
  );
}
