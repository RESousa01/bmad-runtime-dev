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
 *
 * Two shapes of surface, one set of rules. `useDeferredFocusReturn` is for a
 * surface whose owner outlives it and can watch an `isOpen` flag go false.
 * `useFocusReturnOnUnmount` is for one that has no such owner and whose own
 * unmount is the only moment left to act on. They share the guards above
 * deliberately: FE-10 started because two hand-rolled copies had drifted, and a
 * third would be the same mistake in a new shape.
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

/**
 * Whether focus has been left nowhere.
 *
 * This is the whole justification for a return. Removing the element that had
 * focus drops it on `<body>`, the next keystroke goes nowhere, and a keyboard
 * user has to Tab in from the top of the document. If some other surface has
 * since claimed focus, taking it back is the worse bug of the two -- it pulls
 * the user out of whatever they just opened -- so a return is offered, not
 * imposed.
 *
 * There is deliberately no check for a *disconnected* `activeElement`. Removing
 * the focused element is specified to move focus to the body, so the two
 * comparisons below already cover it, and a third clause would be unreachable
 * defence carrying an explanation of a failure that does not happen.
 */
export function focusIsUnclaimed(activeElement: Element | null): boolean {
  return activeElement === null
    || activeElement === document.body
    || activeElement === document.documentElement;
}

/**
 * Returns focus when the surface itself is the thing that unmounts.
 *
 * The recovery review has no owner watching it: the changes panel renders it
 * from two different branches and the activity panel from a third, and each
 * simply stops rendering it. So there is no `isOpen` to fall, and the unmount
 * is the only event left.
 *
 * The unmount is not the end of the story, and that is the defect this
 * replaces. The review used to call `focus()` straight from its effect
 * cleanup, guarded by an `isConnected` check made at that instant. Passive
 * effects for a commit are flushed ahead of the DOM mutations of any render
 * that commit schedules synchronously -- and a `useLayoutEffect` that writes
 * state, anywhere in the shell, schedules exactly that. The check could
 * therefore run against a half-applied DOM, decide the target was gone, and
 * give up for good. Focus stayed on `<body>`. Nothing in the review changed to
 * cause it; a state write added to the title bar was enough, which is what a
 * one-shot check during a commit buys you.
 *
 * A microtask is late enough by construction: it runs once the task that
 * committed the unmount has finished, so every render that task scheduled
 * synchronously has already mutated the DOM. Waiting longer is not safer --
 * it only widens the window in which this fights a surface the user has since
 * opened -- and `focusIsUnclaimed` is what makes arriving late harmless rather
 * than rude.
 */
export function useFocusReturnOnUnmount(target: HTMLElement | null): void {
  const latest = useRef(target);

  useEffect(() => {
    latest.current = target;
  }, [target]);

  // Empty deps, deliberately. The cleanup has to mean "the surface is gone",
  // and keying it on the target would also fire it whenever the target moved
  // -- returning focus out of a surface that is still on screen.
  useEffect(() => () => {
    const returnTarget = latest.current;
    /*
     * Attempted twice, and the second attempt is not belt-and-braces.
     *
     * The immediate try is the ordinary path: nothing else has scheduled work,
     * the DOM is settled, and focus lands before anything can observe it
     * missing. Deferring unconditionally instead would leave focus on `<body>`
     * for a microtask on every close -- unobservable to a person, but it is a
     * real state the app passes through, and anything that samples focus in
     * between reads it.
     *
     * The retry is for the case the immediate try cannot handle: a
     * `useLayoutEffect` that writes state, anywhere in the shell, makes React
     * flush this cleanup against a half-applied DOM, where the target can look
     * disconnected and the attempt is dropped for good. A microtask runs after
     * that task completes, so the DOM is whole by then.
     *
     * They compose safely because `focusIsUnclaimed` gates the retry: if the
     * first attempt worked, focus is claimed and the retry does nothing.
     */
    const restore = () => {
      if (!focusIsUnclaimed(document.activeElement)) return;
      if (isFocusRestorable(returnTarget) && returnTarget !== null) returnTarget.focus();
    };
    restore();
    queueMicrotask(restore);
  }, []);
}
