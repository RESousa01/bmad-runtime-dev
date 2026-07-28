/**
 * The task tab strip, and the menu holding the tabs that did not fit.
 *
 * FE-18. `fitTabs` decides *what* overflows; this decides how the strip finds
 * the numbers to give it, and what the menu behaves like once there is one.
 *
 * Where the numbers come from
 * ---------------------------
 * From the DOM, after the fact. A tab's width depends on its label, the label
 * is auto-generated from the first message, and the row it lives in resizes
 * with the window -- so there is no count of tabs that is right, and anything
 * hard-coded is wrong at the next resize.
 *
 * That means a tab has to be laid out before its width is known, and a tab in
 * the menu is not laid out at all. The two facts together would deadlock: an
 * overflowed tab could never be re-measured, so it could never come back. What
 * breaks the deadlock is that widths are remembered per task rather than read
 * fresh each pass, and a task whose width is not known yet counts as zero --
 * which always fits, so it renders in the strip once, which is exactly the
 * frame it gets measured in.
 *
 * Under jsdom every one of those readings is zero, which `fitTabs` treats as
 * "not measured" and answers with "show everything". That keeps the component
 * honest in tests but leaves overflow unreachable through the DOM, so the
 * measurement is injectable -- see `measurement` below.
 */

import { X } from "lucide-react";
import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";

import { useDeferredFocusReturn } from "../../app/focus/deferredFocusReturn";
import { OpenCodeIcon } from "../OpenCodeIcon";
import { fitTabs } from "./fitTabs";

export interface TaskTab {
  readonly id: string;
  readonly title: string;
}

/**
 * The layout numbers the strip normally reads from the DOM.
 *
 * Passing this in replaces the measurement wholesale. It exists because jsdom
 * has no layout: `offsetWidth` and `clientWidth` are zero for everything, so
 * without an injection point the only reachable state in a test is the one
 * where nothing overflows -- and the overflow is the entire feature.
 */
export interface TaskTabStripMeasurement {
  /** Room the tab row may fill, in CSS pixels. */
  readonly available: number;
  /** What the overflow control costs once it is on screen. */
  readonly overflowWidth: number;
  /** Outer width of each tab, by task id, including the row gap. */
  readonly tabWidths: Readonly<Record<string, number>>;
}

export interface TaskTabStripProps {
  readonly activeTaskId?: string | undefined;
  /// Closes a task from its tab. Closing keeps the transcript; deleting is a
  /// separate, confirmed action on the task itself.
  readonly onCloseTask?: ((id: string) => void) | undefined;
  readonly onSelectTask?: ((id: string) => void) | undefined;
  readonly tasks: readonly TaskTab[];
  /** Layout numbers to use instead of the DOM's. See the interface. */
  readonly measurement?: TaskTabStripMeasurement | undefined;
}

/**
 * What the overflow control is assumed to cost before it has ever been shown.
 *
 * The control has no width until it is on screen, and it is only on screen
 * once something overflows -- so the very first overflow has to be decided
 * against a guess. A generous one: guessing high overflows one tab early and
 * corrects on the next frame, guessing low overruns the row and pushes the
 * window controls off the edge, which is the defect this all exists to stop.
 */
const ASSUMED_OVERFLOW_WIDTH = 72;

const UNMEASURED: TaskTabStripMeasurement = {
  available: 0,
  overflowWidth: ASSUMED_OVERFLOW_WIDTH,
  tabWidths: {},
};

function sameMeasurement(a: TaskTabStripMeasurement, b: TaskTabStripMeasurement): boolean {
  if (a.available !== b.available || a.overflowWidth !== b.overflowWidth) return false;
  const keys = Object.keys(b.tabWidths);
  if (keys.length !== Object.keys(a.tabWidths).length) return false;
  return keys.every((key) => a.tabWidths[key] === b.tabWidths[key]);
}

/** A flex gap in pixels, or zero when the property is `normal` or absent. */
function rowGap(element: HTMLElement | null): number {
  if (element === null) return 0;
  return Number.parseFloat(getComputedStyle(element).columnGap) || 0;
}

