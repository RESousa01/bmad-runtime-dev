/**
 * Activity: what this app did, and the proof of each claim.
 *
 * FE-24. Every decision about *what is true* lives in
 * `features/activity/activityJournal` -- which states may become an entry,
 * what counts as a time, and which of "nothing happened", "we have not
 * looked" and "we looked and could not read it" the journal is entitled to
 * say. This file only lays that out.
 *
 * The one judgement made here is which controls to offer. Refresh is withdrawn
 * when `presentHostError` has already ruled that re-reading cannot work, and
 * the caveat says so in the host's own words instead. A control that cannot
 * succeed is worse than no control: it makes the failure look like the user's
 * to fix.
 */

import { Button } from "@sapphirus/ui";
import { History, RefreshCw, RotateCcw } from "lucide-react";
import type { BmadRequestState } from "../../lib/bmadModelProjection";
import type {
  ChangesHistoryProjection,
  ChangesRecoveryPrepared,
  LocalHostError,
  RecoveryApprovalChoice,
} from "../../lib/hostClient";
import {
  buildActivityJournal,
  describeRecovery,
  describeTime,
  type JournalCaveat,
  type JournalEntry,
  type JournalFact,
  type JournalTime,
} from "../../features/activity/activityJournal";
import { RecoveryReview } from "../RecoveryReview";

export interface ActivityPanelProps {
  helpState: BmadRequestState;
  history: ChangesHistoryProjection | null;
  historyBusy: boolean;
  historyAvailable: boolean;
  /// Whether the history has been read, so "nothing happened" is only said when
  /// it is known. `history === null` alone cannot tell "never asked" from
  /// "asked, empty", and the panel used to render both as "No activity yet".
  historyStatus: "unread" | "loading" | "loaded" | "unavailable";
  /// Why the read failed, when it did. Without this a failed read renders as an
  /// empty list, which claims the opposite of what happened.
  historyError: string | null;
  /// The failed read itself, where the shell keeps it.
  ///
  /// Optional so the existing wiring compiles unchanged: the shell currently
  /// flattens host failures to `historyError` before they reach here. Supplying
  /// it is what lets the panel print a correlation id and stop offering a
  /// Refresh the host has already ruled out -- both of which the panel must not
  /// guess at when it is absent.
  historyFailure?: LocalHostError | null;
  onRefreshHistory: () => void;
  onDecideRecovery: (choice: RecoveryApprovalChoice) => void;
  onPrepareRecovery: (journalId: string, trigger: HTMLElement) => void;
  onUndo: (executionId: string) => void;
  recoveryBusy: boolean;
  recoveryReturnFocusTarget: HTMLElement | null;
  recoveryReview: Extract<ChangesRecoveryPrepared, { status: "review_required" }> | null;
}

function EntryTime({ time }: { time: JournalTime }) {
  const text = describeTime(time);
  if (time.kind !== "recorded") {
    // Deliberately not a dash and not a relative phrase. The sentence says
    // which kind of nothing this is.
    return <span className="activity-panel__time activity-panel__time--absent">{text}</span>;
  }
  return (
    <time className="activity-panel__time" dateTime={new Date(time.epochMs).toISOString()}>
      {text}
    </time>
  );
}

function EntryFact({ fact }: { fact: JournalFact }) {
  return (
    <div className="activity-panel__fact">
      <dt>{fact.label}</dt>
      {/* Identifiers are the tokens a user may have to quote back to support,
          so they are rendered in a face that distinguishes 0 from O and are
          never abbreviated. */}
      <dd className={fact.form === "identifier" ? "activity-panel__id" : undefined}>
        {fact.value}
      </dd>
    </div>
  );
}

function EntryRow({
  busy,
  entry,
  onPrepareRecovery,
  onUndo,
}: {
  busy: boolean;
  entry: JournalEntry;
  onPrepareRecovery: (journalId: string, trigger: HTMLElement) => void;
  onUndo: (executionId: string) => void;
}) {
  const { action } = entry;
  return (
    <li className="activity-panel__entry">
      <div className="activity-panel__entry-main">
        <strong className="activity-panel__headline">{entry.headline}</strong>
        <EntryTime time={entry.time} />
        <dl className="activity-panel__facts">
          {/* The subject leads: an entry names what it is evidence of before
              it says anything else about it. */}
          <EntryFact fact={entry.subject} />
          {entry.facts.map((fact) => <EntryFact fact={fact} key={fact.label} />)}
        </dl>
        {entry.note === null
          ? null
          : <small className="activity-panel__note">{entry.note}</small>}
      </div>
      {action === null ? null : action.kind === "undo" ? (
        <Button
          aria-label={`Undo execution ${action.executionId}`}
          onPress={() => onUndo(action.executionId)}
          size="small"
          variant="quiet"
        >
          <RotateCcw aria-hidden="true" size={14} /> Undo
        </Button>
      ) : (
        <Button
          aria-label="Review recovery"
          isDisabled={busy}
          onPress={(event) => {
            if (event.target instanceof HTMLElement) {
              onPrepareRecovery(action.journalId, event.target);
            }
          }}
          size="small"
          variant="secondary"
        >
          Review recovery
        </Button>
      )}
    </li>
  );
}

