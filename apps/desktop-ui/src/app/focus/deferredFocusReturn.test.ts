import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  focusIsUnclaimed,
  focusReturnTarget,
  isFocusRestorable,
  useDeferredFocusReturn,
  useFocusReturnOnUnmount,
} from "./deferredFocusReturn";

const created: HTMLElement[] = [];

function button(label: string): HTMLButtonElement {
  const element = document.createElement("button");
  element.textContent = label;
  document.body.append(element);
  created.push(element);
  return element;
}

afterEach(() => {
  for (const element of created.splice(0)) element.remove();
});

describe("choosing what to return focus to", () => {
  it("prefers an explicit target over whatever is focused", () => {
    // The control that opened a surface is not always the one focus should
    // return to: a menu item is gone by the time the panel closes.
    const explicit = button("explicit");
    const active = button("active");
    expect(focusReturnTarget(explicit, active)).toBe(explicit);
  });

  it("falls back to the focused element", () => {
    const active = button("active");
    expect(focusReturnTarget(null, active)).toBe(active);
  });

  it("never captures the document root", () => {
    // Restoring to `<body>` is indistinguishable from not restoring, but it
    // looks like the mechanism worked.
    expect(focusReturnTarget(null, document.body)).toBeNull();
    expect(focusReturnTarget(null, document.documentElement)).toBeNull();
  });

  it("captures nothing when nothing is focused", () => {
    expect(focusReturnTarget(null, null)).toBeNull();
  });
});

describe("isFocusRestorable", () => {
  it("accepts an element still in the document", () => {
    expect(isFocusRestorable(button("live"))).toBe(true);
  });

  it("rejects an element that has been removed", () => {
    // The row of a task that was just deleted.
    const detached = button("gone");
    detached.remove();
    expect(isFocusRestorable(detached)).toBe(false);
  });

  it("rejects an element inside an inert subtree", () => {
    // `focus()` on an inert element silently does nothing, so this has to be
    // checked rather than attempted.
    const container = document.createElement("div");
    container.setAttribute("inert", "");
    const inside = document.createElement("button");
    container.append(inside);
    document.body.append(container);
    created.push(container);
    expect(isFocusRestorable(inside)).toBe(false);
  });

  it("rejects null", () => {
    expect(isFocusRestorable(null)).toBe(false);
  });
});

describe("the open and close cycle", () => {
  function open(isOpen: boolean) {
    return renderHook(({ open: value }) => useDeferredFocusReturn(value), {
      initialProps: { open: isOpen },
    });
  }

  it("returns focus to where it came from", () => {
    const opener = button("opener");
    opener.focus();
    const { rerender, result } = open(false);

    act(() => result.current.capture());
    rerender({ open: true });
    opener.blur();

    act(() => result.current.arm());
    rerender({ open: false });

    expect(document.activeElement).toBe(opener);
  });

  it("restores after the surface has closed, not during the dismiss", () => {
    // Focusing during the dismiss loses the race against the closing surface's
    // own teardown, which moves focus itself.
    const opener = button("opener");
    opener.focus();
    const { rerender, result } = open(true);
    act(() => result.current.capture(opener));
    opener.blur();

    act(() => result.current.arm());
    // Still open: nothing has been restored yet.
    expect(document.activeElement).not.toBe(opener);

    rerender({ open: false });
    expect(document.activeElement).toBe(opener);
  });

  it("does not restore when the dismissal hands focus elsewhere", () => {
    // Opening a second surface from the first: restoring would yank focus back
    // out of the thing that just opened.
    const opener = button("opener");
    const elsewhere = button("elsewhere");
    const { rerender, result } = open(true);
    act(() => result.current.capture(opener));

    act(() => result.current.arm(false));
    elsewhere.focus();
    rerender({ open: false });

    expect(document.activeElement).toBe(elsewhere);
  });

  it("does not throw when the target has been removed", () => {
    const opener = button("opener");
    const { rerender, result } = open(true);
    act(() => result.current.capture(opener));
    opener.remove();

    act(() => result.current.arm());
    expect(() => rerender({ open: false })).not.toThrow();
  });

  it("does not restore twice", () => {
    // A second close with nothing armed must not re-steal focus from wherever
    // the user has since moved it.
    const opener = button("opener");
    const later = button("later");
    const { rerender, result } = open(true);
    act(() => result.current.capture(opener));
    act(() => result.current.arm());
    rerender({ open: false });
    expect(document.activeElement).toBe(opener);

    later.focus();
    rerender({ open: true });
    rerender({ open: false });
    expect(document.activeElement).toBe(later);
  });
});

