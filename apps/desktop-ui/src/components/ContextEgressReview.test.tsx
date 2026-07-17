// @vitest-environment jsdom
import "../test/setup";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import type { BmadHelpContextReviewProjection } from "../lib/bmadModelProjection";
import { ContextEgressReview } from "./ContextEgressReview";

const review: BmadHelpContextReviewProjection = {
  workspaceId: "workspace_test",
  workspaceGrantEpoch: 7,
  runId: "run_test",
  sessionId: "session_test",
  destinationLabel: "Deterministic local model",
  developmentOnly: true,
  consentDisclosure: "Send only the exact context shown below for one Help request.",
  manifestHash: `sha256:${"a".repeat(64)}`,
  purpose: "Method Help recommendation",
  region: "localdev",
  retentionMode: "transient_no_store",
  expiresAt: Date.UTC(2026, 6, 16, 12, 30),
  items: [{
    relativeLabel: "method/current-intent.txt",
    semanticRole: "current_intent",
    language: "text",
    outboundByteCount: 29,
    tokenEstimate: 8,
    classification: "internal",
    redactions: [{ kind: "email", occurrenceCount: 1 }],
    outboundContent: "Review architecture readiness",
  }],
  exclusions: [{ relativeLabel: "workspace/secrets.env", reason: "Secret-bearing input excluded" }],
  secretFindings: [{ relativeLabel: "method/current-intent.txt", kind: "email", occurrenceCount: 1 }],
  totalOutboundBytes: 29,
  totalTokenEstimate: 8,
  redactionLimitation: "Redaction reduces risk but cannot prove every secret was detected.",
};

describe("ContextEgressReview", () => {
  it("renders exact ordered outbound text inertly and focuses the review heading", () => {
    const { container } = render(
      <ContextEgressReview
        onApprove={vi.fn()}
        onCancel={vi.fn()}
        onSend={vi.fn()}
        phase="review_required"
        review={review}
      />,
    );

    const heading = screen.getByRole("heading", { name: "Review request context" });
    expect(document.activeElement).toBe(heading);
    const exactText = screen.getByText("Review architecture readiness");
    expect(exactText.tagName).toBe("CODE");
    expect(exactText.closest("pre")).not.toBeNull();
    expect(exactText.closest('[role="status"], [role="alert"], [aria-live]')).toBeNull();
    expect(screen.getByText(review.consentDisclosure)).toBeTruthy();
    expect(screen.getByText("Deterministic local model — development only")).toBeTruthy();
    expect(screen.getAllByText("method/current-intent.txt")).toHaveLength(2);
    expect(container.textContent).not.toContain(review.manifestHash);
  });

  it("keeps send disabled until approval and dispatches each explicit gesture", async () => {
    const user = userEvent.setup();
    const onApprove = vi.fn();
    const onCancel = vi.fn();
    const onSend = vi.fn();
    const { rerender } = render(
      <ContextEgressReview
        onApprove={onApprove}
        onCancel={onCancel}
        onSend={onSend}
        phase="review_required"
        review={review}
      />,
    );

    expect(screen.getByRole("button", { name: "Send request" })).toHaveProperty("disabled", true);
    await user.click(screen.getByRole("button", { name: "Approve context" }));
    expect(onApprove).toHaveBeenCalledOnce();

    rerender(
      <ContextEgressReview
        onApprove={onApprove}
        onCancel={onCancel}
        onSend={onSend}
        phase="approved"
        review={review}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Send request" }));
    expect(onSend).toHaveBeenCalledOnce();
    await user.click(screen.getByRole("button", { name: "Cancel review" }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("has no automatically detectable accessibility violations", async () => {
    const { container } = render(
      <ContextEgressReview
        onApprove={vi.fn()}
        onCancel={vi.fn()}
        onSend={vi.fn()}
        phase="approved"
        review={review}
      />,
    );

    expect((await axe.run(container)).violations).toEqual([]);
  });
});
