// @vitest-environment jsdom
import "../../test/setup";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { initialBmadRequestState } from "../../lib/bmadModelProjection";
import type { ChangesHistoryProjection, ChangesRecoveryPrepared } from "../../lib/hostClient";
import { ActivityPanel, type ActivityPanelProps } from "./ActivityPanel";

function createProps(overrides: Partial<ActivityPanelProps> = {}): ActivityPanelProps {
  return {
    helpState: initialBmadRequestState,
    history: null,
    historyAvailable: true,
    historyBusy: false,
    onRefreshHistory: vi.fn(),
    onDecideRecovery: vi.fn(),
    onPrepareRecovery: vi.fn(),
    onUndo: vi.fn(),
    recoveryBusy: false,
    recoveryReturnFocusTarget: null,
    recoveryReview: null,
    ...overrides,
  };
}

const history: ChangesHistoryProjection = {
  workspaceId: "workspace_1",
  entries: [
    {
      executionId: "execution_1",
      journalState: "completed",
      fileCount: 2,
      completedAt: "2026-07-18T00:00:00Z",
      undoable: true,
    },
    {
      executionId: "execution_2",
      journalState: "completed",
      fileCount: 1,
      completedAt: "2026-07-17T23:00:00Z",
      undoable: false,
    },
  ],
  openJournals: [],
};

const recoveryReview: Extract<ChangesRecoveryPrepared, { status: "review_required" }> = {
  status: "review_required",
  recoveryApprovalId: "recovery_private_01J00000000000000000000000",
  displayedRecoveryHash: `sha256:${"e".repeat(64)}`,
  journalId: "journal_private_01J00000000000000000000000",
  executionId: "execution_private_01J00000000000000000000000",
  operations: [{
    relativePath: "src/main.rs",
    operation: "replace",
    explanation: "Restore the file content saved before the interrupted change.",
  }],
  expiresAt: 601_000,
};

describe("ActivityPanel", () => {
  it("opens and closes recovery from the keyboard and restores Activity focus", async () => {
    const user = userEvent.setup();
    const onPrepareRecovery = vi.fn();
    const onDecideRecovery = vi.fn();
    const recoveryHistory: ChangesHistoryProjection = {
      ...history,
      entries: [],
      openJournals: [{
        journalId: "journal_keyboard_1",
        executionId: "execution_keyboard_1",
        state: "recovery_required",
        updatedAt: "2026-07-18T00:00:00Z",
        recoveryAvailability: "review_available",
      }],
    };
    const base = createProps({
      history: recoveryHistory,
      onDecideRecovery,
      onPrepareRecovery,
    });
    const { rerender } = render(<ActivityPanel {...base} />);
    const trigger = screen.getByRole("button", { name: "Review recovery" });
    trigger.focus();
    await user.keyboard(" ");
    expect(onPrepareRecovery).toHaveBeenCalledWith("journal_keyboard_1", trigger);

    rerender(<ActivityPanel {...base}
      recoveryReturnFocusTarget={trigger}
      recoveryReview={recoveryReview}
    />);
    expect(document.activeElement).toBe(screen.getByRole("heading", {
      name: "Review checkpoint recovery",
    }));
    await user.tab();
    await user.keyboard("{Enter}");
    expect(onDecideRecovery).toHaveBeenCalledWith("cancel");
    rerender(<ActivityPanel {...base} recoveryReturnFocusTarget={trigger} />);
    expect(document.activeElement).toBe(trigger);
  });

  it("shows an empty state when there is no activity", () => {
    render(<ActivityPanel {...createProps()} />);
    expect(screen.getByRole("heading", { name: "No activity yet" })).toBeTruthy();
  });

  it("lists governed executions and only offers undo where allowed", async () => {
    const onUndo = vi.fn();
    const user = userEvent.setup();
    render(<ActivityPanel {...createProps({ history, onUndo })} />);

    expect(screen.getByText("2 files changed")).toBeTruthy();
    expect(screen.getByText("1 file changed")).toBeTruthy();
    const undoButtons = screen.getAllByRole("button", { name: /Undo execution/ });
    expect(undoButtons).toHaveLength(1);

    await user.click(undoButtons[0]!);
    expect(onUndo).toHaveBeenCalledWith("execution_1");
  });

  it("surfaces an open-journal banner", () => {
    render(
      <ActivityPanel
        {...createProps({
          history: {
            ...history,
            entries: [],
            openJournals: [
              {
                journalId: "journal_1",
                executionId: "execution_9",
                state: "recovery_required",
                updatedAt: "2026-07-18T00:00:00Z",
                recoveryAvailability: "review_available",
              },
            ],
          },
        })}
      />,
    );
    expect(screen.getByText(/execution journal needs attention/)).toBeTruthy();
    expect(screen.getAllByText(/recovery required/)).toHaveLength(2);
  });

  it("uses the same recovery preparation entry point from Activity", async () => {
    const user = userEvent.setup();
    const onPrepareRecovery = vi.fn();
    render(<ActivityPanel {...createProps({
      onPrepareRecovery,
      history: {
        ...history,
        entries: [],
        openJournals: [{
          journalId: "journal_1",
          executionId: "execution_9",
          state: "recovery_required",
          updatedAt: "2026-07-18T00:00:00Z",
          recoveryAvailability: "review_available",
        }],
      },
    })} />);
    const trigger = screen.getByRole("button", { name: "Review recovery" });
    await user.click(trigger);
    expect(onPrepareRecovery).toHaveBeenCalledWith("journal_1", trigger);
  });

  it("keeps quarantined and manual-review Activity journals non-actionable", () => {
    render(<ActivityPanel {...createProps({
      history: {
        ...history,
        entries: [],
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
  });

  it("summarizes a completed skill-guidance run", () => {
    render(
      <ActivityPanel
        {...createProps({
          helpState: {
            kind: "completed",
            result: {
              schemaVersion: "bmad-help-completed.v1",
              runKind: "bmad_help",
              lifecycle: "completed",
              workspaceId: "workspace_1",
              runId: "run_1",
              sessionId: "session_1",
              runnable: false,
              completionClaimed: true,
              recommendation: {
                recommendationKind: "recommended_capability",
                displayName: "Create Architecture",
                moduleCode: "bmm",
                skillName: "architecture-create",
                action: "create",
                evidenceClass: "authoritative",
                guidanceRequired: false,
                rationaleSummary: "Recommended next step",
                createdAt: 2,
              },
              receipt: {
                schemaVersion: "bmad-model-receipt-summary.v1",
                receiptId: "receipt_1",
                status: "succeeded",
                retentionMode: "transient_no_store",
                region: "local",
                inputBytes: 512,
                outputBytes: 256,
                startedAt: 1,
                completedAt: 2,
              },
            },
          },
        })}
      />,
    );
    expect(screen.getByText("Skill guidance completed")).toBeTruthy();
    expect(screen.getByText(/512 bytes out, 256 bytes back/)).toBeTruthy();
  });
});
