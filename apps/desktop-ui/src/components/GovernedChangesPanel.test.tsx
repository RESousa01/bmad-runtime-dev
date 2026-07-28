import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  ChangesExecutionProjection,
  ChangesHistoryProjection,
  ChangesReviewEnvelopeProjection,
  ChangesRecoveryPrepared,
} from "../lib/hostClient";
import {
  GovernedChangesPanel,
  type GovernedChangesPanelProps,
} from "./GovernedChangesPanel";

const sha = (seed: string) => `sha256:${seed.repeat(64).slice(0, 64)}`;

const reviewEnvelope: ChangesReviewEnvelopeProjection = {
  approvalId: "approval_01J00000000000000000000000",
  displayedDiffHash: sha("a"),
  review: {
    schemaVersion: "sapphirus.changes-review.v1",
    proposalId: "proposal_01J00000000000000000000000",
    candidateId: "candidate_01J00000000000000000000000",
    candidateHash: sha("b"),
    workspaceId: "workspace_01J00000000000000000000000",
    workspaceGrantEpoch: 2,
    proposalKind: "edit",
    sourceExecutionId: null,
    files: [{
      relativePath: "src/main.rs",
      operation: "modify",
      beforeContent: "fn main() {}\n",
      afterContent: "fn main() { updated(); }\n",
      beforeHash: sha("c"),
      afterHash: sha("d"),
      beforeBytes: 13,
      afterBytes: 26,
    }],
    totalChangedBytes: 26,
    createdAt: 1_000,
    expiresAt: 601_000,
  },
};

const appliedExecution: ChangesExecutionProjection = {
  executionId: "execution_01J00000000000000000000000",
  checkpointId: "checkpoint_01J00000000000000000000000",
  completedAt: 5_000,
  undoable: true,
  files: [{
    relativePath: "src/main.rs",
    operation: "modified",
    exists: true,
    contentHash: sha("d"),
  }],
};

const changesHistory: ChangesHistoryProjection = {
  workspaceId: "workspace_01J00000000000000000000000",
  entries: [{
    executionId: appliedExecution.executionId,
    journalState: "completed",
    fileCount: 1,
    completedAt: "2026-07-17T12:00:00Z",
    undoable: true,
  }],
  openJournals: [],
};

const recoveryReview: Extract<ChangesRecoveryPrepared, { status: "review_required" }> = {
  status: "review_required",
  recoveryApprovalId: "recovery_private_01J00000000000000000000000",
  displayedRecoveryHash: sha("e"),
  journalId: "journal_private_01J00000000000000000000000",
  executionId: "execution_private_01J00000000000000000000000",
  operations: [{
    relativePath: "src/main.rs",
    operation: "replace",
    explanation: "Restore the file content saved before the interrupted change.",
  }],
  expiresAt: 601_000,
};

function createProps(
  overrides: Partial<GovernedChangesPanelProps> = {},
): GovernedChangesPanelProps {
  return {
    canEnableEdits: false,
    enableEditsBusy: false,
    errorMessage: null,
    onDecide: vi.fn(),
    onEnableEdits: vi.fn(),
    onDecideRecovery: vi.fn(),
    onPrepareRecovery: vi.fn(),
    onRefreshHistory: vi.fn(),
    onStartNewProposal: vi.fn(),
    onUndo: vi.fn(),
    history: null,
    historyBusy: false,
    recoveryBusy: false,
    recoveryReturnFocusTarget: null,
    recoveryReview: null,
    state: { kind: "idle" },
    ...overrides,
  };
}

afterEach(cleanup);

