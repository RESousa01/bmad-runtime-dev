import { Button } from "@sapphirus/ui";
import { RotateCcw, ShieldAlert, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { useFocusReturnOnUnmount } from "../app/focus/deferredFocusReturn";
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
  // Two representations of the same fact, deliberately.
  //
  // The ref is the synchronous double-submit guard: two presses landing in one
  // tick both read the pre-update value of state, so state alone cannot stop
  // the second from reaching `onDecide` and sending a second decision for one
  // checkpoint recovery.
  //
  // The state is what the buttons render from. They used to read the ref during
  // render, which does not work: writing a ref schedules no render, so the
  // buttons stayed enabled until something unrelated re-rendered this subtree.
  // Whether a submitted recovery looked submitted was down to timing.
  const decisionStartedRef = useRef(false);
  const [decisionStarted, setDecisionStarted] = useState(false);

  // Where focus goes when this closes is the shared helper's problem, not
  // this component's. The copy that used to live here focused the return
  // target from an effect cleanup, which runs mid-commit and so could be
  // wrong about what was in the document -- see the helper for the failure
  // that produced.
  useFocusReturnOnUnmount(returnFocusTarget);

  // A different review is a different thing to read, so the heading takes
  // focus again. Keyed on the approval id alone: `returnFocusTarget` is not
  // something the reader can see changing, and re-focusing the heading on it
  // would pull focus off whichever control inside the review they had reached.
  useEffect(() => {
    headingRef.current?.focus();
  }, [review.recoveryApprovalId]);

  const decide = (choice: RecoveryApprovalChoice) => {
    if (busy || decisionStartedRef.current) return;
    decisionStartedRef.current = true;
    setDecisionStarted(true);
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
        the native desktop host and is not displayed.
      </p>

      <div className="change-actions">
        <Button
          isDisabled={busy || decisionStarted}
          onPress={() => decide("cancel")}
          size="large"
          variant="secondary"
        >
          <X aria-hidden="true" size={17} />
          Cancel
        </Button>
        <Button
          isDisabled={busy || decisionStarted}
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
