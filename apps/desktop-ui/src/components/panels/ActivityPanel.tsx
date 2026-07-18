import { Button } from "@sapphirus/ui";
import { History, RefreshCw, RotateCcw, ShieldAlert } from "lucide-react";
import type { BmadRequestState } from "../../lib/bmadModelProjection";
import type {
  ChangesHistoryProjection,
  ChangesRecoveryPrepared,
  RecoveryApprovalChoice,
} from "../../lib/hostClient";
import { RecoveryReview } from "../RecoveryReview";

export interface ActivityPanelProps {
  helpState: BmadRequestState;
  history: ChangesHistoryProjection | null;
  historyBusy: boolean;
  historyAvailable: boolean;
  onRefreshHistory: () => void;
  onDecideRecovery: (choice: RecoveryApprovalChoice) => void;
  onPrepareRecovery: (journalId: string, trigger: HTMLElement) => void;
  onUndo: (executionId: string) => void;
  recoveryBusy: boolean;
  recoveryReturnFocusTarget: HTMLElement | null;
  recoveryReview: Extract<ChangesRecoveryPrepared, { status: "review_required" }> | null;
}

function journalStateLabel(state: string): string {
  return state.replaceAll("_", " ");
}

function helpSummary(helpState: BmadRequestState): {
  detail: string;
  label: string;
} | null {
  switch (helpState.kind) {
    case "idle":
      return null;
    case "creating":
      return { label: "Skill guidance", detail: "Preparing a local review" };
    case "review_required":
      return { label: "Skill guidance", detail: "Review required · nothing sent" };
    case "approving":
      return { label: "Skill guidance", detail: "Approving · nothing sent" };
    case "approved":
      return { label: "Skill guidance", detail: "Approved · ready to send" };
    case "submitting":
      return { label: "Skill guidance", detail: "Sending the approved context once" };
    case "completed": {
      const receipt = helpState.result.receipt;
      return {
        label: "Skill guidance completed",
        detail: `Verified receipt · ${receipt.inputBytes} bytes out, ${receipt.outputBytes} bytes back`,
      };
    }
    case "interrupted":
      return { label: "Skill guidance", detail: "Interrupted · cannot resume" };
    case "terminal":
      return { label: "Skill guidance", detail: "Review ended" };
    case "unavailable":
      return { label: "Skill guidance", detail: helpState.message };
    default:
      return { label: "Skill guidance", detail: "Retained local run available" };
  }
}

export function ActivityPanel({
  helpState,
  history,
  historyAvailable,
  historyBusy,
  onRefreshHistory,
  onDecideRecovery,
  onPrepareRecovery,
  onUndo,
  recoveryBusy,
  recoveryReturnFocusTarget,
  recoveryReview,
}: ActivityPanelProps) {
  const help = helpSummary(helpState);
  const entries = history?.entries ?? [];
  const openJournals = history?.openJournals ?? [];

  return (
    <section aria-label="Workspace activity" className="activity-panel">
      <div className="activity-panel__toolbar">
        <p>Read-only record of governed executions and skill-guidance runs.</p>
        {historyAvailable ? (
          <Button
            aria-label="Refresh activity"
            isDisabled={historyBusy}
            onPress={onRefreshHistory}
            size="small"
            variant="quiet"
          >
            <RefreshCw aria-hidden="true" size={14} />
            {historyBusy ? "Refreshing" : "Refresh"}
          </Button>
        ) : null}
      </div>

      {openJournals.length > 0 ? (
        <div className="activity-panel__journal-banner" role="status">
          <ShieldAlert aria-hidden="true" size={15} />
          <span>
            {openJournals.length === 1
              ? "One execution journal needs attention"
              : `${openJournals.length} execution journals need attention`}
            {" · "}
            {openJournals.map((journal) => journalStateLabel(journal.state)).join(", ")}
          </span>
        </div>
      ) : null}

      {recoveryReview !== null ? (
        <RecoveryReview
          busy={recoveryBusy}
          onDecide={onDecideRecovery}
          returnFocusTarget={recoveryReturnFocusTarget}
          review={recoveryReview}
        />
      ) : null}

      {openJournals.length > 0 ? (
        <div
          aria-label="Open recovery journals"
          className="activity-panel__list"
          key="open-recovery-journals"
          role="list"
        >
          {openJournals.map((journal) => (
            <div className="activity-panel__entry" key={journal.journalId} role="listitem">
              <div className="activity-panel__entry-main">
                <strong>{journalStateLabel(journal.state)}</strong>
                <small>
                  {journal.recoveryAvailability === "quarantined"
                    ? "Select the exact workspace and governed-edits grant to review recovery."
                    : journal.recoveryAvailability === "manual_review"
                      ? "This journal requires manual review outside this recovery flow."
                      : "A bounded checkpoint review is available."}
                </small>
              </div>
              {journal.recoveryAvailability === "review_available" ? (
                <Button
                  aria-label="Review recovery"
                  isDisabled={historyBusy || recoveryBusy || recoveryReview !== null}
                  onPress={(event) => {
                    if (event.target instanceof HTMLElement) {
                      onPrepareRecovery(journal.journalId, event.target);
                    }
                  }}
                  size="small"
                  variant="secondary"
                >
                  Review recovery
                </Button>
              ) : null}
            </div>
          ))}
        </div>
      ) : null}

      {help ? (
        <article className="activity-panel__entry activity-panel__entry--help">
          <div className="activity-panel__entry-main">
            <strong>{help.label}</strong>
            <small>{help.detail}</small>
          </div>
        </article>
      ) : null}

      {entries.length === 0 && help === null && openJournals.length === 0 ? (
        <div className="activity-panel__empty">
          <History aria-hidden="true" size={22} />
          <h3>No activity yet</h3>
          <p>
            {historyAvailable
              ? "Governed changes and skill-guidance runs will appear here after they happen."
              : "Enable governed edits on a workspace to record executions here."}
          </p>
        </div>
      ) : (
        <ul aria-label="Governed executions" className="activity-panel__list">
          {entries.map((entry) => (
            <li className="activity-panel__entry" key={entry.executionId}>
              <div className="activity-panel__entry-main">
                <strong>
                  {entry.fileCount === 1 ? "1 file changed" : `${entry.fileCount} files changed`}
                </strong>
                <small>
                  {journalStateLabel(entry.journalState)} · {entry.completedAt}
                </small>
              </div>
              {entry.undoable ? (
                <Button
                  aria-label={`Undo execution ${entry.executionId}`}
                  onPress={() => onUndo(entry.executionId)}
                  size="small"
                  variant="quiet"
                >
                  <RotateCcw aria-hidden="true" size={14} /> Undo
                </Button>
              ) : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
