import { useEffect, useRef, type KeyboardEvent, type RefObject } from "react";

const focusableSelector = [
  "button:not([disabled])",
  "[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

function focusableElements(panel: HTMLElement): HTMLElement[] {
  return Array.from(panel.querySelectorAll<HTMLElement>(focusableSelector)).filter(
    (element) => !element.closest("[hidden], [aria-hidden='true'], [inert]"),
  );
}

export function useModalPanelFocus(active: boolean): RefObject<HTMLElement | null> {
  const panelRef = useRef<HTMLElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);
  const transitionRef = useRef(0);
  const wasActiveRef = useRef(false);

  useEffect(() => {
    const transition = transitionRef.current + 1;
    transitionRef.current = transition;
    if (active && !wasActiveRef.current) {
      const activeElement = document.activeElement;
      returnFocusRef.current = activeElement instanceof HTMLElement ? activeElement : null;
      window.requestAnimationFrame(() => {
        const panel = panelRef.current;
        if (transitionRef.current === transition && panel) {
          focusableElements(panel).at(0)?.focus();
        }
      });
    } else if (!active && wasActiveRef.current) {
      const returnTarget = returnFocusRef.current;
      returnFocusRef.current = null;
      window.requestAnimationFrame(() => {
        if (
          transitionRef.current === transition
          && returnTarget?.isConnected
          && !returnTarget.closest("[inert]")
        ) {
          returnTarget.focus();
        }
      });
    }
    wasActiveRef.current = active;
  }, [active]);

  return panelRef;
}

export function containModalPanelFocus(
  event: KeyboardEvent<HTMLElement>,
  panelRef: RefObject<HTMLElement | null>,
  active: boolean,
): void {
  if (!active || event.key !== "Tab" || !panelRef.current) {
    return;
  }
  const focusable = focusableElements(panelRef.current);
  if (focusable.length === 0) {
    event.preventDefault();
    return;
  }
  const first = focusable[0]!;
  const last = focusable.at(-1)!;
  if (!focusable.includes(document.activeElement as HTMLElement)) {
    event.preventDefault();
    first.focus();
  } else if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}
