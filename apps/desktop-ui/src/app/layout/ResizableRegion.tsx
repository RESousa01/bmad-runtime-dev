/* eslint-disable jsx-a11y/no-noninteractive-tabindex --
   The resize handle is a focusable `separator` carrying aria-valuenow, which
   is the WAI-ARIA window splitter pattern; the tabindex is what makes it
   operable, since a drag-only handle cannot be resized from a keyboard.
   jsx-a11y treats `separator` as non-interactive and does not model the
   splitter case, so the rule is exempted here rather than the pattern being
   abandoned for one the rule happens to recognise. */
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import {
  clampToBounds,
  loadLayout,
  resolveRegionSize,
  saveLayout,
  type RegionBounds,
} from "./layoutPersistence";

export interface ResizableRegionProps {
  /** Stable key for persistence. Changing it discards the stored size. */
  regionId: string;
  bounds: RegionBounds;
  /** Names the separator for assistive technology. */
  label: string;
  /** Which edge the handle sits on. */
  edge?: "inline-start" | "inline-end";
  children: ReactNode;
  className?: string;
}

/** How far one arrow press moves the divider. Shift multiplies by this again. */
const KEYBOARD_STEP = 16;

/**
 * A region the user can resize, with the size remembered.
 *
 * The separator is a real `separator` role with `aria-valuenow`, and it is
 * operable from the keyboard. A drag-only handle is not resizable to anyone
 * using a keyboard or a screen reader, and a workbench whose panels can only
 * be sized with a mouse is one whose layout they cannot change at all.
 *
 * The clamp is the same function the loader uses. Two implementations of a
 * minimum is how a panel ends up usable while dragging and unusable after a
 * reload.
 */
export function ResizableRegion({
  regionId,
  bounds,
  label,
  edge = "inline-start",
  children,
  className = "",
}: ResizableRegionProps) {
  const [storedSize, setStoredSize] = useState(() =>
    resolveRegionSize(loadLayout(), regionId, bounds));
  const [dragging, setDragging] = useState(false);

  /*
   * Clamped during render, not in an effect.
   *
   * Bounds change between builds, so a size held in state can stop being legal
   * without anything having happened. Correcting that in an effect would set
   * state during render's commit and cascade a second render for a value that
   * is simply derived -- so it is derived.
   */
  const size = clampToBounds(storedSize, bounds);
  const setSize = setStoredSize;
  // The pointerup handler is registered once per drag and would otherwise close
  // over the size at drag start. Synced in an effect, because writing a ref
  // during render is a render side effect.
  const sizeRef = useRef(size);
  useEffect(() => { sizeRef.current = size; }, [size]);

  const commit = useCallback((next: number) => {
    const clamped = clampToBounds(next, bounds);
    setSize(clamped);
    const layout = loadLayout();
    saveLayout({ sizes: { ...layout.sizes, [regionId]: clamped } });
  }, [bounds, regionId, setSize]);

  useEffect(() => {
    if (!dragging) return undefined;
    const onMove = (event: PointerEvent) => {
      const fromEdge = edge === "inline-start"
        ? window.innerWidth - event.clientX
        : event.clientX;
      setSize(clampToBounds(fromEdge, bounds));
    };
    // The size is only written to storage when the drag ends, so a drag across
    // the window is one stored value rather than a hundred.
    const onUp = () => {
      setDragging(false);
      commit(sizeRef.current);
    };
    document.addEventListener("pointermove", onMove);
    document.addEventListener("pointerup", onUp);
    return () => {
      document.removeEventListener("pointermove", onMove);
      document.removeEventListener("pointerup", onUp);
    };
  }, [bounds, commit, dragging, edge, setSize]);

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? KEYBOARD_STEP * 4 : KEYBOARD_STEP;
    const grow = edge === "inline-start" ? "ArrowLeft" : "ArrowRight";
    const shrink = edge === "inline-start" ? "ArrowRight" : "ArrowLeft";
    if (event.key === grow) {
      event.preventDefault();
      commit(size + step);
    } else if (event.key === shrink) {
      event.preventDefault();
      commit(size - step);
    } else if (event.key === "Home") {
      event.preventDefault();
      commit(bounds.min);
    } else if (event.key === "End") {
      event.preventDefault();
      commit(bounds.max);
    }
  };

  return (
    <div
      className={["resizable-region", className].filter(Boolean).join(" ")}
      data-region={regionId}
      style={{ inlineSize: `${size}px` }}
    >
      <div
        aria-label={label}
        aria-orientation="vertical"
        aria-valuemax={bounds.max}
        aria-valuemin={bounds.min}
        aria-valuenow={size}
        className={`resizable-region__handle resizable-region__handle--${edge}`}
        onKeyDown={onKeyDown}
        onPointerDown={(event) => {
          event.preventDefault();
          setDragging(true);
        }}
        role="separator"
        tabIndex={0}
      />
      <div className="resizable-region__content">{children}</div>
    </div>
  );
}
