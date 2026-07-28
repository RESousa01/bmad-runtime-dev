/**
 * Which task tabs fit on the title bar row, and which have to go in the menu.
 *
 * FE-18. The tab strip shares one 36px row with the app menu, the home and
 * new-task controls, the settings and account buttons and the window controls,
 * and the window is resizable -- so "more tabs than fit" is the ordinary case
 * rather than the edge one. What the strip did about it before was scroll
 * sideways with the scrollbar hidden, which is the worst of the answers
 * available: the tabs outside the viewport are gone with nothing on screen
 * admitting it, and there is no keyboard route to them at all.
 *
 * The decision lives here, apart from the component that renders it, because
 * that is the only place it can be tested. jsdom reports zero for `offsetWidth`,
 * `clientWidth` and every rectangle, so a fit computed inside a render can only
 * ever be exercised in the degenerate "nothing has been measured" case -- the
 * one case whose answer is "show everything". With the widths as arguments,
 * every case is reachable and the two rules below can be held to.
 */

export interface TabFitInput {
  /**
   * The tab the user is looking at. Any index outside `widths` means there is
   * no active task, which is what the home view looks like.
   */
  readonly activeIndex: number;
  /**
   * Room the row may fill, in CSS pixels.
   *
   * Zero means "not measured yet", never "no room". The strip is measured
   * after it has rendered, so the first paint of every session and of every
   * newly opened task asks this question before there is an answer.
   */
  readonly available: number;
  /** What the overflow control costs once it is on screen. */
  readonly overflowWidth: number;
  /** Outer width of each tab, in strip order, including the row gap. */
  readonly widths: readonly number[];
}

export interface TabFit {
  /** Indices of the tabs that render in the strip, in strip order. */
  readonly visible: readonly number[];
  /** Indices of the tabs that render in the overflow menu, in strip order. */
  readonly overflow: readonly number[];
}

/**
 * Splits the tabs into the ones the strip shows and the ones the menu lists.
 *
 * Two rules, and each exists because of a way this fails:
 *
 * The active tab never overflows. Hiding the task the user is looking at is
 * not a graceful degradation, it is a lie about where they are -- so the
 * active tab's width is spent before any other tab may claim room, even when
 * the active tab is last in the strip and every earlier tab would have fitted
 * without it.
 *
 * The overflow control is charged for. It stands in the same row as the tabs,
 * so a budget that ignores it overruns the strip by exactly the width of the
 * control that exists to stop the strip overrunning.
 */
export function fitTabs({ activeIndex, available, overflowWidth, widths }: TabFitInput): TabFit {
  const everyTab = widths.map((_, index) => index);
  // An unmeasured strip reports zero, and so does every strip under jsdom.
  // Reading that as "no room" would sweep every tab into the menu on first
  // paint, and since a tab in the menu is never laid out it would never be
  // measured -- the strip would settle there and stay empty.
  if (available <= 0) return { overflow: [], visible: everyTab };

  const total = widths.reduce((sum, width) => sum + width, 0);
  if (total <= available) return { overflow: [], visible: everyTab };

  const budget = available - overflowWidth;
  const active = activeIndex >= 0 && activeIndex < widths.length ? activeIndex : -1;
  let used = active < 0 ? 0 : (widths[active] ?? 0);
  const visible: number[] = [];
  const overflow: number[] = [];
  let full = false;
  for (const index of everyTab) {
    if (index === active) {
      visible.push(index);
      continue;
    }
    const width = widths[index] ?? 0;
    // Everything after the first tab that does not fit overflows too, rather
    // than a narrow tab slipping into the room a wide one could not use. Tabs
    // vanishing from the middle of the strip while their neighbours stay reads
    // as corruption, and the order the user opened them in stops being legible.
    if (full || used + width > budget) {
      full = true;
      overflow.push(index);
      continue;
    }
    used += width;
    visible.push(index);
  }
  return { overflow, visible };
}
