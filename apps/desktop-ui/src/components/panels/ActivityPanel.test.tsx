// @vitest-environment jsdom
import "../../test/setup";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { initialBmadRequestState, type BmadRequestState } from "../../lib/bmadModelProjection";
import type { ChangesHistoryProjection, ChangesRecoveryPrepared } from "../../lib/hostClient";
import { ActivityPanel, type ActivityPanelProps } from "./ActivityPanel";

function createProps(overrides: Partial<ActivityPanelProps> = {}): ActivityPanelProps {
  return {
    helpState: initialBmadRequestState,
    history: null,
    historyAvailable: true,
    historyBusy: false,
    // Default to a completed read, so a test that does not care about the read
    // state exercises the same "loaded and empty" branch it always did.
    historyStatus: "loaded",
    historyError: null,
    historyFailure: null,
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

const completedGuidance: BmadRequestState = {
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
      createdAt: 1_784_712_600_000,
    },
    receipt: {
      schemaVersion: "bmad-model-receipt-summary.v1",
      receiptId: "receipt_1",
      status: "succeeded",
      retentionMode: "transient_no_store",
      region: "localdev",
      inputBytes: 512,
      outputBytes: 256,
      startedAt: 1_784_712_599_000,
      completedAt: 1_784_712_600_000,
    },
  },
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
});

describe("the three things the panel can say when it has nothing to show", () => {
  it("says nothing happened once the journal has been read and is empty", () => {
    render(<ActivityPanel {...createProps({
      history: { workspaceId: "workspace_1", entries: [], openJournals: [] },
    })} />);
    expect(screen.getByRole("heading", { name: "Nothing has happened yet" })).toBeTruthy();
  });

  /// "Nothing has happened" is a claim about the workspace. It used to be
  /// rendered from `history === null`, which is also what "never asked" looks
  /// like — and nothing asked, because the read was reachable only from the
  /// Refresh button. So a user opening Activity to check whether yesterday's
  /// change could still be rolled back was told there was nothing to roll back.
  it("says the journal has not been read rather than that nothing happened", () => {
    render(<ActivityPanel {...createProps({ history: null, historyStatus: "unread" })} />);
    expect(
      screen.getByRole("heading", { name: "Activity has not been read yet" }),
    ).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Nothing has happened yet" })).toBeNull();
  });

  it("surfaces a failed read as an error, not as an empty list", () => {
    render(<ActivityPanel {...createProps({
      history: null,
      historyStatus: "unavailable",
      historyError: "The host could not read the journal.",
    })} />);
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("cannot say whether anything happened");
    // The host's own reason has to survive to the user, or the panel is just
    // asserting failure without saying what failed.
    expect(alert.textContent).toContain("The host could not read the journal.");
    expect(screen.queryByRole("heading", { name: "Nothing has happened yet" })).toBeNull();
  });

  it("does not claim an empty workspace when the host keeps no journal for it", () => {
    // Governed edits being off is a fact about settings, not evidence that
    // nothing happened.
    render(<ActivityPanel {...createProps({ historyAvailable: false })} />);
    expect(
      screen.getByRole("heading", { name: "No execution journal for this workspace" }),
    ).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Nothing has happened yet" })).toBeNull();
  });

  it("keeps reporting a failed read when it does have something to show", () => {
    // One guidance entry used to be enough to swallow the fact that the
    // governed-change half of the journal was missing entirely.
    render(<ActivityPanel {...createProps({
      helpState: completedGuidance,
      history: null,
      historyStatus: "unavailable",
      historyError: "The host could not read the journal.",
    })} />);
    expect(screen.getByRole("alert").textContent)
      .toContain("cannot say whether anything happened");
    expect(screen.getByText("Skill guidance run completed")).toBeTruthy();
  });
});

