import { afterEach, describe, expect, it, type MockInstance, vi } from "vitest";
import type { KeyboardEvent } from "react";

import { containModalPanelFocus, focusableElements } from "./panelFocus";

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

  it("wraps forward off the last control to the first one", () => {
    // This fixture's collapsed disclosure sits *mid-panel*, where including or
    // excluding its content changes neither `first` nor `last`. So this case
    // pins the forward wrap and nothing else — the collapsed-content filter is
    // pinned by "excludes a collapsed disclosure's content" below, which puts
    // the disclosure last so a leaked child would actually become the target.
    const panel = renderReviewPanel(false);
    byId(panel, "discard").focus();

    const { event } = tabEvent();
    containModalPanelFocus(event, { current: panel }, true);

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

describe("focusableElements", () => {
  /// The ring is only meaningful in document order, and `querySelectorAll` is
  /// not a reliable source of it here: jsdom's selector engine stops returning
  /// document order once the selector list contains a descendant combinator,
  /// which `details > summary` added. Under jsdom it grouped the results by
  /// selector, putting an `<input>` that is first in the DOM after two buttons
  /// that follow it — so the wrap-around asserted by the tests above was not
  /// the one Chromium performs.
  it("returns the ring in document order regardless of selector shape", () => {
    const panel = document.createElement("section");
    panel.innerHTML = `
      <input id="search" type="text" />
      <button type="button" id="first-action">New task</button>
      <button type="button" id="apply">Apply</button>
    `;
    document.body.append(panel);

    expect(focusableElements(panel).map((element) => element.id)).toEqual([
      "search",
      "first-action",
      "apply",
    ]);
  });

  it("excludes a collapsed disclosure's content, which cannot take focus", () => {
    // A closed `<details>` renders nothing inside itself, so `focus()` on a
    // control in there is a no-op. The disclosure is deliberately *last* in the
    // panel: that is the only arrangement in which a leaked child becomes the
    // wrap target, so Tab would be suppressed and focus would go nowhere.
    const panel = document.createElement("section");
    panel.innerHTML = `
      <button type="button" id="close">Close</button>
      <details>
        <summary id="file">src/app.ts</summary>
        <button type="button" id="copy">Copy diff</button>
      </details>
    `;
    document.body.append(panel);

    // The summary stays in the ring whether or not the disclosure is open.
    expect(focusableElements(panel).map((element) => element.id)).toEqual(["close", "file"]);

    byId(panel, "file").focus();
    const { event, preventDefault } = tabEvent();
    containModalPanelFocus(event, { current: panel }, true);

    expect(preventDefault).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(byId(panel, "close"));
    expect(document.activeElement).not.toBe(byId(panel, "copy"));
  });

  it("excludes controls hidden from the accessibility tree or made inert", () => {
    // `AppShellLayout` marks the layer beneath the topmost surface both
    // `aria-hidden` and `inert`, and panels collapse sections with `hidden`.
    // A control in any of those subtrees is not in the browser's sequential
    // focus order, so counting one makes `first`/`last` unfocusable and Tab
    // dead-ends. Nothing else in the suite exercised this filter.
    const panel = document.createElement("section");
    panel.innerHTML = `
      <button type="button" id="close">Close</button>
      <div aria-hidden="true"><button type="button" id="decorative">Scrim</button></div>
      <div hidden><button type="button" id="folded">Hidden</button></div>
      <div inert><button type="button" id="blocked">Inert</button></div>
      <button type="button" id="apply">Apply</button>
    `;
    document.body.append(panel);

    expect(focusableElements(panel).map((element) => element.id)).toEqual(["close", "apply"]);

    // And the ring wraps across them rather than through them.
    byId(panel, "apply").focus();
    const { event } = tabEvent();
    containModalPanelFocus(event, { current: panel }, true);

    expect(document.activeElement).toBe(byId(panel, "close"));
  });

  it("wraps from the last control to the first one in the DOM", () => {
    const panel = document.createElement("section");
    panel.innerHTML = `
      <input id="search" type="text" />
      <button type="button" id="first-action">New task</button>
      <button type="button" id="apply">Apply</button>
    `;
    document.body.append(panel);
    panel.querySelector<HTMLElement>("#apply")!.focus();

    const { event, preventDefault } = tabEvent();
    containModalPanelFocus(event, { current: panel }, true);

    expect(preventDefault).toHaveBeenCalledTimes(1);
    expect(document.activeElement).toBe(panel.querySelector("#search"));
  });
});