describe("GovernedChangesPanel", () => {
  it("opens and closes recovery from the keyboard and restores Changes focus", async () => {
    const user = userEvent.setup();
    const onPrepareRecovery = vi.fn();
    const onDecideRecovery = vi.fn();
    const history: ChangesHistoryProjection = {
      ...changesHistory,
      entries: [],
      openJournals: [{
        journalId: "journal_keyboard_01J00000000000000000000000",
        executionId: "execution_keyboard_01J00000000000000000000000",
        state: "recovery_required",
        updatedAt: "2026-07-18T00:00:00Z",
        recoveryAvailability: "review_available",
      }],
    };
    const base = createProps({ history, onDecideRecovery, onPrepareRecovery });
    const { rerender } = render(<GovernedChangesPanel {...base} />);
    const trigger = screen.getByRole("button", { name: "Review recovery" });
    trigger.focus();
    await user.keyboard("{Enter}");
    expect(onPrepareRecovery).toHaveBeenCalledWith(
      "journal_keyboard_01J00000000000000000000000",
      trigger,
    );

    rerender(<GovernedChangesPanel {...base}
      recoveryReturnFocusTarget={trigger}
      recoveryReview={recoveryReview}
    />);
    expect(document.activeElement).toBe(screen.getByRole("heading", {
      name: "Review checkpoint recovery",
    }));
    await user.tab();
    await user.keyboard(" ");
    expect(onDecideRecovery).toHaveBeenCalledWith("cancel");
    rerender(<GovernedChangesPanel {...base} recoveryReturnFocusTarget={trigger} />);
    expect(document.activeElement).toBe(trigger);
  });

  it("offers governed-edits enablement only when the host allows it", () => {
    const props = createProps({
      canEnableEdits: true,
      state: { kind: "unavailable", reason: "This workspace is read only." },
    });
    render(<GovernedChangesPanel {...props} />);
    expect(screen.getByText("This workspace is read only.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Allow governed edits" }));
    expect(props.onEnableEdits).toHaveBeenCalledTimes(1);

    cleanup();
    render(<GovernedChangesPanel {...createProps({
      state: { kind: "unavailable", reason: "Host unavailable." },
    })} />);
    expect(screen.queryByRole("button", { name: "Allow governed edits" })).toBeNull();
  });

  it("presents an idle chat-first empty state without a manual composer", () => {
    render(<GovernedChangesPanel {...createProps({ state: { kind: "idle" } })} />);
    expect(screen.getByRole("heading", { name: "No proposed changes" })).toBeTruthy();
    expect(
      screen.getByText(/Ask an agent in the task chat to make changes/u),
    ).toBeTruthy();
    expect(screen.queryByLabelText("Relative path")).toBeNull();
    expect(screen.queryByLabelText("Proposed content")).toBeNull();
    expect(screen.queryByRole("button", { name: /Review .*changes/u })).toBeNull();
  });

  /**
   * FE-23. The three controls were disabled on `state.busy`, which is host
   * state and so lands a render late. Until it does all three stay live, and
   * the dangerous press is not a second Apply -- it is a Discard arriving
   * behind one, because a per-button guard never sees two different buttons.
   */
  it("takes one decision per proposal, whichever controls are pressed", () => {
    const props = createProps({
      state: { kind: "review", busy: false, review: reviewEnvelope },
    });
    render(<GovernedChangesPanel {...props} />);

    /*
     * The nodes are captured before any of them is pressed, and pressed
     * without awaiting.
     *
     * Both details matter. Awaiting would let React re-render between presses
     * and hide the very race under test; re-querying by name would fail on the
     * third press, because the first one relabels Apply to "Applying…" -- so
     * the query, not the guard, would be what stopped it.
     */
    const apply = screen.getByRole("button", { name: "Apply changes" });
    const discard = screen.getByRole("button", { name: "Discard" });
    fireEvent.click(apply);
    fireEvent.click(discard);
    fireEvent.click(apply);

    expect(props.onDecide).toHaveBeenCalledTimes(1);
    expect(props.onDecide).toHaveBeenCalledWith("apply");
  });

  /**
   * The same double-press with no render between the two.
   *
   * The test above passes even against a guard read from render state, because
   * React flushes a discrete click before the next is dispatched and the
   * disabled attribute lands in time. Inside one batch -- which `act`
   * reproduces, and which a deferred or transition-wrapped render does in
   * earnest -- no re-render happens between the presses, and a guard read from
   * render state sees the state from before the first one.
   *
   * The ref is what closes that. This is the test that says so, and without it
   * the mutation swapping the ref for render state survives.
   */
  it("still takes one decision when both presses land in a single batch", () => {
    const props = createProps({
      state: { kind: "review", busy: false, review: reviewEnvelope },
    });
    render(<GovernedChangesPanel {...props} />);
    const apply = screen.getByRole("button", { name: "Apply changes" });
    const discard = screen.getByRole("button", { name: "Discard" });

    act(() => {
      apply.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      discard.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });

    expect(props.onDecide).toHaveBeenCalledTimes(1);
    expect(props.onDecide).toHaveBeenCalledWith("apply");
  });

  it("says which decision is running, and only on that control", () => {
    // Three disabled buttons explain nothing to someone who has just committed
    // to writing files. Relabelling all three would say three things run.
    const props = createProps({
      state: { kind: "review", busy: false, review: reviewEnvelope },
    });
    render(<GovernedChangesPanel {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "Apply changes" }));

    expect(screen.getByRole("button", { name: "Applying…" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Revise" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Discard" })).toBeTruthy();
  });

  it("re-opens the decision when a new proposal arrives", () => {
    // A different approval is a different decision to make. Carrying the guard
    // across would leave the next proposal permanently undecidable.
    const props = createProps({
      state: { kind: "review", busy: false, review: reviewEnvelope },
    });
    const { rerender } = render(<GovernedChangesPanel {...props} />);
    fireEvent.click(screen.getByRole("button", { name: "Apply changes" }));

    rerender(<GovernedChangesPanel {...createProps({
      onDecide: props.onDecide,
      state: {
        kind: "review",
        busy: false,
        review: { ...reviewEnvelope, approvalId: "approval_second" },
      },
    })} />);
    fireEvent.click(screen.getByRole("button", { name: "Apply changes" }));

    expect(props.onDecide).toHaveBeenCalledTimes(2);
  });

  it("renders the exact reviewed content as a line diff and binds each decision", () => {
    const props = createProps({
      state: { kind: "review", busy: false, review: reviewEnvelope },
    });
    render(<GovernedChangesPanel {...props} />);
    expect(screen.getByRole("heading", { name: "Review changes" })).toBeTruthy();
    const diff = screen.getByRole("region", { name: "Changes to src/main.rs" });
    expect(diff.textContent).toContain("fn main() {}");
    expect(diff.textContent).toContain("fn main() { updated(); }");
    expect(diff.querySelector(".diff-line--removed")?.textContent).toContain("fn main() {}");
    expect(diff.querySelector(".diff-line--added")?.textContent).toContain(
      "fn main() { updated(); }",
    );

    /*
     * Each control is proven on its own proposal, not three in a row on one.
     *
     * This used to press all three against a single review and assert three
     * calls -- which is now impossible by design: a reviewed proposal takes
     * exactly one decision. Pressing Apply and then Discard was never a
     * sequence anyone should be able to perform, and the old assertion was
     * documenting that they could.
     */
    fireEvent.click(screen.getByRole("button", { name: "Apply changes" }));
    expect(props.onDecide).toHaveBeenNthCalledWith(1, "apply");
    expect(props.onDecide).toHaveBeenCalledTimes(1);

    for (const [name, choice] of [["Revise", "revise"], ["Discard", "discard"]] as const) {
      cleanup();
      const fresh = createProps({
        state: { kind: "review", busy: false, review: reviewEnvelope },
      });
      render(<GovernedChangesPanel {...fresh} />);
      fireEvent.click(screen.getByRole("button", { name }));
      expect(fresh.onDecide).toHaveBeenCalledWith(choice);
      expect(fresh.onDecide).toHaveBeenCalledTimes(1);
    }
  });

  it("disables every decision while a decision is in flight", () => {
    render(<GovernedChangesPanel {...createProps({
      state: { kind: "review", busy: true, review: reviewEnvelope },
    })} />);
    for (const name of ["Apply changes", "Revise", "Discard"]) {
      expect((screen.getByRole("button", { name }) as HTMLButtonElement).disabled).toBe(true);
    }
  });

  it("offers Undo changes after an applied execution", () => {
    const props = createProps({
      state: { kind: "applied", busy: false, execution: appliedExecution },
    });
    render(<GovernedChangesPanel {...props} />);
    expect(screen.getByRole("heading", { name: "Changes applied" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Undo changes" }));
    expect(props.onUndo).toHaveBeenCalledWith(appliedExecution.executionId);
  });

  it("surfaces persistent change history and can request undo from it", () => {
    const props = createProps({ history: changesHistory, state: { kind: "idle" } });
    render(<GovernedChangesPanel {...props} />);

    expect(screen.getByRole("heading", { name: "Change history" })).toBeTruthy();
    expect(screen.getByText("1 file · completed")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Undo historical change" }));
    expect(props.onUndo).toHaveBeenCalledWith(appliedExecution.executionId);
    fireEvent.click(screen.getByRole("button", { name: "Refresh history" }));
    expect(props.onRefreshHistory).toHaveBeenCalledTimes(1);
  });

  it("opens the shared recovery review from history without exposing private bindings", () => {
    const onPrepareRecovery = vi.fn();
    const historyWithRecovery: ChangesHistoryProjection = {
      ...changesHistory,
      openJournals: [{
        journalId: recoveryReview.journalId,
        executionId: recoveryReview.executionId,
        state: "recovery_required",
        updatedAt: "2026-07-18T00:00:00Z",
        recoveryAvailability: "review_available",
      }],
    };
    const { rerender } = render(<GovernedChangesPanel {...createProps({
      history: historyWithRecovery,
      onPrepareRecovery,
    })} />);
    const trigger = screen.getByRole("button", { name: "Review recovery" });
    fireEvent.click(trigger);
    expect(onPrepareRecovery).toHaveBeenCalledWith(recoveryReview.journalId, trigger);

    rerender(<GovernedChangesPanel {...createProps({
      history: historyWithRecovery,
      recoveryReview,
      recoveryReturnFocusTarget: trigger,
    })} />);
    expect(screen.getByRole("heading", { name: "Review checkpoint recovery" })).toBeTruthy();
    expect(document.body.textContent).not.toContain(recoveryReview.displayedRecoveryHash);
    expect(document.body.textContent).not.toContain(recoveryReview.recoveryApprovalId);
  });

  it("shows quarantined and manual-review journals without a restore action", () => {
    render(<GovernedChangesPanel {...createProps({
      history: {
        ...changesHistory,
        openJournals: [
          {
            journalId: "journal_quarantined",
            executionId: "execution_quarantined",
            state: "recovery_required",
            updatedAt: "2026-07-18T00:00:00Z",
            recoveryAvailability: "quarantined",
          },
          {
            journalId: "journal_manual",
            executionId: "execution_manual",
            state: "manual_review",
            updatedAt: "2026-07-18T00:00:00Z",
            recoveryAvailability: "manual_review",
          },
        ],
      },
    })} />);
    expect(screen.getByText(/exact workspace and governed-edits grant/i)).toBeTruthy();
    expect(screen.getByText(/requires manual review outside this recovery flow/i)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Review recovery" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Restore checkpoint" })).toBeNull();
  });

  it("explains undo conflicts without offering an effect", () => {
    const props = createProps({
      state: {
        kind: "undo_unavailable",
        value: {
          executionId: appliedExecution.executionId,
          reason: "The workspace changed after this change was applied.",
          conflicts: [{
            relativePath: "src/main.rs",
            expectedExists: true,
            currentExists: true,
          }],
        },
      },
    });
    render(<GovernedChangesPanel {...props} />);
    expect(screen.getByRole("heading", { name: "Undo changes is unavailable" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Undo changes" })).toBeNull();
    expect(screen.getByRole("list", { name: "Undo conflicts" })).toBeTruthy();
  });

  it("surfaces safe host errors as an alert", () => {
    render(<GovernedChangesPanel {...createProps({
      errorMessage: "The workspace changed after review.",
      state: { kind: "review", busy: false, review: reviewEnvelope },
    })} />);
    expect(screen.getByRole("alert").textContent).toContain(
      "The workspace changed after review.",
    );
  });
});
