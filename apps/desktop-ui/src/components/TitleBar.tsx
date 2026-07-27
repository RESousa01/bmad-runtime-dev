import { Button } from "@sapphirus/ui";
import { CircleUserRound, Maximize2, Minus, X } from "lucide-react";
import { useState } from "react";
import { brandIdentity } from "../lib/brand";
import { performWindowAction } from "../lib/windowActions";
import { OpenCodeIcon } from "./OpenCodeIcon";

export interface TitleBarTaskTab {
  readonly id: string;
  readonly title: string;
}

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
  const visibleTaskTabs = taskTabs
    ?? (taskTitle === undefined ? [] : [{ id: "current-task", title: taskTitle }]);
  const selectedTaskId = activeTaskId ?? visibleTaskTabs[0]?.id;

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
        {visibleTaskTabs.length === 0 ? null : (
          // A `tablist` in the APG pattern is deliberately not focusable: the
          // roving `tabindex` lives on the tabs, and each one below carries it.
          // Making the container focusable would add a tab stop that lands on
          // nothing and put the tabs one extra key press away.
          // eslint-disable-next-line jsx-a11y/interactive-supports-focus
          <div
            aria-label="Open tasks"
            className="title-strip__tabs"
            onKeyDown={(event) => {
              // Delete on the focused tab closes it. This is the ARIA pattern
              // for closable tabs and the only way to reach the close action
              // from the keyboard without putting a second tab stop inside the
              // roving tabindex.
              if (event.key === "Delete" && onCloseTask && selectedTaskId !== undefined) {
                const focused = visibleTaskTabs.find((task) => task.id === selectedTaskId);
                if (focused) {
                  event.preventDefault();
                  onCloseTask(focused.id);
                  return;
                }
              }
              // Roving tabindex means only the active tab is reachable by Tab,
              // so without arrow handling the other tasks could not be reached
              // from the keyboard at all.
              const keys = ["ArrowLeft", "ArrowRight", "Home", "End"];
              if (!keys.includes(event.key)) return;
              const current = visibleTaskTabs.findIndex((task) => task.id === selectedTaskId);
              const last = visibleTaskTabs.length - 1;
              const index = current < 0 ? 0 : current;
              const next =
                event.key === "Home"
                  ? 0
                  : event.key === "End"
                    ? last
                    : event.key === "ArrowLeft"
                      ? (index === 0 ? last : index - 1)
                      : (index === last ? 0 : index + 1);
              const target = visibleTaskTabs[next];
              if (!target) return;
              event.preventDefault();
              onSelectTask?.(target.id);
              const tabs = event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]');
              tabs[next]?.focus();
            }}
            role="tablist"
          >
            {visibleTaskTabs.map((task) => {
              const active = task.id === selectedTaskId;
              return (
                <button
                  aria-selected={active}
                  className="title-strip__tab"
                  data-active={active || undefined}
                  key={task.id}
                  onClick={() => onSelectTask?.(task.id)}
                  role="tab"
                  tabIndex={active ? 0 : -1}
                  title={task.title}
                  type="button"
                >
                  <OpenCodeIcon name="edit" size={14} />
                  <span>{task.title}</span>
                  {/*
                    A tablist may contain nothing but tabs, so the close
                    affordance cannot be a sibling button — axe rejects that
                    outright — and a nested <button> would be invalid HTML.
                    It is therefore a pointer-only target inside the tab, and
                    the keyboard route to the same action is Delete on the
                    focused tab (handled by the tablist), the "Close task"
                    control in the task header, and the command palette.
                  */}
                  {onCloseTask === undefined ? null : (
                    <span
                      aria-hidden="true"
                      className="title-strip__tab-close"
                      onClick={(event) => {
                        // Without this the tab's own click also fires and the
                        // shell selects the task it is closing.
                        event.stopPropagation();
                        onCloseTask(task.id);
                      }}
                      title={`Close task ${task.title}`}
                    >
                      <X size={12} strokeWidth={2} />
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        )}
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
