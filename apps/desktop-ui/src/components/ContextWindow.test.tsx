import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ContextPreviewProjection } from "../lib/hostClient";
import { ContextWindow } from "./ContextWindow";

const preview: ContextPreviewProjection = {
  workspaceId: "workspace_01",
  manifestHash: "sha256:preview",
  items: [
    {
      relativePath: "src/App.tsx",
      startLine: 1,
      endLine: 80,
      reason: "Selected for this task",
      contentHash: "sha256:item",
      classification: "source",
      redactions: [],
      byteCount: 4096,
      estimatedTokens: 1024,
      content: "local source",
    },
  ],
  totalBytes: 4096,
  estimatedTokens: 1024,
  modelTarget: null,
};

afterEach(cleanup);

describe("ContextWindow", () => {
  it("shows the enforced local budget and exact per-file token estimates", () => {
    render(
      <ContextWindow
        canSuggest
        onClear={vi.fn()}
        onSuggest={vi.fn()}
        preview={preview}
        selectionMode="automatic"
        suggestionBusy={false}
      />,
    );

    expect(screen.getByRole("heading", { name: "Context window" })).toBeTruthy();
    expect(screen.getByText("1K / 64K")).toBeTruthy();
    expect(screen.getByText("src/App.tsx")).toBeTruthy();
    expect(screen.getByText("1K")).toBeTruthy();
    expect(screen.getByText("Automatic")).toBeTruthy();
    expect(screen.getByRole("progressbar", { name: "Local context budget" })
      .getAttribute("aria-valuemax")).toBe("65536");
    expect(screen.getByText(/only files you add are included with your message/i)).toBeTruthy();
    expect(screen.queryByText(/confirm sending/i)).toBeNull();
  });

  it("exposes bounded auto-selection and clearing as explicit actions", () => {
    const onSuggest = vi.fn();
    const onClear = vi.fn();
    render(
      <ContextWindow
        canSuggest
        onClear={onClear}
        onSuggest={onSuggest}
        preview={preview}
        selectionMode="manual"
        suggestionBusy={false}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Auto-select context" }));
    fireEvent.click(screen.getByRole("button", { name: "Clear context" }));
    expect(onSuggest).toHaveBeenCalledOnce();
    expect(onClear).toHaveBeenCalledOnce();
  });

  it("does not imply that a local preview is bound to a model", () => {
    render(
      <ContextWindow
        canSuggest={false}
        onClear={vi.fn()}
        onSuggest={vi.fn()}
        preview={null}
        selectionMode="manual"
        suggestionBusy={false}
      />,
    );
    expect(screen.getByText("No files")).toBeTruthy();
    expect(screen.getByText("Pinned")).toBeTruthy();
    expect(screen.queryByText(/model limit|model window/i)).toBeNull();
    expect(screen.getByRole("button", { name: "Auto-select context" }))
      .toHaveProperty("disabled", true);
  });

  it("collapses the v2 composer context into one OpenCode-style status row", () => {
    const onOpen = vi.fn();
    render(
      <ContextWindow
        canSuggest={false}
        onClear={vi.fn()}
        onOpen={onOpen}
        onSuggest={vi.fn()}
        preview={null}
        selectionMode="manual"
        suggestionBusy={false}
      />,
    );

    expect(screen.getByRole("button", { name: "Open context details" })).toBeTruthy();
    expect(screen.getByText("0 / 64K")).toBeTruthy();
    expect(screen.getByText("No files")).toBeTruthy();
    expect(screen.queryByText("Local preview limit")).toBeNull();
    expect(screen.queryByText("Model window set during request review")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open context details" }));
    expect(onOpen).toHaveBeenCalledOnce();
  });
});