/**
 * Every pixel of the title strip the tabs could have.
 *
 * Not `region.clientWidth`. The region does not grow to fill the row -- if it
 * did, the new-task control would be shoved away from the tabs and over to the
 * window buttons -- so its own width is what the tabs currently occupy, which
 * says nothing about how much more they could take. What is left for them is
 * the row minus the controls standing in it, and that is a question about the
 * row, so it is asked of the row.
 *
 * Zero under jsdom, where every one of these readings is zero, and zero is
 * what `fitTabs` reads as "not measured".
 */
function roomInRow(region: HTMLElement): number {
  const row = region.parentElement;
  if (row === null) return region.clientWidth;
  const style = getComputedStyle(row);
  const gap = Number.parseFloat(style.columnGap) || 0;
  // One gap per sibling is exactly the number of gaps in the row: n children
  // are separated by n - 1 gaps, and there are n - 1 siblings.
  let taken = (Number.parseFloat(style.paddingLeft) || 0)
    + (Number.parseFloat(style.paddingRight) || 0);
  for (const child of Array.from(row.children)) {
    if (child !== region && child instanceof HTMLElement) taken += child.offsetWidth + gap;
  }
  return Math.max(0, row.clientWidth - taken);
}

export function TaskTabStrip({
  activeTaskId,
  measurement,
  onCloseTask,
  onSelectTask,
  tasks,
}: TaskTabStripProps) {
  const regionRef = useRef<HTMLDivElement | null>(null);
  const tabsRef = useRef<HTMLDivElement | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  const tabRefs = useRef(new Map<string, HTMLButtonElement>());
  /// A tab that does not exist yet but must be focused as soon as it does.
  const pendingFocus = useRef<string | null>(null);
  const [observed, setObserved] = useState(UNMEASURED);
  const [menuOpen, setMenuOpen] = useState(false);
  const { arm, capture } = useDeferredFocusReturn(menuOpen);
  const menuId = useId();

  const layout = measurement ?? observed;
  const activeIndex = tasks.findIndex((task) => task.id === activeTaskId);
  const fit = fitTabs({
    activeIndex,
    available: layout.available,
    overflowWidth: layout.overflowWidth,
    widths: tasks.map((task) => layout.tabWidths[task.id] ?? 0),
  });
  const pick = (indices: readonly number[]): readonly TaskTab[] =>
    indices.flatMap((index) => {
      const task = tasks[index];
      return task === undefined ? [] : [task];
    });
  const visibleTasks = pick(fit.visible);
  const overflowTasks = pick(fit.overflow);
  const selectedTaskId = activeIndex < 0 ? undefined : activeTaskId;
  /// Everything a fresh measurement could read differently: which tabs are laid
  /// out, the labels they carry, and whether the overflow control is standing in
  /// the row taking space of its own. Deliberately the *visible* tabs -- a tab
  /// in the menu has no box to measure, and it re-enters this key on the render
  /// that puts it back in the strip.
  const layoutKey = JSON.stringify([
    overflowTasks.length > 0,
    visibleTasks.map((task) => [task.id, task.title]),
  ]);

  const remeasure = useCallback(() => {
    const region = regionRef.current;
    if (region === null) return;
    // The gap between tabs is part of what a tab costs the row. Leaving it out
    // is a steady under-count of two pixels per tab, and the strip overruns by
    // exactly that much once enough tasks are open.
    const gap = rowGap(tabsRef.current);
    const measured: Record<string, number> = {};
    for (const [id, tab] of tabRefs.current) measured[id] = tab.offsetWidth + gap;
    const available = roomInRow(region);
    const trigger = triggerRef.current;
    const overflowWidth = trigger === null ? null : trigger.offsetWidth + rowGap(region);
    setObserved((previous) => {
      const next: TaskTabStripMeasurement = {
        available,
        overflowWidth: overflowWidth ?? previous.overflowWidth,
        // Merged rather than replaced: only the tabs currently in the strip
        // were laid out, and forgetting the rest is what would let an
        // overflowed tab lose the width that keeps it in the menu.
        tabWidths: { ...previous.tabWidths, ...measured },
      };
      return sameMeasurement(previous, next) ? previous : next;
    });
  }, []);

  // Keyed on `layoutKey`, emphatically not run on every render.
  //
  // A layout effect that writes state schedules a second render before the
  // browser paints, which is exactly what is wanted when the strip's shape has
  // changed and exactly what must not happen otherwise: the title bar
  // re-renders on every keystroke in the composer, and an extra pre-paint
  // commit on each one both forces a synchronous reflow and reorders the
  // shell's effect flush. The recovery review restores focus from an effect
  // cleanup, and that restore landed on a detached node when this ran
  // unconditionally.
  //
  // `layoutKey` names every input a measurement could answer differently for;
  // width changes that leave it alone are the observer's job below.
  useLayoutEffect(() => {
    if (measurement !== undefined) return;
    remeasure();
  }, [layoutKey, measurement, remeasure]);

  // The row, not the region. The region is sized by its own content -- the
  // tabs that were chosen to fit -- so it sits at its natural width with the
  // spare room beside it, and dragging the window wider never touches it. The
  // row is the thing that tracks the window, and the row is what the available
  // width is computed from.
  useEffect(() => {
    const row = regionRef.current?.parentElement;
    if (measurement !== undefined || row == null) return undefined;
    // jsdom has no ResizeObserver. What is lost there is the response to a
    // resize that causes no render, and jsdom never resizes anything.
    if (typeof ResizeObserver === "undefined") return undefined;
    const observer = new ResizeObserver(remeasure);
    observer.observe(row);
    return () => observer.disconnect();
  }, [measurement, remeasure]);

  // Focus deferred to after the render that creates the tab. Nothing else can
  // work: the task the user picked from the menu has no tab at the moment they
  // pick it, and it is only guaranteed one because the active tab never
  // overflows.
  useEffect(() => {
    const id = pendingFocus.current;
    if (id === null) return;
    pendingFocus.current = null;
    const tab = tabRefs.current.get(id);
    if (tab !== undefined && tab.isConnected) tab.focus();
  });

  useEffect(() => {
    if (!menuOpen) return;
    menuRef.current?.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus();
  }, [menuOpen]);

  useEffect(() => {
    if (!menuOpen) return undefined;
    function closeOnOutsidePointer(event: PointerEvent) {
      if (regionRef.current?.contains(event.target as Node) === true) return;
      // Deliberately not armed: the user is pressing something else, and
      // yanking focus back to the trigger would take it off whatever they
      // just aimed at.
      setMenuOpen(false);
    }
    document.addEventListener("pointerdown", closeOnOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeOnOutsidePointer);
  }, [menuOpen]);

  function focusTask(id: string) {
    const tab = tabRefs.current.get(id);
    if (tab !== undefined && tab.isConnected) {
      tab.focus();
      return;
    }
    pendingFocus.current = id;
  }

  function dismissMenu() {
    arm(true);
    setMenuOpen(false);
  }

  function moveToTask(id: string) {
    onSelectTask?.(id);
    focusTask(id);
  }

  if (tasks.length === 0) return null;

  return (
    // The drag region is the reason the window can be moved at all, and this
    // element now covers the empty stretch of title bar that `.title-strip`
    // used to. Without the attribute here that stretch stops being a handle
    // and a maximised window becomes immovable.
    <div className="title-strip__tabs-region" data-tauri-drag-region ref={regionRef}>
      {visibleTasks.length === 0 ? null : (
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
              const focused = tasks.find((task) => task.id === selectedTaskId);
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
            // Over every open task, not just the ones on screen. The arrows
            // are the keyboard's whole answer to an overflowed strip: a task
            // in the menu is selected here, and because the active tab never
            // overflows it has a tab to be focused on by the next render.
            const current = tasks.findIndex((task) => task.id === selectedTaskId);
            const last = tasks.length - 1;
            const index = current < 0 ? 0 : current;
            const next =
              event.key === "Home"
                ? 0
                : event.key === "End"
                  ? last
                  : event.key === "ArrowLeft"
                    ? (index === 0 ? last : index - 1)
                    : (index === last ? 0 : index + 1);
            const target = tasks[next];
            if (!target) return;
            event.preventDefault();
            moveToTask(target.id);
          }}
          ref={tabsRef}
          role="tablist"
        >
          {visibleTasks.map((task) => {
            const active = task.id === selectedTaskId;
            const position = tasks.indexOf(task);
            return (
              <button
                // The strip holds only what fits, so a screen reader counting
                // the rendered tabs would announce a different number of open
                // tasks at every window width. The set is the open tasks.
                aria-posinset={position + 1}
                aria-selected={active}
                aria-setsize={tasks.length}
                className="title-strip__tab"
                data-active={active || undefined}
                key={task.id}
                onClick={() => onSelectTask?.(task.id)}
                ref={(element) => {
                  const tabs = tabRefs.current;
                  if (element === null) return undefined;
                  tabs.set(task.id, element);
                  return () => {
                    tabs.delete(task.id);
                  };
                }}
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
      {overflowTasks.length === 0 ? null : (
        <div className="title-strip__overflow">
          <button
            // The visible text is the accessible name's prefix on purpose:
            // WCAG 2.5.3 is what lets someone say "three more" and have it
            // match what they are looking at.
            aria-expanded={menuOpen}
            aria-haspopup="menu"
            aria-label={`${overflowTasks.length} more open ${overflowTasks.length === 1 ? "task" : "tasks"}`}
            className="title-strip__overflow-trigger"
            onClick={() => {
              if (menuOpen) {
                dismissMenu();
                return;
              }
              capture(triggerRef.current);
              setMenuOpen(true);
            }}
            onKeyDown={(event) => {
              if (event.key !== "ArrowDown" || menuOpen) return;
              event.preventDefault();
              capture(triggerRef.current);
              setMenuOpen(true);
            }}
            ref={triggerRef}
            type="button"
            {...(menuOpen ? { "aria-controls": menuId } : {})}
          >
            <span>{overflowTasks.length} more</span>
          </button>
          {!menuOpen ? null : (
            // Same reasoning as the tablist: a `menu` in the APG pattern is not
            // itself a tab stop, the roving `tabindex` lives on the items, and
            // focus is put on the first of them the moment this opens. Making
            // the container focusable would add a stop that lands on nothing.
            // eslint-disable-next-line jsx-a11y/interactive-supports-focus
            <div
              aria-label="More open tasks"
              className="title-strip__overflow-menu"
              id={menuId}
              onKeyDown={(event) => {
                // Tab is treated as a dismissal rather than a way out of the
                // menu. The alternative is letting focus walk into a menu the
                // user has stopped using, or losing it entirely when the menu
                // unmounts from under it.
                if (event.key === "Escape" || event.key === "Tab") {
                  event.preventDefault();
                  event.stopPropagation();
                  dismissMenu();
                  return;
                }
                const keys = ["ArrowDown", "ArrowUp", "Home", "End"];
                if (!keys.includes(event.key)) return;
                const items = [
                  ...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]'),
                ];
                const last = items.length - 1;
                const current = items.indexOf(document.activeElement as HTMLButtonElement);
                const index = current < 0 ? 0 : current;
                const next =
                  event.key === "Home"
                    ? 0
                    : event.key === "End"
                      ? last
                      : event.key === "ArrowUp"
                        ? (index === 0 ? last : index - 1)
                        : (index === last ? 0 : index + 1);
                event.preventDefault();
                items[next]?.focus();
              }}
              ref={menuRef}
              role="menu"
            >
              {overflowTasks.map((task) => (
                <button
                  className="title-strip__overflow-item"
                  key={task.id}
                  onClick={() => {
                    // Not armed for a return to the trigger: the picked task
                    // becomes the active one, the active task never overflows,
                    // so a tab for it exists on the next render and that tab
                    // is where focus belongs. The trigger may not even be
                    // there any more once the count drops.
                    arm(false);
                    setMenuOpen(false);
                    moveToTask(task.id);
                  }}
                  role="menuitem"
                  tabIndex={-1}
                  title={task.title}
                  type="button"
                >
                  <OpenCodeIcon name="edit" size={14} />
                  <span>{task.title}</span>
                </button>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
