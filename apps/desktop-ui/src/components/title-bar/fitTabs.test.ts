import { describe, expect, it } from "vitest";

import { fitTabs, type TabFitInput } from "./fitTabs";

/// Every case names its widths explicitly, which is the whole point of pulling
/// the decision out of the render: jsdom reports zero for every layout
/// property, so through the DOM only the "nothing measured yet" case exists.
function fit(input: Partial<TabFitInput> & Pick<TabFitInput, "widths">): ReturnType<typeof fitTabs> {
  return fitTabs({
    activeIndex: input.activeIndex ?? 0,
    available: input.available ?? 0,
    overflowWidth: input.overflowWidth ?? 60,
    widths: input.widths,
  });
}

describe("an unmeasured strip", () => {
  it("shows every tab rather than assuming there is no room", () => {
    // A tab in the menu is never laid out, so it is never measured. Reading
    // the zero of a first paint as "no room" would put every tab there and
    // leave the strip permanently empty.
    expect(fit({ available: 0, widths: [120, 120, 120] })).toEqual({
      overflow: [],
      visible: [0, 1, 2],
    });
  });

  it("treats a tab of unknown width as one that fits, so it gets measured", () => {
    // A task opened this frame has no measurement yet and arrives as zero. It
    // has to render in the strip once for the DOM to have a width to report.
    expect(fit({ available: 200, widths: [100, 0] })).toEqual({
      overflow: [],
      visible: [0, 1],
    });
  });
});

describe("deciding what fits", () => {
  it("keeps every tab when the last one ends exactly on the edge", () => {
    expect(fit({ available: 200, widths: [100, 100] })).toEqual({
      overflow: [],
      visible: [0, 1],
    });
  });

  it("overflows as soon as the tabs ask for one pixel more than there is", () => {
    // The boundary is the assertion. An overflow that triggers a tab late is
    // a strip that has already pushed the window controls off the edge by the
    // time it admits there is a problem.
    expect(fit({ available: 200, widths: [100, 101] })).toEqual({
      overflow: [1],
      visible: [0],
    });
  });

  it("charges the tabs for the overflow control's own width", () => {
    // 180 of tabs into 170 of room: the control appears, and it stands in the
    // same row. Spending the full 170 on tabs overruns the strip by exactly
    // the width of the thing that was meant to prevent the overrun.
    expect(fit({ available: 170, overflowWidth: 60, widths: [100, 60, 20] })).toEqual({
      overflow: [1, 2],
      visible: [0],
    });
  });

  it("overflows everything after the first tab that does not fit", () => {
    // Not a best-fit packing: a narrow tab slipping into the room a wide one
    // could not use makes tabs disappear from the middle of the strip.
    expect(fit({ available: 150, overflowWidth: 50, widths: [40, 200, 40] })).toEqual({
      overflow: [1, 2],
      visible: [0],
    });
  });

  it("accounts for every tab exactly once", () => {
    const { overflow, visible } = fit({
      activeIndex: 3,
      available: 260,
      overflowWidth: 50,
      widths: [90, 80, 70, 60, 50],
    });
    expect([...visible, ...overflow].sort((a, b) => a - b)).toEqual([0, 1, 2, 3, 4]);
    expect(visible.filter((index) => overflow.includes(index))).toEqual([]);
  });
});

describe("the active tab", () => {
  it("stays in the strip even when it is the last one and the room runs out", () => {
    // The tab the user is looking at is the one thing the strip may not hide.
    // Filling from the left without reserving it puts tab 2 in the menu and
    // leaves the user staring at a strip that does not contain their task.
    expect(fit({ activeIndex: 2, available: 250, overflowWidth: 50, widths: [100, 100, 100] }))
      .toEqual({ overflow: [1], visible: [0, 2] });
  });

  it("takes its room from the other tabs rather than from the strip", () => {
    // Reserving the active tab is only half of it. Reserving it *after* the
    // other tabs have spent the budget keeps it visible by overrunning the
    // row, which is the defect wearing a different hat.
    const widths = [100, 100, 100];
    const available = 250;
    const overflowWidth = 50;
    const { visible } = fit({ activeIndex: 2, available, overflowWidth, widths });
    const spent = visible.reduce((sum, index) => sum + (widths[index] ?? 0), 0);
    expect(spent + overflowWidth).toBeLessThanOrEqual(available);
  });

  it("keeps the active tab alone when nothing else can fit beside it", () => {
    expect(fit({ activeIndex: 1, available: 120, overflowWidth: 50, widths: [90, 90, 90] }))
      .toEqual({ overflow: [0, 2], visible: [1] });
  });

  it("reserves nothing when no task is active", () => {
    // The home view: tabs are open but none of them is the one on screen, so
    // there is no tab that must survive and the strip fills from the left.
    expect(fit({ activeIndex: -1, available: 250, overflowWidth: 50, widths: [100, 100, 100] }))
      .toEqual({ overflow: [2], visible: [0, 1] });
  });
});