describe("entries carry the evidence they are about", () => {
  it("names the execution each governed change is evidence of", () => {
    render(<ActivityPanel {...createProps({ history })} />);
    expect(screen.getByText("Governed change · 2 files")).toBeTruthy();
    expect(screen.getByText("Governed change · 1 file")).toBeTruthy();
    expect(screen.getByText("execution_1")).toBeTruthy();
    expect(screen.getByText("execution_2")).toBeTruthy();
  });

  it("shows the receipt a completed guidance run is proved by", () => {
    render(<ActivityPanel {...createProps({ helpState: completedGuidance })} />);
    expect(screen.getByText("Skill guidance run completed")).toBeTruthy();
    expect(screen.getByText("receipt_1")).toBeTruthy();
    expect(screen.getByText("run_1")).toBeTruthy();
    expect(screen.getByText("transient_no_store")).toBeTruthy();
    expect(screen.getByText("512 bytes")).toBeTruthy();
    expect(screen.getByText("256 bytes")).toBeTruthy();
  });

  it("only offers undo where the host said the execution is undoable", async () => {
    const onUndo = vi.fn();
    const user = userEvent.setup();
    render(<ActivityPanel {...createProps({ history, onUndo })} />);

    const undoButtons = screen.getAllByRole("button", { name: /Undo execution/ });
    expect(undoButtons).toHaveLength(1);
    await user.click(undoButtons[0]!);
    expect(onUndo).toHaveBeenCalledWith("execution_1");
  });

  it("holds unresolved journals in their own section, newest record apart", () => {
    render(<ActivityPanel {...createProps({
      history: {
        ...history,
        openJournals: [{
          journalId: "journal_1",
          executionId: "execution_9",
          state: "recovery_required",
          updatedAt: "2026-07-19T00:00:00Z",
          recoveryAvailability: "review_available",
        }],
      },
    })} />);
    const attention = screen.getByRole("list", { name: "Needs attention" });
    expect(within(attention).getByText("Recovery required")).toBeTruthy();
    expect(within(attention).getByText("journal_1")).toBeTruthy();
    // The unresolved journal has the newest time of anything here, and still
    // must not be filed into the record as though it had finished.
    const recorded = screen.getByRole("list", { name: "Recorded" });
    expect(within(recorded).queryByText("journal_1")).toBeNull();
  });

  it("keeps quarantined and manual-review journals non-actionable, and says why", async () => {
    const user = userEvent.setup();
    const onPrepareRecovery = vi.fn();
    render(<ActivityPanel {...createProps({
      onPrepareRecovery,
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
    await user.click(screen.getByRole("button", { name: "Refresh activity" }));
    expect(onPrepareRecovery).not.toHaveBeenCalled();
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
});

describe("times", () => {
  it("renders a recorded instant in the host's zone, machine-readably", () => {
    render(<ActivityPanel {...createProps({
      history: { ...history, entries: [history.entries[0]!] },
    })} />);
    const time = screen.getByText("Completed 2026-07-18 00:00:00 UTC");
    expect(time.tagName).toBe("TIME");
    expect(time.getAttribute("datetime")).toBe("2026-07-18T00:00:00.000Z");
  });

  it("renders an absent time as absent, never as now and never as a dash", () => {
    render(<ActivityPanel {...createProps({
      history: {
        ...history,
        entries: [{ ...history.entries[0]!, completedAt: "" }],
      },
    })} />);
    expect(screen.getByText("No time recorded.")).toBeTruthy();
    expect(screen.queryByText("—")).toBeNull();
    // Nothing may present itself as a timestamp when there is none to present.
    expect(document.querySelector("time")).toBeNull();
  });

  it("shows a host value that is not a time rather than smoothing it over", () => {
    render(<ActivityPanel {...createProps({
      history: {
        ...history,
        entries: [{ ...history.entries[0]!, completedAt: "yesterday" }],
      },
    })} />);
    expect(screen.getByText(/recorded completed as “yesterday”/)).toBeTruthy();
    expect(document.querySelector("time")).toBeNull();
  });

  it("does not roll a date the calendar does not have into one it does", () => {
    // `Date.parse` turns 2026-02-30 into 2026-03-02 without complaint.
    render(<ActivityPanel {...createProps({
      history: {
        ...history,
        entries: [{ ...history.entries[0]!, completedAt: "2026-02-30T00:00:00Z" }],
      },
    })} />);
    expect(screen.queryByText(/2026-03-02/)).toBeNull();
    expect(screen.getByText(/recorded completed as “2026-02-30T00:00:00Z”/)).toBeTruthy();
  });
});

describe("what the panel offers to do about a failed read", () => {
  it("keeps Refresh when the host says the read could go differently", () => {
    render(<ActivityPanel {...createProps({
      history: null,
      historyStatus: "unavailable",
      historyFailure: {
        code: "transport_failed",
        safeMessage: "The transport dropped.",
        retryable: true,
        correlationId: null,
      },
    })} />);
    expect(screen.getByRole("button", { name: "Refresh activity" })).toBeTruthy();
    expect(screen.getByText("Refreshing may read it.")).toBeTruthy();
  });

  it("withdraws Refresh when the host has already ruled the failure final", () => {
    // A retry the host ruled out is a control that cannot succeed, and makes
    // the failure look like the user's to fix.
    render(<ActivityPanel {...createProps({
      history: null,
      historyStatus: "unavailable",
      historyFailure: {
        code: "transport_failed",
        safeMessage: "The transport dropped.",
        retryable: false,
        correlationId: null,
      },
    })} />);
    expect(screen.queryByRole("button", { name: "Refresh activity" })).toBeNull();
    expect(screen.getByText(/reported this as final/)).toBeTruthy();
  });

  it("names the remedy the shared table gave rather than inventing one", () => {
    render(<ActivityPanel {...createProps({
      history: null,
      historyStatus: "unavailable",
      historyFailure: {
        code: "unauthorized",
        safeMessage: "That key was rejected.",
        retryable: false,
        correlationId: null,
      },
    })} />);
    expect(screen.getByText("Sign in again, then refresh.")).toBeTruthy();
    // `unauthorized` was never a retry, so the host's `retryable: false` must
    // not have withdrawn a control that was not the retry.
    expect(screen.getByRole("button", { name: "Refresh activity" })).toBeTruthy();
  });

  it("prints a correlation id when the failure carried one", () => {
    render(<ActivityPanel {...createProps({
      history: null,
      historyStatus: "unavailable",
      historyFailure: {
        code: "internal",
        safeMessage: "The host failed.",
        retryable: true,
        correlationId: "corr_01J7",
      },
    })} />);
    expect(screen.getByText("corr_01J7")).toBeTruthy();
  });

  it("prints no correlation id and no remedy when the shell gave neither", () => {
    render(<ActivityPanel {...createProps({
      history: null,
      historyStatus: "unavailable",
      historyError: "The host could not read the journal.",
    })} />);
    const alert = screen.getByRole("alert");
    expect(alert.textContent).not.toContain("Correlation ID");
    expect(alert.textContent).not.toContain("Refreshing may read it.");
    // Not knowing the remedy is not the same as knowing there is none, so
    // Refresh stays.
    expect(screen.getByRole("button", { name: "Refresh activity" })).toBeTruthy();
  });
});

describe("what the journal cannot attribute", () => {
  it("states a run the host ended without naming, outside the record", () => {
    render(<ActivityPanel {...createProps({
      helpState: { kind: "terminal", reason: "consent_expired" },
      history,
    })} />);
    const note = screen.getByText(/A skill-guidance run ended \(consent expired\)/);
    expect(note.textContent).toContain("did not name the run");
    // It is not an entry: it names nothing that could be checked, so it must
    // not sit in a list of things that can be.
    expect(note.closest("li")).toBeNull();
  });

  it("does not narrate a request that is merely in flight", () => {
    // Status about work in progress is what this surface stopped being.
    render(<ActivityPanel {...createProps({
      helpState: { kind: "creating", activity: "creating" },
    })} />);
    expect(screen.getByRole("heading", { name: "Nothing has happened yet" })).toBeTruthy();
    expect(screen.queryByText(/Preparing/)).toBeNull();
  });
});
