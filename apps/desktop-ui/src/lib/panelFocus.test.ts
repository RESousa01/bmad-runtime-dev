import { afterEach, describe, expect, it, type MockInstance, vi } from "vitest";
import type { KeyboardEvent } from "react";

import { containModalPanelFocus } from "./panelFocus";

/// The shape `containModalPanelFocus` actually reads off a React key event.
function tabEvent(shiftKey = false): {
  event: KeyboardEvent<HTMLElement>;
  preventDefault: MockInstance<() => void>;
} {
  const preventDefault = vi.fn<() => void>();
  return {
    event: { key: "Tab", shiftKey, preventDefault } as unknown as KeyboardEvent<HTMLElement>,
    preventDefault,
  };
}

/// The governed-change review, reduced to the shape that mattered: one
/// disclosure per changed file, with the decision controls *after* it.
function renderReviewPanel(open: boolean): HTMLElement {
  const panel = document.createElement("section");
  panel.innerHTML = `
    <button type="button" id="close">Close</button>
    <details${open ? " open" : ""}>
      <summary id="file">src/app.ts</summary>
      <button type="button" id="copy">Copy diff</button>
    </details>
    <button type="button" id="apply">Apply</button>
    <button type="button" id="revise">Revise</button>
    <button type="button" id="discard">Discard</button>
  `;
  document.body.append(panel);
  return panel;
}

const byId = (panel: HTMLElement, id: string): HTMLElement =>
  panel.querySelector<HTMLElement>(`#${id}`)!;

afterEach(() => {
  document.body.innerHTML = "";
});

describe("containModalPanelFocus", () => {
  it("leaves Tab alone on a file disclosure, so the actions below stay reachable", () => {
    // The defect this pins. `<summary>` is natively tabbable but was missing
    // from `focusableSelector`, and an `activeElement` the selector cannot find
    // is treated as *outside* the panel and forced back to the first entry. In
    // the contained drawer at <=1240px, tabbing onto a changed file therefore
    // snapped focus to the top of the panel, and Apply / Revise / Discard —
    // everything after the disclosure in DOM order — could not be reached by
    // keyboard at all.
    const panel = renderReviewPanel(true);
    const file = byId(panel, "file");
    file.focus();
    expect(document.activeElement).toBe(file);

    const { event, preventDefault } = tabEvent();
    containModalPanelFocus(event, { current: panel }, true);

    // Mid-ring: the panel must not intervene, so the browser's own Tab moves
    // on to the next control instead of the focus being yanked backwards.
    expect(preventDefault).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(file);
  });

  it("treats a disclosure as a real stop when wrapping around the ring", () => {
    const panel = renderReviewPanel(true);
    byId(panel, "close").focus();

    const { event, preventDefault } = tabEvent(true);
    containModalPanelFocus(event, { current: panel }, true);

    // Shift+Tab off the first control wraps to the last one. That is Discard
    // either way, so wrap direction alone would not prove the summary is in the
    // ring — the forward case above is what does.
    expect(preventDefault).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(byId(panel, "discard"));
  });

  it("skips controls inside a collapsed disclosure, which cannot take focus", () => {
    // The mirror image of the bug above: a closed `<details>` does not render
    // its contents, so `focus()` on anything inside is a no-op. Counting one as
    // the wrap target would suppress Tab and leave the user with nowhere to go.
    const panel = renderReviewPanel(false);
    const copy = byId(panel, "copy");
    byId(panel, "discard").focus();

    const { event } = tabEvent();
    containModalPanelFocus(event, { current: panel }, true);

    expect(document.activeElement).not.toBe(copy);
    // Forward off the last control wraps to the first, which is still reachable.
    expect(document.activeElement).toBe(byId(panel, "close"));
  });

  it("keeps an open disclosure's own controls in the ring", () => {
    const panel = renderReviewPanel(true);
    byId(panel, "copy").focus();

    const { event, preventDefault } = tabEvent();
    containModalPanelFocus(event, { current: panel }, true);

    expect(preventDefault).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(byId(panel, "copy"));
  });
});
