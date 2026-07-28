import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { adoptPreview, emptyTaskAttachments } from "../features/attachments/attachmentTypes";
import {
  measure,
  reconcile,
  type ModelContextKey,
  type ModelContextState,
} from "../features/model-context/modelContextTypes";
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
      byteCount: 8_000,
      /// What the host actually reports: `floor((byteCount + 3) / 4)`. The
      /// panel must never present this as anything but the guess it is.
      estimatedTokens: 2_000,
      content: "local source",
    },
  ],
  totalBytes: 8_000,
  estimatedTokens: 2_000,
  modelTarget: null,
};

/// The task-owned shape the panel now takes. Built from a host projection so
/// the fixture still describes what the host actually returns.
const attached = adoptPreview(preview, "automatic");
const pinned = adoptPreview(preview, "manual");

const measurementKey: ModelContextKey = {
  taskId: "task_01",
  workspaceId: "workspace_01",
  workspaceGrantEpoch: 1,
  modelRoute: "openai/gpt-5",
  promptEpoch: 1,
};

function measured(inputTokens: number, windowTokens: number | null): ModelContextState {
  return measure({ key: measurementKey, inputTokens, windowTokens, measuredAt: 1_700_000_000_000 });
}

afterEach(cleanup);

describe("ContextWindow", () => {
  /// FE-16: the budget is the *model's* window. It used to be
  /// `workspaceReadLimits.contextBytes / 4` -- a workspace file-read cap shown
  /// as model capacity -- and the host reports no model window at all, so the
  /// only honest denominator today is the word.
  it("says the model window is unavailable instead of showing a read cap", () => {
    render(
      <ContextWindow
        canSuggest
        onClear={vi.fn()}
        onSuggest={vi.fn()}
        attachments={attached}
      />,
    );

    expect(screen.getByRole("heading", { name: "Context window" })).toBeTruthy();
    expect(screen.getByText("2K / Unavailable")).toBeTruthy();
    expect(screen.queryByText(/64K/)).toBeNull();
    // Nothing to divide by means nothing to draw a bar against.
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.getByText("src/App.tsx")).toBeTruthy();
    // The absence is stated, not merely left blank -- a missing denominator
    // reads as "no limit" to the user it matters most to.
    expect(screen.getByText(/context window is not reported by the host/i)).toBeTruthy();
    expect(screen.getByText(/only files you add are included with your message/i)).toBeTruthy();
    expect(screen.queryByText(/confirm sending/i)).toBeNull();
  });

  it("labels a byte-derived total as an estimate, in the row and per file", () => {
    render(
      <ContextWindow
        canSuggest
        onClear={vi.fn()}
        onSuggest={vi.fn()}
        attachments={attached}
      />,
    );

    expect(screen.getByText("Estimated")).toBeTruthy();
    expect(screen.getByText("~2K")).toBeTruthy();
    expect(screen.getByTitle("2,000 estimated tokens")).toBeTruthy();
    expect(screen.getByLabelText("Attached files and their estimated token counts")).toBeTruthy();
    // The summary's own label is fixed; the provenance rides on a description
    // so a screen reader gets the reason and not just the verdict.
    expect(screen.getByRole("button", { name: "Open context details" })
      .getAttribute("aria-describedby")).toBe("context-window-basis");
    expect(screen.getByText(/not counted by a tokenizer/i)).toBeTruthy();
  });

  /// The per-file numbers stay byte-derived even once the host reports a total,
  /// so their estimate marker cannot be inherited from the row.
  it("keeps the per-file estimate marked when the total is host-reported", () => {
    render(
      <ContextWindow
        canSuggest
        modelContext={measured(12_000, 200_000)}
        onClear={vi.fn()}
        onSuggest={vi.fn()}
        attachments={attached}
      />,
    );

    expect(screen.getByText("Reported")).toBeTruthy();
    expect(screen.getByText("~2K")).toBeTruthy();
  });

  it("shows a measured usage against the window it was measured against", () => {
    render(
      <ContextWindow
        canSuggest
        modelContext={measured(12_000, 200_000)}
        onClear={vi.fn()}
        onSuggest={vi.fn()}
        attachments={attached}
      />,
    );

    // 12K, not the 2K of attachments: the host's measurement covers the
    // transcript and the persona too, and supersedes the local sum.
    expect(screen.getByText("12K / 200K")).toBeTruthy();
    const meter = screen.getByRole("progressbar", { name: "Model context usage" });
    expect(meter.getAttribute("aria-valuemax")).toBe("200000");
    expect(meter.getAttribute("aria-valuenow")).toBe("12000");
    expect(meter.getAttribute("aria-valuetext")).toBe("12K of 200K tokens");
  });

  it("keeps the host's number when the measurement carried no window", () => {
    render(
      <ContextWindow
        canSuggest
        modelContext={measured(12_000, null)}
        onClear={vi.fn()}
        onSuggest={vi.fn()}
        attachments={attached}
      />,
    );

    expect(screen.getByText("12K / Unavailable")).toBeTruthy();
    expect(screen.getByText("Reported")).toBeTruthy();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  /// A measurement taken under a different route describes a request that will
  /// never be made again. It stays on screen -- a number with a stated age
  /// beats a blank panel -- but it must not read as current.
  it("marks a superseded measurement rather than presenting it as current", () => {
    render(
      <ContextWindow
        canSuggest
        modelContext={reconcile(measured(12_000, 200_000), {
          ...measurementKey,
          modelRoute: "azure/gpt-5",
        })}
        onClear={vi.fn()}
        onSuggest={vi.fn()}
        attachments={attached}
      />,
    );

    expect(screen.getByText("Last reported")).toBeTruthy();
    expect(screen.queryByText("Reported")).toBeNull();
    expect(screen.getByText("12K / 200K")).toBeTruthy();
    expect(screen.getByText(/no longer describes what would be sent now/i)).toBeTruthy();
  });

  it("exposes bounded auto-selection and clearing as explicit actions", () => {
    const onSuggest = vi.fn();
    const onClear = vi.fn();
    render(
      <ContextWindow
        canSuggest
        onClear={onClear}
        onSuggest={onSuggest}
        attachments={pinned}
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
        attachments={emptyTaskAttachments}
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
        attachments={emptyTaskAttachments}
      />,
    );

    expect(screen.getByRole("button", { name: "Open context details" })).toBeTruthy();
    expect(screen.getByText("0 / Unavailable")).toBeTruthy();
    expect(screen.getByText("No files")).toBeTruthy();
    expect(screen.queryByText("Local preview limit")).toBeNull();
    expect(screen.queryByText("Model window set during request review")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open context details" }));
    expect(onOpen).toHaveBeenCalledOnce();
  });
});
