import { Button } from "@sapphirus/ui";
import {
  FilePlus2,
  FileX2,
  PencilLine,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  Trash2,
  Undo2,
} from "lucide-react";
import { useId, useState } from "react";
import type {
  ApprovalChoice,
  ChangesExecutionProjection,
  ChangesHistoryProjection,
  ChangesRecoveryPrepared,
  ChangesReviewEnvelopeProjection,
  ChangesReviewFileProjection,
  ChangesUndoUnavailableProjection,
  ProposedChange,
  RecoveryApprovalChoice,
} from "../lib/hostClient";
import { RecoveryReview } from "./RecoveryReview";

export type GovernedChangesUiState =
  | { kind: "unavailable"; reason: string }
  | { kind: "idle" }
  | { kind: "preparing" }
  | { kind: "review"; busy: boolean; review: ChangesReviewEnvelopeProjection }
  | { kind: "applied"; busy: boolean; execution: ChangesExecutionProjection }
  | { kind: "undo_unavailable"; value: ChangesUndoUnavailableProjection }
  | { kind: "discarded" };

export interface GovernedChangesPanelProps {
  canEnableEdits: boolean;
  enableEditsBusy: boolean;
  errorMessage: string | null;
  history: ChangesHistoryProjection | null;
  historyBusy: boolean;
  onDecide: (choice: ApprovalChoice) => void;
  onEnableEdits: () => void;
  onDecideRecovery: (choice: RecoveryApprovalChoice) => void;
  onPrepareRecovery: (journalId: string, trigger: HTMLElement) => void;
  onRefreshHistory: () => void;
  onPropose: (changes: readonly ProposedChange[]) => void;
  onStartNewProposal: () => void;
  onUndo: (executionId: string) => void;
  recoveryBusy: boolean;
  recoveryReturnFocusTarget: HTMLElement | null;
  recoveryReview: Extract<ChangesRecoveryPrepared, { status: "review_required" }> | null;
  state: GovernedChangesUiState;
}

