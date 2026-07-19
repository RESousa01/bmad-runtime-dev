// @vitest-environment jsdom
import "../test/setup";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import type { ChangesRecoveryPrepared } from "../lib/hostClient";
import { RecoveryReview } from "./RecoveryReview";

const privateHash = `sha256:${"a".repeat(64)}`;
const review: Extract<ChangesRecoveryPrepared, { status: "review_required" }> = {
  status: "review_required",
  recoveryApprovalId: "recovery_approval_private_01K0Q6H3",
  displayedRecoveryHash: privateHash,
  journalId: "journal_private_01K0Q6H3",
  executionId: "execution_private_01K0Q6H3",
  operations: [{
    relativePath: "src/example.ts",
    operation: "replace",
    explanation: "Restore the file content saved before the interrupted change.",
  }],
  expiresAt: 1_725_000_060_000,
};

describe("RecoveryReview", () => {
  it("shows only bounded relative recovery facts and focuses the heading", () => {
    const { container } = render(
      <RecoveryReview
        busy={false}
        onDecide={vi.fn()}
        returnFocusTarget={null}
        review={review}
      />,
    );
    const heading = screen.getByRole("heading", { name: "Review checkpoint recovery" });
    expect(document.activeElement).toBe(heading);
    expect(screen.getByText("src/example.ts")).toBeTruthy();
    expect(screen.getByText(/returns the listed paths to the durable checkpoint/i)).toBeTruthy();
    expect(screen.getByText(/confirmation is bound to this exact review/i).closest(
      '[role="status"], [role="alert"], [aria-live]',
    )).toBeNull();
    for (const privateValue of [
      privateHash,
      review.recoveryApprovalId,
      review.journalId,
      review.executionId,
      "C:\\private\\checkpoint",
    ]) {
      expect(container.textContent).not.toContain(privateValue);
    }
  });

  it("dispatches restore and cancel at most once under double click", async () => {
    const user = userEvent.setup();
    const restore = vi.fn();
    const { unmount } = render(
      <RecoveryReview busy={false} onDecide={restore} returnFocusTarget={null} review={review} />,
    );
    await user.dblClick(screen.getByRole("button", { name: "Restore checkpoint" }));
    expect(restore).toHaveBeenCalledOnce();
    expect(restore).toHaveBeenCalledWith("restore");
    unmount();

    const cancel = vi.fn();
    render(<RecoveryReview busy={false} onDecide={cancel} returnFocusTarget={null} review={review} />);
    await user.dblClick(screen.getByRole("button", { name: "Cancel" }));
    expect(cancel).toHaveBeenCalledOnce();
    expect(cancel).toHaveBeenCalledWith("cancel");
  });

  it("supports keyboard activation, disables pending actions, and returns focus", async () => {
    const user = userEvent.setup();
    const targetRef = createRef<HTMLButtonElement>();
    const { rerender, unmount } = render(
      <>
        <button ref={targetRef} type="button">Open recovery</button>
        <RecoveryReview
          busy={false}
          onDecide={vi.fn()}
          returnFocusTarget={targetRef.current}
          review={review}
        />
      </>,
    );
    rerender(
      <>
        <button ref={targetRef} type="button">Open recovery</button>
        <RecoveryReview busy onDecide={vi.fn()} returnFocusTarget={targetRef.current} review={review} />
      </>,
    );
    expect(screen.getByRole("button", { name: "Restore checkpoint" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveProperty("disabled", true);
    unmount();

    render(<button ref={targetRef} type="button">Open recovery</button>);
    targetRef.current?.focus();
    await user.keyboard("{Enter}");
    expect(document.activeElement).toBe(targetRef.current);
  });

  it("has no automatically detectable accessibility violations", async () => {
    const { container } = render(
      <RecoveryReview busy={false} onDecide={vi.fn()} returnFocusTarget={null} review={review} />,
    );
    expect((await axe.run(container)).violations).toEqual([]);
  });
});
