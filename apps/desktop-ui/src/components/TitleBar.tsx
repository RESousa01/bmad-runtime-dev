import { Button } from "@sapphirus/ui";
import { LayoutGrid, Maximize2, Menu, Minus, Plus, SquarePen, X } from "lucide-react";
import { useState } from "react";
import { performWindowAction } from "../lib/windowActions";
import { BrandMark } from "./BrandMark";

export interface TitleBarProps {
  isInert?: boolean;
  onMenu?: (() => void) | undefined;
  onHome?: (() => void) | undefined;
  onNewTask?: (() => void) | undefined;
  taskTitle?: string | undefined;
}

export function TitleBar({
  isInert = false,
  onMenu,
  onHome,
  onNewTask,
  taskTitle,
}: TitleBarProps) {
  const [windowActionError, setWindowActionError] = useState("");

  function invokeWindowAction(action: Parameters<typeof performWindowAction>[0]) {
    setWindowActionError("");
    void performWindowAction(action).catch(() => {
      setWindowActionError("The window action is unavailable.");
    });
  }

  return (
    <header
      aria-label="Sapphirus application"
      className="title-bar"
      data-tauri-drag-region
      inert={isInert}
    >
      <div className="brand-lockup title-bar__brand" data-tauri-drag-region>
        <BrandMark size={23} />
        <span>Sapphirus</span>
      </div>
      <div className="title-strip" data-tauri-drag-region>
        {onMenu === undefined ? null : (
          <Button
            aria-label="Open app menu"
            className="title-strip__button"
            onPress={onMenu}
            size="icon"
            variant="quiet"
          >
            <Menu aria-hidden="true" size={15} strokeWidth={1.7} />
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
            <LayoutGrid aria-hidden="true" size={15} strokeWidth={1.7} />
          </Button>
        )}
        {taskTitle === undefined ? null : (
          <span className="title-strip__tab">
            <SquarePen aria-hidden="true" size={13} strokeWidth={1.7} />
            <span>{taskTitle}</span>
          </span>
        )}
        {onNewTask === undefined ? null : (
          <Button
            aria-label="Start a new task"
            className="title-strip__button"
            onPress={onNewTask}
            size="icon"
            variant="quiet"
          >
            <Plus aria-hidden="true" size={15} strokeWidth={1.7} />
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