describe("focusIsUnclaimed", () => {
  it("reads the document root as nowhere", () => {
    expect(focusIsUnclaimed(document.body)).toBe(true);
    expect(focusIsUnclaimed(document.documentElement)).toBe(true);
  });

  it("reads nothing focused at all as nowhere", () => {
    expect(focusIsUnclaimed(null)).toBe(true);
  });

  it("reads a live element as a claim to leave alone", () => {
    expect(focusIsUnclaimed(button("claimed"))).toBe(false);
  });
});

describe("returning focus when the surface itself unmounts", () => {
  /// The restore is queued as a microtask, so one turn of the queue is what a
  /// test has to wait for. Two, to leave no doubt about ordering.
  async function settle(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
  }

  beforeEach(() => {
    // Focus left over from an earlier test would read as a claim and mask
    // exactly the cases these tests are about.
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
  });

  it("puts focus back on the control the surface took it from", async () => {
    const opener = button("opener");
    const inside = button("inside the surface");
    inside.focus();
    const { unmount } = renderHook(() => useFocusReturnOnUnmount(opener));

    // The surface goes, and its focused control leaves the document with it.
    inside.remove();
    unmount();

    await settle();
    expect(document.activeElement).toBe(opener);
  });

  it("decides after the commit has finished, not during it", async () => {
    // Stands in for the shell mid-commit. A `useLayoutEffect` that writes
    // state anywhere in the tree forces a second synchronous render, and this
    // cleanup is flushed ahead of that render's DOM mutations -- so at the
    // instant it runs, the row focus belongs on can be out of the document and
    // back again before the task ends. Checking once, there, is what left
    // focus on `<body>`.
    const opener = button("opener");
    const { unmount } = renderHook(() => useFocusReturnOnUnmount(opener));

    opener.remove();
    unmount();
    document.body.append(opener);

    await settle();
    expect(document.activeElement).toBe(opener);
  });

  it("leaves focus where a surface opened since has put it", async () => {
    const opener = button("opener");
    const elsewhere = button("elsewhere");
    const { unmount } = renderHook(() => useFocusReturnOnUnmount(opener));

    unmount();
    elsewhere.focus();

    await settle();
    expect(document.activeElement).toBe(elsewhere);
  });

  it("does not restore into a subtree another overlay has made inert", async () => {
    const shelter = document.createElement("div");
    shelter.setAttribute("inert", "");
    const opener = document.createElement("button");
    shelter.append(opener);
    document.body.append(shelter);
    created.push(shelter);

    const { unmount } = renderHook(() => useFocusReturnOnUnmount(opener));
    unmount();

    await settle();
    expect(document.activeElement).toBe(document.body);
  });

  it("does not return focus while the surface is still on screen", async () => {
    // Nothing is focused, so a return firing here would look justified -- and
    // it would take focus out of a surface the user is still reading.
    const first = button("first");
    const second = button("second");
    const { rerender } = renderHook(
      ({ target }) => useFocusReturnOnUnmount(target),
      { initialProps: { target: first as HTMLElement | null } },
    );

    rerender({ target: second });

    await settle();
    expect(document.activeElement).toBe(document.body);
  });
});

describe("transfer", () => {
  it("hands the captured target to another surface and forgets it", () => {
    // One surface replacing another: the second inherits the return target
    // rather than capturing the first as the place to go back to.
    const opener = button("opener");
    const { result } = renderHook(() => useDeferredFocusReturn(true));
    act(() => result.current.capture(opener));

    expect(result.current.transfer()).toBe(opener);
    expect(result.current.transfer()).toBeNull();
  });

  it("leaves nothing to restore after a transfer", () => {
    const opener = button("opener");
    const elsewhere = button("elsewhere");
    const { rerender, result } = renderHook(
      ({ open }) => useDeferredFocusReturn(open),
      { initialProps: { open: true } },
    );
    act(() => result.current.capture(opener));
    act(() => { result.current.transfer(); });

    act(() => result.current.arm());
    elsewhere.focus();
    rerender({ open: false });

    expect(document.activeElement).toBe(elsewhere);
  });
});
