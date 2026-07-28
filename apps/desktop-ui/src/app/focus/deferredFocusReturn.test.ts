import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import {
  focusReturnTarget,
  isFocusRestorable,
  useDeferredFocusReturn,
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
