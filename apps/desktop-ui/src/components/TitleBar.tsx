import { Button } from "@sapphirus/ui";
import { Maximize2, Minus, X } from "lucide-react";
import { useState } from "react";
import { performWindowAction } from "../lib/windowActions";
import { BrandMark } from "./BrandMark";

export function TitleBar({ isInert = false }: { isInert?: boolean }) {
  const [windowActionError, setWindowActionError] = useState("");

  function invokeWindowAction(action: Parameters<typeof performWindowAction>[0]) {
    setWindowActionError("");
    void performWindowAction(action).catch(() => {
      setWindowActionError("The window action is unavailable.");
    });
  }

  return (
    <header className="title-bar" data-tauri-drag-region inert={isInert}>
      <div className="brand-lockup" data-tauri-drag-region>
        <BrandMark size={23} />
        <span>Sapphirus</span>
      </div>
      <div aria-label="Window controls" className="window-controls">
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