function ChangeHistory({
  history,
  historyBusy,
  onRefreshHistory,
  onPrepareRecovery,
  onUndo,
  recoveryBusy,
}: {
  history: ChangesHistoryProjection | null;
  historyBusy: boolean;
  onRefreshHistory: () => void;
  onPrepareRecovery: (journalId: string, trigger: HTMLElement) => void;
  onUndo: (executionId: string) => void;
  recoveryBusy: boolean;
}) {
  return (
    <section aria-labelledby="change-history-heading" className="changes-history">
      <div className="inspector-section-heading">
        <h2 id="change-history-heading">Change history</h2>
        <Button
          aria-label="Refresh history"
          isDisabled={historyBusy || recoveryBusy}
          onPress={onRefreshHistory}
          size="small"
          variant="quiet"
        >
          <RefreshCw aria-hidden="true" size={15} />
          {historyBusy ? "Refreshing…" : "Refresh"}
        </Button>
      </div>
      {history?.openJournals.length ? (
        <div className="changes-error" role="alert">
          <ShieldAlert aria-hidden="true" size={16} />
          {history.openJournals.length} change journal{history.openJournals.length === 1 ? "" : "s"}
          {" require recovery review."}
        </div>
      ) : null}
      {history?.openJournals.length ? (
        <div aria-label="Open recovery journals" className="changes-history__list" role="list">
          {history.openJournals.map((journal) => (
            <div className="changes-history__row" key={journal.journalId} role="listitem">
              <div>
                <strong>{journal.state.replaceAll("_", " ")}</strong>
                {journal.recoveryAvailability === "quarantined" ? (
                  <p>Select the exact workspace and governed-edits grant to review recovery.</p>
                ) : journal.recoveryAvailability === "manual_review" ? (
                  <p>This journal requires manual review outside this recovery flow.</p>
                ) : (
                  <p>A bounded checkpoint review is available.</p>
                )}
              </div>
              {journal.recoveryAvailability === "review_available" ? (
                <Button
                  aria-label="Review recovery"
                  isDisabled={historyBusy || recoveryBusy}
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
      {history === null ? (
        <p className="changes-composer__hint">Refresh to load durable changes for this workspace.</p>
      ) : history.entries.length === 0 ? (
        <p className="changes-composer__hint">No applied changes have been recorded yet.</p>
      ) : (
        <div aria-label="Applied change history" className="changes-history__list" role="list">
          {history.entries.map((entry) => (
            <div className="changes-history__row" key={entry.executionId} role="listitem">
              <div>
                <strong>{entry.fileCount} {entry.fileCount === 1 ? "file" : "files"} · {entry.journalState}</strong>
                <time dateTime={entry.completedAt}>{entry.completedAt}</time>
              </div>
              <Button
                aria-label="Undo historical change"
                isDisabled={historyBusy || !entry.undoable}
                onPress={() => onUndo(entry.executionId)}
                size="small"
                variant="secondary"
              >
                <Undo2 aria-hidden="true" size={14} />
                Undo
              </Button>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function operationLabel(operation: ChangesReviewFileProjection["operation"]): string {
  switch (operation) {
    case "create":
      return "Create";
    case "modify":
      return "Modify";
    case "delete":
      return "Delete";
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`;
}

function ProposalComposer({
  disabled,
  onPropose,
}: {
  disabled: boolean;
  onPropose: (changes: readonly ProposedChange[]) => void;
}) {
  const operationFieldId = useId();
  const pathFieldId = useId();
  const contentFieldId = useId();
  const [operation, setOperation] = useState<ProposedChange["change"]>("set_content");
  const [relativePath, setRelativePath] = useState("");
  const [content, setContent] = useState("");
  const [changes, setChanges] = useState<ProposedChange[]>([]);
  const normalizedPath = relativePath.trim();
  const duplicatePath = changes.some((change) => change.relativePath === normalizedPath);
  const currentChange = normalizedPath.length === 0 || duplicatePath
    ? null
    : operation === "delete"
      ? { change: "delete" as const, relativePath: normalizedPath }
      : { change: "set_content" as const, relativePath: normalizedPath, content };
  const addDisabled = disabled || currentChange === null || changes.length >= 20;
  const submitDisabled = disabled || (changes.length === 0 && currentChange === null)
    || (normalizedPath.length > 0 && duplicatePath);

  const resetCurrentChange = () => {
    setOperation("set_content");
    setRelativePath("");
    setContent("");
  };

  const addCurrentChange = () => {
    if (!addDisabled && currentChange !== null) {
      setChanges((current) => [...current, currentChange]);
      resetCurrentChange();
    }
  };

  return (
    <form
      className="changes-composer"
      onSubmit={(event) => {
        event.preventDefault();
        if (!submitDisabled) {
          onPropose(currentChange === null ? changes : [...changes, currentChange]);
        }
      }}
    >
      <div className="inspector-section-heading">
        <h2>Proposed changes</h2>
        <span>Governed local edit</span>
      </div>
      <p className="changes-composer__hint">
        Compose up to 20 workspace-relative create, modify, or delete operations. The host
        observes every current file, and nothing changes until you review and apply.
      </p>
      <label htmlFor={operationFieldId}>Change operation</label>
      <select
        disabled={disabled}
        id={operationFieldId}
        onChange={(event) => setOperation(event.target.value as ProposedChange["change"])}
        value={operation}
      >
        <option value="set_content">Create or replace content</option>
        <option value="delete">Delete file</option>
      </select>
      <label htmlFor={pathFieldId}>Relative path</label>
      <input
        autoComplete="off"
        disabled={disabled}
        id={pathFieldId}
        onChange={(event) => setRelativePath(event.target.value)}
        placeholder="src/example.ts"
        spellCheck={false}
        type="text"
        value={relativePath}
      />
      {operation === "set_content" ? (
        <>
          <label htmlFor={contentFieldId}>Proposed content</label>
          <textarea
            disabled={disabled}
            id={contentFieldId}
            onChange={(event) => setContent(event.target.value)}
            rows={10}
            spellCheck={false}
            value={content}
          />
        </>
      ) : (
        <p className="changes-composer__warning">
          Delete is exact and fail-closed: the host will reject the proposal if the file changes
          before approval.
        </p>
      )}
      {duplicatePath ? (
        <p className="changes-composer__warning" role="alert">
          Each relative path can appear only once in a proposal.
        </p>
      ) : null}
      <Button
        isDisabled={addDisabled}
        onPress={addCurrentChange}
        size="large"
        type="button"
        variant="secondary"
      >
        <FilePlus2 aria-hidden="true" size={17} />
        Add file change
      </Button>
      {changes.length > 0 ? (
        <div aria-label="Draft file changes" className="changes-draft-list" role="list">
          {changes.map((change) => (
            <div className="changes-draft-row" key={change.relativePath} role="listitem">
              {change.change === "delete"
                ? <FileX2 aria-hidden="true" size={16} />
                : <PencilLine aria-hidden="true" size={16} />}
              <code>{change.relativePath}</code>
              <span>{change.change === "delete" ? "Delete" : "Set content"}</span>
              <Button
                aria-label={`Remove ${change.relativePath}`}
                isDisabled={disabled}
                onPress={() => setChanges((current) => current.filter(
                  (candidate) => candidate.relativePath !== change.relativePath,
                ))}
                size="small"
                type="button"
                variant="quiet"
              >
                <Trash2 aria-hidden="true" size={14} />
              </Button>
            </div>
          ))}
        </div>
      ) : null}
      <Button isDisabled={submitDisabled} size="large" type="submit" variant="primary">
        <PencilLine aria-hidden="true" size={17} />
        {changes.length === 0
          ? "Review changes"
          : `Review ${changes.length + (currentChange === null ? 0 : 1)} file changes`}
      </Button>
    </form>
  );
}

function ReviewFiles({ files }: { files: ChangesReviewFileProjection[] }) {
  return (
    <div className="changes-review-files">
      {files.map((file, index) => (
        <details className="changes-review-file" key={file.relativePath} open={index === 0}>
          <summary>
            {file.operation === "delete"
              ? <FileX2 aria-hidden="true" size={16} />
              : <FilePlus2 aria-hidden="true" size={16} />}
            <code>{file.relativePath}</code>
            <span>
              {operationLabel(file.operation)}
              {` · ${formatBytes(file.beforeBytes)} → ${formatBytes(file.afterBytes)}`}
            </span>
          </summary>
          {file.beforeContent !== null ? (
            <section aria-label={`Current content of ${file.relativePath}`}>
              <h4>Current</h4>
              <pre tabIndex={0}><code>{file.beforeContent}</code></pre>
            </section>
          ) : null}
          {file.afterContent !== null ? (
            <section aria-label={`Proposed content of ${file.relativePath}`}>
              <h4>Proposed</h4>
              <pre tabIndex={0}><code>{file.afterContent}</code></pre>
            </section>
          ) : (
            <p>The file is deleted by this change.</p>
          )}
        </details>
      ))}
    </div>
  );
}

export function GovernedChangesPanel({
  canEnableEdits,
  enableEditsBusy,
  errorMessage,
  history,
  historyBusy,
  onDecide,
  onEnableEdits,
  onDecideRecovery,
  onPrepareRecovery,
  onRefreshHistory,
  onPropose,
  onStartNewProposal,
  onUndo,
  recoveryBusy,
  recoveryReturnFocusTarget,
  recoveryReview,
  state,
}: GovernedChangesPanelProps) {
  const errorBanner = errorMessage
    ? (
      <p className="changes-error" role="alert">
        <ShieldAlert aria-hidden="true" size={16} />
        {errorMessage}
      </p>
    )
    : null;

  if (state.kind === "unavailable") {
    return (
      <div className="inspector-empty-state">
        <ShieldCheck aria-hidden="true" size={24} />
        <h3>No proposed changes</h3>
        <p>{state.reason}</p>
        {canEnableEdits ? (
          <Button
            isDisabled={enableEditsBusy}
            onPress={onEnableEdits}
            size="large"
            variant="primary"
          >
            {enableEditsBusy ? "Enabling governed edits…" : "Allow governed edits"}
          </Button>
        ) : null}
        {errorBanner}
      </div>
    );
  }

  if (state.kind === "idle" || state.kind === "preparing") {
    return (
      <>
        {recoveryReview !== null ? (
          <RecoveryReview
            busy={recoveryBusy}
            onDecide={onDecideRecovery}
            returnFocusTarget={recoveryReturnFocusTarget}
            review={recoveryReview}
          />
        ) : (
          <ProposalComposer disabled={state.kind === "preparing"} onPropose={onPropose} />
        )}
        {errorBanner}
        <ChangeHistory
          key="change-history"
          history={history}
          historyBusy={historyBusy}
          onRefreshHistory={onRefreshHistory}
          onPrepareRecovery={onPrepareRecovery}
          onUndo={onUndo}
          recoveryBusy={recoveryBusy || recoveryReview !== null}
        />
      </>
    );
  }

  if (state.kind === "review") {
    const { review } = state.review;
    return (
      <>
        <div className="inspector-section-heading">
          <h2>{state.review.review.proposalKind === "undo" ? "Undo changes" : "Review changes"}</h2>
          <span>
            {review.files.length} {review.files.length === 1 ? "file" : "files"}
            {` · ${formatBytes(review.totalChangedBytes)} proposed`}
          </span>
        </div>
        <div className="context-review-notice" role="note">
          <strong>Nothing has changed yet</strong>
          <span>
            Applying consumes this exact reviewed proposal once, records a checkpoint, and
            writes the files atomically.
          </span>
        </div>
        <ReviewFiles files={review.files} />
        <div className="change-actions">
          <Button
            isDisabled={state.busy}
            onPress={() => onDecide("discard")}
            size="large"
            variant="secondary"
          >
            <Trash2 aria-hidden="true" size={17} />
            Discard
          </Button>
          <Button
            isDisabled={state.busy}
            onPress={() => onDecide("revise")}
            size="large"
            variant="secondary"
          >
            <PencilLine aria-hidden="true" size={17} />
            Revise
          </Button>
          <Button
            isDisabled={state.busy}
            onPress={() => onDecide("apply")}
            size="large"
            variant="primary"
          >
            <ShieldCheck aria-hidden="true" size={17} />
            Apply changes
          </Button>
        </div>
        {errorBanner}
        <p className="inspector-footnote">
          Approval binds the exact reviewed bytes. If the workspace changes first, applying
          fails closed with no partial write.
        </p>
      </>
    );
  }

  if (state.kind === "applied") {
    return (
      <>
        <div className="inspector-section-heading">
          <h2>Changes applied</h2>
          <span>
            {state.execution.files.length}{" "}
            {state.execution.files.length === 1 ? "file" : "files"} · checkpoint recorded
          </span>
        </div>
        <div className="proposal-files">
          {state.execution.files.map((file) => (
            <div className="proposal-file-row" key={file.relativePath}>
              <ShieldCheck aria-hidden="true" size={16} />
              <code>{file.relativePath}</code>
              <span>{file.exists ? file.operation : "deleted"}</span>
            </div>
          ))}
        </div>
        <div className="change-actions">
          <Button
            isDisabled={state.busy || !state.execution.undoable}
            onPress={() => onUndo(state.execution.executionId)}
            size="large"
            variant="secondary"
          >
            <Undo2 aria-hidden="true" size={17} />
            Undo changes
          </Button>
          <Button
            isDisabled={state.busy}
            onPress={onStartNewProposal}
            size="large"
            variant="primary"
          >
            <PencilLine aria-hidden="true" size={17} />
            Propose another change
          </Button>
        </div>
        {errorBanner}
        {recoveryReview !== null ? (
          <RecoveryReview
            busy={recoveryBusy}
            onDecide={onDecideRecovery}
            returnFocusTarget={recoveryReturnFocusTarget}
            review={recoveryReview}
          />
        ) : null}
        <ChangeHistory
          key="change-history"
          history={history}
          historyBusy={historyBusy}
          onRefreshHistory={onRefreshHistory}
          onPrepareRecovery={onPrepareRecovery}
          onUndo={onUndo}
          recoveryBusy={recoveryBusy || recoveryReview !== null}
        />
      </>
    );
  }

  if (state.kind === "undo_unavailable") {
    return (
      <div className="inspector-empty-state">
        <ShieldAlert aria-hidden="true" size={24} />
        <h3>Undo changes is unavailable</h3>
        <p>{state.value.reason}</p>
        {state.value.conflicts.length > 0 ? (
          <ul aria-label="Undo conflicts" className="changes-conflicts">
            {state.value.conflicts.map((conflict) => (
              <li key={conflict.relativePath}><code>{conflict.relativePath}</code></li>
            ))}
          </ul>
        ) : null}
        <Button onPress={onStartNewProposal} size="large" variant="primary">
          <PencilLine aria-hidden="true" size={17} />
          Propose another change
        </Button>
        {errorBanner}
      </div>
    );
  }

  return (
    <div className="inspector-empty-state">
      <Trash2 aria-hidden="true" size={24} />
      <h3>No proposed changes</h3>
      <p>The previous proposal was discarded without changing your local workspace.</p>
      <Button onPress={onStartNewProposal} size="large" variant="primary">
        <PencilLine aria-hidden="true" size={17} />
        Propose another change
      </Button>
      {errorBanner}
    </div>
  );
}
