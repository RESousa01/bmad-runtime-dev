import { Button } from "@sapphirus/ui";
import { RotateCcw, ShieldAlert, X } from "lucide-react";
import { useEffect, useId, useRef } from "react";
import type {
  ChangesRecoveryPrepared,
  RecoveryApprovalChoice,
} from "../lib/hostClient";

export interface RecoveryReviewProps {
  readonly busy: boolean;
  readonly onDecide: (choice: RecoveryApprovalChoice) => void;
  readonly returnFocusTarget: HTMLElement | null;
  readonly review: Extract<ChangesRecoveryPrepared, { status: "review_required" }>;
}

function operationLabel(operation: "create" | "replace" | "delete"): string {
  switch (operation) {
    case "create": return "Recreate";
    case "replace": return "Restore content";
    case "delete": return "Remove partial file";
  }
}

export function RecoveryReview({
  busy,
  onDecide,
  returnFocusTarget,
  review,
}: RecoveryReviewProps) {
  const headingId = useId();
  const headingRef = useRef<HTMLHeadingElement>(null);
  const decisionStartedRef = useRef(false);

  useEffect(() => {
    headingRef.current?.focus();
    return () => {
      if (returnFocusTarget?.isConnected && !returnFocusTarget.closest("[inert]")) {
        returnFocusTarget.focus();
      }
    };
  }, [returnFocusTarget, review.recoveryApprovalId]);

  const decide = (choice: RecoveryApprovalChoice) => {
    if (busy || decisionStartedRef.current) return;
    decisionStartedRef.current = true;
    onDecide(choice);
  };

  return (
    <section aria-labelledby={headingId} className="recovery-review">
      <div className="inspector-section-heading">
        <div>
          <h2 id={headingId} ref={headingRef} tabIndex={-1}>Review checkpoint recovery</h2>
          <p>
            Restoration returns the listed paths to the durable checkpoint recorded before
            the interrupted change.
          </p>
        </div>
        <ShieldAlert aria-hidden="true" size={18} />
      </div>

      <div aria-label="Checkpoint recovery operations" className="proposal-files" role="list">
        {review.operations.map((operation) => (
          <div className="proposal-file-row" key={operation.relativePath} role="listitem">
            <RotateCcw aria-hidden="true" size={16} />
            <code>{operation.relativePath}</code>
            <span>{operationLabel(operation.operation)}</span>
            <small>{operation.explanation}</small>
          </div>
        ))}
      </div>

      <p className="inspector-footnote">
        Confirmation is bound to this exact review. The private confirmation value stays in
        the signed desktop host and is not displayed.
      </p>

      <div className="change-actions">
        <Button
          isDisabled={busy || decisionStartedRef.current}
          onPress={() => decide("cancel")}
          size="large"
          variant="secondary"
        >
          <X aria-hidden="true" size={17} />
          Cancel
        </Button>
        <Button
          isDisabled={busy || decisionStartedRef.current}
          onPress={() => decide("restore")}
          size="large"
          variant="primary"
        >
          <RotateCcw aria-hidden="true" size={17} />
          Restore checkpoint
        </Button>
      </div>
    </section>
  );
}
