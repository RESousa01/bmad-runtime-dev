import { Button } from "@sapphirus/ui";
import { Send } from "lucide-react";
import { useEffect, useRef, useState, type FormEvent } from "react";
import type { WorkspaceProjection } from "../lib/hostClient";
import { homeEmptyState, type HomeHostKind } from "./home/homeEmptyState";
import { ProjectRail } from "./home/ProjectRail";
import { TaskIndex } from "./home/TaskIndex";
import "./home/home-index.css";

export interface HomeTaskSummary {
  readonly id: string;
  readonly title: string;
  readonly updatedAt: string;
}

export interface HomeViewProps {
  readonly activeWorkspaceId: string | null;
  readonly canOpenFolder: boolean;
  readonly composerDisabled: boolean;
  /**
   * What the host is doing, which is the only thing that can distinguish "you
   * have not opened a folder yet" from "Forge cannot reach the process that
   * would open one".
   *
   * Optional so the shell can be wired separately, and defaulted to `ready`
   * because that is the state in which the rest of Home's copy is already
   * correct: an unwired Home says "open a folder", which is right whenever the
   * host can open one and merely imprecise while it is starting up.
   */
  readonly hostKind?: HomeHostKind | undefined;
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
 * The landing surface, as an index rather than an introduction.
 *
 * A user opening Forge is almost always resuming, so Home answers the three
 * questions that resuming needs — which projects exist, which one is live, what
 * was I last doing — in one screen, with no disclosure to open and no card to
 * read past. The projects rail and the task index are separate components
 * because they are separate questions; this file owns only the composer, the
 * layout, and the decision about what an empty index means.
 *
 * Tasks are listed flat rather than nested under their project on purpose. A
 * `TaskRecord` carries no workspace id — the store keeps one list for the whole
 * app — so grouping the rows under the rail would invent a relationship the
 * data does not have, and the grouping would start lying the moment the user
 * switched projects and the same rows reappeared under a different heading.
 */
export function HomeView({
  activeWorkspaceId,
  canOpenFolder,
  composerDisabled,
  hostKind = "ready",
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
        <ProjectRail
          activeWorkspaceId={activeWorkspaceId}
          canOpenFolder={canOpenFolder}
          onActivateWorkspace={onActivateWorkspace}
          onOpenWorkspace={onOpenWorkspace}
          onOpenWorkspaceManager={onOpenWorkspaceManager}
          onRetryHost={onRetryHost}
          workspaces={workspaces}
          workspaceStatusLabel={workspaceStatusLabel}
        />

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
            {/* 3a spends its one accent on the consequential decision, which
                on Home is not "start a task" -- it is Apply changes, later.
                So this is a quiet send glyph on the readout row, the way the
                prototype draws it, and the hint carries the state. */}
            <div className="home-view__composer-row">
              <span className="home-view__hint">{statusHint}</span>
              <Button
                aria-label="Start task"
                isDisabled={composerDisabled || trimmed.length === 0}
                size="small"
                type="submit"
                variant="quiet"
              >
                <Send aria-hidden="true" size={14} />
              </Button>
            </div>
          </form>

          <TaskIndex
            closedTasks={closedTasks}
            emptyState={homeEmptyState({
              hostKind,
              projectCount: workspaces.length,
              taskCount: recentTasks.length,
            })}
            onNewTask={onNewTask}
            onOpenWorkspace={onOpenWorkspace}
            onReopenTask={onReopenTask}
            onSelectTask={onSelectTask}
            recentTasks={recentTasks}
          />
        </section>
      </div>
    </main>
  );
}
