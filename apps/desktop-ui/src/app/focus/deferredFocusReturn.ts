/**
 * Putting focus back where an overlay took it from.
 *
 * FE-10 step 3. The utility panel and the context drawer each had their own
 * copy of this: a ref for where focus came from, a second "pending" ref armed
 * on dismiss, and an effect that consumed it once the surface was gone. Three
 * moving parts, twice, and every one of them is a way to leave focus on
 * `<body>` -- which means the next keystroke goes nowhere and a keyboard user
 * has to Tab in from the top of the document.
 *
 * Three rules, and each exists because of a way this fails:
 *
 * Deferred, not immediate. The restore happens in an effect that runs *after*
 * the surface has closed. Focusing during the dismiss loses the race against
 * the closing surface's own teardown, which moves focus itself.
 *
 * Never `<body>`. Capturing `document.body` or `documentElement` as the return
 * target and then "restoring" to it is the same as not restoring at all, but
 * it looks like the mechanism worked.
 *
 * Never detached or inert. On the way back, the element may have been removed
 * -- the row of a task that was just deleted -- or be inside an `[inert]`
 * subtree because another overlay opened. `focus()` on an inert element
 * silently does nothing, so this has to be checked rather than attempted.
 */

import { useEffect, useRef } from "react";

export interface DeferredFocusReturn {
  /**
   * Records where focus should go when the surface closes.
   *
   * Called as the surface opens. An explicit target wins over the active
   * element, because the control that opened the surface is not always the one
   * focus should return to -- a menu item that opens a panel is gone by the
   * time the panel closes.
   */
  readonly capture: (explicitTarget?: HTMLElement | null) => void;
  /**
   * Arms the restore. Called as the surface is dismissed.
   *
   * `restore: false` for a dismissal that hands focus somewhere else itself --
   * opening a second surface from the first, where restoring would yank focus
   * back out of the thing that just opened.
   */
  readonly arm: (restore?: boolean) => void;
  /** Hands the captured target to another surface, and forgets it. */
  readonly transfer: () => HTMLElement | null;
}

/** Whether an element can still usefully receive focus. */
export function isFocusRestorable(target: HTMLElement | null): boolean {
  // `closest` rather than a property check: `inert` is inherited from any
  // ancestor, and the element itself usually is not the one marked.
  return target !== null && target.isConnected && !target.closest("[inert]");
}

/** The element focus should return to, or null when there is no useful one. */
export function focusReturnTarget(
  explicitTarget: HTMLElement | null | undefined,
  activeElement: Element | null,
): HTMLElement | null {
  if (explicitTarget != null) return explicitTarget;
  if (!(activeElement instanceof HTMLElement)) return null;
  // Restoring to the document root is indistinguishable from not restoring,
  // and recording it hides the fact that nothing was captured.
  if (activeElement === document.body || activeElement === document.documentElement) {
    return null;
  }
  return activeElement;
}

/**
 * @param isOpen Whether the surface is currently open. The restore fires on
 *   the transition to closed.
 */
export function useDeferredFocusReturn(isOpen: boolean): DeferredFocusReturn {
  const captured = useRef<HTMLElement | null>(null);
  const pending = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (isOpen) return;
    const target = pending.current;
    pending.current = null;
    if (isFocusRestorable(target)) target!.focus();
  }, [isOpen]);

  return {
    arm(restore = true) {
      pending.current = restore ? captured.current : null;
      captured.current = null;
    },
    capture(explicitTarget) {
      captured.current = focusReturnTarget(explicitTarget, document.activeElement);
    },
    transfer() {
      const target = captured.current;
      captured.current = null;
      return target;
    },
  };
}
