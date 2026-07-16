import { Button } from "@sapphirus/ui";
import {
  FilePlus2,
  FileX2,
  PencilLine,
  ShieldAlert,
  ShieldCheck,
  Trash2,
  Undo2,
} from "lucide-react";
import { useId, useState } from "react";
import type {
  ApprovalChoice,
  ChangesExecutionProjection,
  ChangesReviewEnvelopeProjection,
  ChangesReviewFileProjection,
  ChangesUndoUnavailableProjection,
} from "../lib/hostClient";

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
  onDecide: (choice: ApprovalChoice) => void;
  onEnableEdits: () => void;
  onPropose: (relativePath: string, content: string) => void;
  onStartNewProposal: () => void;
  onUndo: (executionId: string) => void;
  state: GovernedChangesUiState;
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
  onPropose: (relativePath: string, content: string) => void;
}) {
  const pathFieldId = useId();
  const contentFieldId = useId();
  const [relativePath, setRelativePath] = useState("");
  const [content, setContent] = useState("");
  const submitDisabled = disabled || relativePath.trim().length === 0;

  return (
    <form
      className="changes-composer"
      onSubmit={(event) => {
        event.preventDefault();
        if (!submitDisabled) {
          onPropose(relativePath.trim(), content);
        }
      }}
    >
      <div className="inspector-section-heading">
        <h2>Proposed changes</h2>
        <span>Governed local edit</span>
      </div>
      <p className="changes-composer__hint">
        Enter a workspace-relative file path and its complete proposed content. The host
        observes the current file, and nothing changes until you review and apply.
      </p>
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
      <label htmlFor={contentFieldId}>Proposed content</label>
      <textarea
        disabled={disabled}
        id={contentFieldId}
        onChange={(event) => setContent(event.target.value)}
        rows={10}
        spellCheck={false}
        value={content}
      />
      <Button isDisabled={submitDisabled} size="large" type="submit" variant="primary">
        <PencilLine aria-hidden="true" size={17} />
        Review changes
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
  onDecide,
  onEnableEdits,
  onPropose,
  onStartNewProposal,
  onUndo,
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
        <ProposalComposer disabled={state.kind === "preparing"} onPropose={onPropose} />
        {errorBanner}
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
