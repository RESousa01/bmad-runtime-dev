import { Button } from "@sapphirus/ui";
import { CircleUserRound, Maximize2, Minus, X } from "lucide-react";
import { useState } from "react";
import { brandIdentity } from "../lib/brand";
import { performWindowAction } from "../lib/windowActions";
import { OpenCodeIcon } from "./OpenCodeIcon";
import { TaskTabStrip, type TaskTab } from "./title-bar/TaskTabStrip";

/// The shell's name for a tab, kept as the exported one so the call sites in
/// `app/` do not have to move for FE-18.
export type TitleBarTaskTab = TaskTab;

export interface TitleBarProps {
  activeTaskId?: string | undefined;
  isInert?: boolean;
  onMenu?: (() => void) | undefined;
  onHome?: (() => void) | undefined;
  onNewTask?: (() => void) | undefined;
  /// Closes a task from its tab. Closing keeps the transcript; deleting is a
  /// separate, confirmed action on the task itself.
  onCloseTask?: ((id: string) => void) | undefined;
  onOpenAccount?: (() => void) | undefined;
  onOpenSettings?: (() => void) | undefined;
  onSelectTask?: ((id: string) => void) | undefined;
  taskTitle?: string | undefined;
  taskTabs?: readonly TitleBarTaskTab[] | undefined;
}

export function TitleBar({
  activeTaskId,
  isInert = false,
  onMenu,
  onHome,
  onNewTask,
  onCloseTask,
  onOpenAccount,
  onOpenSettings,
  onSelectTask,
  taskTitle,
  taskTabs,
}: TitleBarProps) {
  const [windowActionError, setWindowActionError] = useState("");
  const openTasks = taskTabs
    ?? (taskTitle === undefined ? [] : [{ id: "current-task", title: taskTitle }]);
  const selectedTaskId = activeTaskId ?? openTasks[0]?.id;

  function invokeWindowAction(action: Parameters<typeof performWindowAction>[0]) {
    setWindowActionError("");
    void performWindowAction(action).catch(() => {
      setWindowActionError("The window action is unavailable.");
    });
  }

  return (
    <header
      aria-label={`${brandIdentity.displayName} application`}
      className="title-bar title-bar--opencode-v2"
      data-tauri-drag-region
      inert={isInert}
    >
      <div className="title-strip" data-tauri-drag-region>
        {onMenu === undefined ? null : (
          <Button
            aria-label="Open app menu"
            className="title-strip__button"
            onPress={onMenu}
            size="icon"
            variant="quiet"
          >
            <OpenCodeIcon name="menu" />
          </Button>
        )}
        {onHome === undefined ? null : (
          <Button
            aria-label="Show tasks overview"
            className="title-strip__button"
            onPress={onHome}
            size="icon"
            variant="quiet"
          >
            <OpenCodeIcon name="grid-plus" />
          </Button>
        )}
        <TaskTabStrip
          activeTaskId={selectedTaskId}
          onCloseTask={onCloseTask}
          onSelectTask={onSelectTask}
          tasks={openTasks}
        />
        {onNewTask === undefined ? null : (
          <Button
            aria-label="New task"
            className="title-strip__button"
            onPress={onNewTask}
            size="icon"
            variant="quiet"
          >
            <OpenCodeIcon name="plus" />
          </Button>
        )}
      </div>
      <div className="title-strip__end">
        {onOpenSettings === undefined ? null : (
          <Button
            aria-label="Settings"
            className="title-strip__button"
            onPress={onOpenSettings}
            size="icon"
            variant="quiet"
          >
            <OpenCodeIcon name="settings-gear" />
          </Button>
        )}
        {onOpenAccount === undefined ? null : (
          <Button
            aria-label="Account"
            className="title-strip__button"
            onPress={onOpenAccount}
            size="icon"
            variant="quiet"
          >
            <CircleUserRound aria-hidden="true" size={15} strokeWidth={1.7} />
          </Button>
        )}
      </div>
      <div aria-label="Window controls" className="window-controls title-bar__controls">
        <Button
          aria-label="Minimize window"
          className="window-control"
          onPress={() => invokeWindowAction("minimize")}
          size="icon"
          variant="quiet"
        >
          <Minus aria-hidden="true" size={15} strokeWidth={1.7} />
        </Button>
        <Button
          aria-label="Maximize or restore window"
          className="window-control"
          onPress={() => invokeWindowAction("toggleMaximize")}
          size="icon"
          variant="quiet"
        >
          <Maximize2 aria-hidden="true" size={13} strokeWidth={1.7} />
        </Button>
        <Button
          aria-label="Close window"
          className="window-control window-control--close"
          onPress={() => invokeWindowAction("close")}
          size="icon"
          variant="quiet"
        >
          <X aria-hidden="true" size={16} strokeWidth={1.7} />
        </Button>
      </div>
      <span aria-live="polite" className="sr-only">{windowActionError}</span>
    </header>
  );
}
