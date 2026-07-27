import { useEffect, useRef, type KeyboardEvent, type RefObject } from "react";

/// Everything Chromium puts in the sequential focus order.
///
/// `containModalPanelFocus` treats an `activeElement` it cannot find in this
/// list as *outside* the panel and forces focus back to the first entry, so a
/// natively tabbable element missing from the selector is not a cosmetic gap —
/// it is a keyboard trap for everything after it in DOM order. `summary` was
/// the one that bit: the governed-change review renders one
/// `<details><summary>` per changed file above its Apply / Revise / Discard
/// row, so under the <=1240px contained drawer, tabbing into a file summary
/// snapped focus back to the top and those three controls could not be
/// reached by keyboard at all. `BmadLibraryPanel` has the same shape.
const focusableSelector = [
  "button:not([disabled])",
  "[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "details > summary:not([tabindex='-1'])",
  "[contenteditable]:not([contenteditable='false'])",
  "audio[controls]",
  "video[controls]",
  "iframe",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

/// A collapsed `<details>` does not render its contents, so nothing inside it
/// is in the tab order — only its own summary is. Counting that content as
/// focusable is the mirror image of the bug above: `first`/`last` would land
/// on an element that cannot take focus, `focus()` would no-op, and the
/// suppressed Tab would leave the user stuck.
function isInsideCollapsedDetails(element: HTMLElement): boolean {
  let node: HTMLElement | null = element;
  while (node !== null) {
    const details: HTMLDetailsElement | null =
      node.parentElement?.closest("details") ?? null;
    if (details === null) {
      return false;
    }
    // A disclosure's own summary stays in the tab order whether or not the
    // disclosure is open; everything else inside it does not.
    const isOwnSummary = node.parentElement === details && node.tagName === "SUMMARY";
    if (!details.open && !isOwnSummary) {
      return true;
    }
    node = details;
  }
  return false;
}

/// Exported so every focus trap in the shell agrees on what "focusable" means.
/// A second, hand-rolled selector is how the `summary` gap above went unnoticed
/// in one panel while being fixed in another.
export function focusableElements(panel: HTMLElement): HTMLElement[] {
  return Array.from(panel.querySelectorAll<HTMLElement>(focusableSelector))
    .filter(
      (element) =>
        !element.closest("[hidden], [aria-hidden='true'], [inert]")
        && !isInsideCollapsedDetails(element),
    )
    // Sorted explicitly, because `first` and `last` are only meaningful in
    // document order and this list is not reliably in it.
    //
    // The spec says `querySelectorAll` returns document order, and Chromium
    // does. jsdom's selector engine stops doing so once the selector list
    // contains a descendant combinator — adding `details > summary` to the
    // list above is what tipped it over — and returns a per-selector union
    // instead. In this palette that put the search input, which is first in
    // the DOM, last in the list. Every Tab-containment test in the renderer
    // therefore ran against a wrap-around the browser never performs, so the
    // tests passed while asserting the wrong element.
    .sort((left, right) =>
      (left.compareDocumentPosition(right) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0 ? -1 : 1,
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
      returnFocusRef.current = activeElement instanceof HTMLElement
        && activeElement !== document.body
        && activeElement !== document.documentElement
        ? activeElement
        : null;
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
