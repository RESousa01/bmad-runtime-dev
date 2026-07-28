// @vitest-environment jsdom
import "../test/setup";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { afterEach, describe, expect, it, vi } from "vitest";
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

  it("supports keyboard activation and disables pending actions", async () => {
    const user = userEvent.setup();
    const onDecide = vi.fn();
    const { rerender } = render(
      <RecoveryReview busy={false} onDecide={onDecide} returnFocusTarget={null} review={review} />,
    );

    // Reached and fired from the keyboard alone. The heading holds focus on
    // mount, so Tab is the only route to the decisions and Enter the only way
    // to take one.
    await user.tab();
    await user.tab();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Restore checkpoint" }));
    await user.keyboard("{Enter}");
    expect(onDecide).toHaveBeenCalledExactlyOnceWith("restore");

    rerender(
      <RecoveryReview busy onDecide={onDecide} returnFocusTarget={null} review={review} />,
    );
    expect(screen.getByRole("button", { name: "Restore checkpoint" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveProperty("disabled", true);
  });

  it("has no automatically detectable accessibility violations", async () => {
    const { container } = render(
      <RecoveryReview busy={false} onDecide={vi.fn()} returnFocusTarget={null} review={review} />,
    );
    expect((await axe.run(container)).violations).toEqual([]);
  });
/// The guard used to be a ref read during render. Writing a ref schedules no
  /// render, so the buttons stayed enabled after a decision was sent and only
  /// went disabled if something unrelated happened to re-render this subtree.
  /// The parent sets `busy` eventually, but "eventually" is a host round trip.
  it("disables both decisions as soon as one is taken, without waiting for busy", async () => {
    const user = userEvent.setup();
    const onDecide = vi.fn();
    render(
      <RecoveryReview busy={false} onDecide={onDecide} returnFocusTarget={null} review={review} />,
    );

    const restore = screen.getByRole("button", { name: "Restore checkpoint" });
    const cancel = screen.getByRole("button", { name: "Cancel" });
    expect(restore).toHaveProperty("disabled", false);

    await user.click(restore);

    // `busy` is still false: the parent has not heard back from the host yet.
    expect(onDecide).toHaveBeenCalledExactlyOnceWith("restore");
    expect(restore).toHaveProperty("disabled", true);
    expect(cancel).toHaveProperty("disabled", true);
  });
});

/// Closing the review removes the element that had focus, and an element
/// removed while focused drops the user on `<body>`: the next keystroke goes
/// nowhere and reaching anything means Tabbing in from the top of the app.
///
/// The trigger the review must go back to lives in a sibling panel, so it is
/// built here rather than rendered -- which is also what lets these tests say
/// precisely when it is in the document and when it is not.
describe("RecoveryReview focus return", () => {
  const created: HTMLElement[] = [];

  function trigger(): HTMLButtonElement {
    const element = document.createElement("button");
    element.type = "button";
    element.textContent = "Review recovery";
    document.body.append(element);
    created.push(element);
    return element;
  }

  /// The return is queued as a microtask, so one turn of the queue is what a
  /// test waits for. Two, to leave no doubt about ordering.
  async function settle(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
  }

  afterEach(() => {
    for (const element of created.splice(0)) element.remove();
  });

  it("leaves focus on the trigger rather than on the body", async () => {
    const opener = trigger();
    opener.focus();
    const { unmount } = render(
      <RecoveryReview
        busy={false}
        onDecide={vi.fn()}
        returnFocusTarget={opener}
        review={review}
      />,
    );
    // The review took focus off the trigger and onto its own heading, so
    // unmounting it is what strands the user.
    expect(document.activeElement).toBe(
      screen.getByRole("heading", { name: "Review checkpoint recovery" }),
    );

    unmount();

    await settle();
    expect(document.activeElement).toBe(opener);
  });

  it("returns focus even when the trigger is out of the document at the unmount", async () => {
    // The exact defect. A `useLayoutEffect` that writes state anywhere in the
    // shell -- the title bar does -- forces a second synchronous render, and
    // this component's effect cleanup is flushed ahead of that render's DOM
    // mutations. The old code checked `isConnected` there, once, saw the row
    // missing, and gave up for good.
    const opener = trigger();
    opener.focus();
    const { unmount } = render(
      <RecoveryReview
        busy={false}
        onDecide={vi.fn()}
        returnFocusTarget={opener}
        review={review}
      />,
    );

    opener.remove();
    unmount();
    document.body.append(opener);

    await settle();
    expect(document.activeElement).toBe(opener);
  });

  it("does not pull focus back out of whatever opened next", async () => {
    const opener = trigger();
    const elsewhere = trigger();
    const { unmount } = render(
      <RecoveryReview
        busy={false}
        onDecide={vi.fn()}
        returnFocusTarget={opener}
        review={review}
      />,
    );

    unmount();
    elsewhere.focus();

    await settle();
    expect(document.activeElement).toBe(elsewhere);
  });

  it("keeps the heading focused when the panel re-renders it under a new branch", async () => {
    // The changes panel renders this from two separate branches, so switching
    // branches remounts it. A return that fired unconditionally would land on
    // the trigger a moment after the new instance had taken focus for its
    // heading, and the user would be reading a review with focus behind it.
    const opener = trigger();
    opener.focus();
    const { rerender } = render(
      <div key="idle">
        <RecoveryReview
          busy={false}
          onDecide={vi.fn()}
          returnFocusTarget={opener}
          review={review}
        />
      </div>,
    );
    rerender(
      <div key="applied">
        <RecoveryReview
          busy={false}
          onDecide={vi.fn()}
          returnFocusTarget={opener}
          review={review}
        />
      </div>,
    );

    await settle();
    expect(document.activeElement).toBe(
      screen.getByRole("heading", { name: "Review checkpoint recovery" }),
    );
  });
});