/** The reason, the id and the remedy -- each shown only when the host gave it. */
function CaveatDetail({ caveat }: { caveat: JournalCaveat }) {
  return (
    <>
      {caveat.reason === null
        ? null
        : <p className="activity-panel__caveat-reason">{caveat.reason}</p>}
      {caveat.recovery === null
        ? null
        : <p className="activity-panel__caveat-recovery">{describeRecovery(caveat.recovery)}</p>}
      {caveat.correlationId === null ? null : (
        <p className="activity-panel__caveat-correlation">
          Correlation ID <code>{caveat.correlationId}</code>
        </p>
      )}
    </>
  );
}

export function ActivityPanel({
  helpState,
  history,
  historyAvailable,
  historyBusy,
  historyStatus,
  historyError,
  historyFailure = null,
  onRefreshHistory,
  onDecideRecovery,
  onPrepareRecovery,
  onUndo,
  recoveryBusy,
  recoveryReturnFocusTarget,
  recoveryReview,
}: ActivityPanelProps) {
  const journal = buildActivityJournal({
    guidance: helpState,
    history,
    historyError,
    historyFailure,
    historyRecorded: historyAvailable,
    historyStatus,
  });
  const { caveat } = journal;
  // Withdrawn rather than disabled: a greyed-out Refresh reads as "wait a
  // moment", and this is the host saying re-reading will never work.
  const refreshRuledOut = caveat?.recovery?.kind === "none";
  const reviewBusy = historyBusy || recoveryBusy || recoveryReview !== null;

  return (
    <section aria-label="Workspace activity" className="activity-panel">
      <div className="activity-panel__toolbar">
        <p>Read-only record of governed executions and skill-guidance runs.</p>
        {historyAvailable && !refreshRuledOut ? (
          <Button
            aria-label="Refresh activity"
            isDisabled={historyBusy || recoveryBusy}
            onPress={onRefreshHistory}
            size="small"
            variant="quiet"
          >
            <RefreshCw aria-hidden="true" size={14} />
            {historyBusy ? "Refreshing" : "Refresh"}
          </Button>
        ) : null}
      </div>

      {recoveryReview !== null ? (
        <RecoveryReview
          busy={recoveryBusy}
          onDecide={onDecideRecovery}
          returnFocusTarget={recoveryReturnFocusTarget}
          review={recoveryReview}
        />
      ) : null}

      {caveat !== null && caveat.placement === "banner" ? (
        <div
          className={caveat.tone === "alert"
            ? "activity-panel__banner activity-panel__banner--alert"
            : "activity-panel__banner"}
          role={caveat.tone === "alert" ? "alert" : "status"}
        >
          <p className="activity-panel__caveat-body">{caveat.body}</p>
          <CaveatDetail caveat={caveat} />
        </div>
      ) : null}

      {caveat !== null && caveat.placement === "empty" ? (
        <div
          className="activity-panel__empty"
          role={caveat.tone === "alert" ? "alert" : "status"}
        >
          <History aria-hidden="true" size={22} />
          <h3>{caveat.title}</h3>
          <p className="activity-panel__caveat-body">{caveat.body}</p>
          <CaveatDetail caveat={caveat} />
        </div>
      ) : null}

      {journal.sections.map((section) => (
        <section className="activity-panel__section" key={section.key}>
          <h3 className="activity-panel__section-title">{section.title}</h3>
          <p className="activity-panel__section-caption">{section.caption}</p>
          <ul aria-label={section.title} className="activity-panel__list">
            {section.entries.map((entry) => (
              <EntryRow
                busy={reviewBusy}
                entry={entry}
                key={entry.id}
                onPrepareRecovery={onPrepareRecovery}
                onUndo={onUndo}
              />
            ))}
          </ul>
        </section>
      ))}

      {journal.unattributed === null ? null : (
        // Below the record on purpose. It is not an entry -- nothing here can
        // be checked against a run -- but dropping it would lose the fact that
        // a run ended at all.
        <p className="activity-panel__unattributed">{journal.unattributed}</p>
      )}
    </section>
  );
}
