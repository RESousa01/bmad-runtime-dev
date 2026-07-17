import { Button } from "@sapphirus/ui";
import { useEffect, useId, useRef } from "react";
import type { BmadHelpContextReviewProjection } from "../lib/bmadModelProjection";

export type ContextEgressReviewPhase =
  | "review_required"
  | "approving"
  | "approved"
  | "submitting";

export interface ContextEgressReviewProps {
  readonly onApprove: () => void;
  readonly onCancel: () => void;
  readonly onSend: () => void;
  readonly phase: ContextEgressReviewPhase;
  readonly review: BmadHelpContextReviewProjection;
}

function formatUtcTime(timestamp: number): string {
  return new Intl.DateTimeFormat("en-GB", {
    dateStyle: "medium",
    timeStyle: "short",
    timeZone: "UTC",
  }).format(new Date(timestamp));
}

function phaseLabel(phase: ContextEgressReviewPhase): string {
  switch (phase) {
    case "review_required": return "Review required";
    case "approving": return "Approving context…";
    case "approved": return "Context approved — nothing sent yet";
    case "submitting": return "Sending one request…";
  }
}

export function ContextEgressReview({
  onApprove,
  onCancel,
  onSend,
  phase,
  review,
}: ContextEgressReviewProps) {
  const headingId = useId();
  const itemsHeadingId = useId();
  const exclusionsHeadingId = useId();
  const findingsHeadingId = useId();
  const headingRef = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    headingRef.current?.focus();
  }, [review.manifestHash]);

  const interactionPending = phase === "approving" || phase === "submitting";

  return (
    <section aria-labelledby={headingId} className="context-egress-review">
      <div className="context-egress-review__heading">
        <div>
          <h3 id={headingId} ref={headingRef} tabIndex={-1}>Review request context</h3>
          <p>
            Inspect every byte below before granting one-shot permission. Approval does not send.
          </p>
        </div>
        <span className="context-egress-review__phase" role="status">{phaseLabel(phase)}</span>
      </div>

      {review.developmentOnly ? (
        <p className="development-only-label">Deterministic local model — development only</p>
      ) : null}

      <dl className="context-egress-review__facts">
        <div><dt>Purpose</dt><dd>{review.purpose}</dd></div>
        <div><dt>Destination</dt><dd>{review.destinationLabel}</dd></div>
        <div><dt>Region</dt><dd>{review.region}</dd></div>
        <div><dt>Retention</dt><dd>Transient — no store</dd></div>
        <div>
          <dt>Expires</dt>
          <dd><time dateTime={new Date(review.expiresAt).toISOString()}>{formatUtcTime(review.expiresAt)} UTC</time></dd>
        </div>
        <div><dt>Exact outbound size</dt><dd>{review.totalOutboundBytes} bytes</dd></div>
        <div><dt>Estimated tokens</dt><dd>{review.totalTokenEstimate}</dd></div>
      </dl>

      <p className="context-egress-review__disclosure">{review.consentDisclosure}</p>

      <section aria-labelledby={itemsHeadingId}>
        <h4 id={itemsHeadingId}>Exact outbound items</h4>
        <ol className="context-egress-review__items">
          {review.items.map((item, index) => (
            <li key={`${index}:${item.relativeLabel}`}>
              <div className="context-egress-review__item-heading">
                <strong>{item.relativeLabel}</strong>
                <span>{item.semanticRole}</span>
              </div>
              <dl>
                <div><dt>Classification</dt><dd>{item.classification}</dd></div>
                <div><dt>Bytes</dt><dd>{item.outboundByteCount}</dd></div>
                <div><dt>Estimated tokens</dt><dd>{item.tokenEstimate}</dd></div>
                <div><dt>Language</dt><dd>{item.language ?? "Not specified"}</dd></div>
              </dl>
              {item.redactions.length > 0 ? (
                <p>
                  Redactions: {item.redactions.map(({ kind, occurrenceCount }) => (
                    `${kind} (${occurrenceCount})`
                  )).join(", ")}
                </p>
              ) : <p>No redactions recorded.</p>}
              <pre><code>{item.outboundContent}</code></pre>
            </li>
          ))}
        </ol>
      </section>

      <div className="context-egress-review__secondary">
        <section aria-labelledby={exclusionsHeadingId}>
          <h4 id={exclusionsHeadingId}>Excluded context</h4>
          {review.exclusions.length > 0 ? (
            <ul>{review.exclusions.map((item) => (
              <li key={`${item.relativeLabel}:${item.reason}`}>
                <strong>{item.relativeLabel}</strong><span>{item.reason}</span>
              </li>
            ))}</ul>
          ) : <p>No candidate context was excluded.</p>}
        </section>
        <section aria-labelledby={findingsHeadingId}>
          <h4 id={findingsHeadingId}>Secret findings</h4>
          {review.secretFindings.length > 0 ? (
            <ul>{review.secretFindings.map((finding) => (
              <li key={`${finding.relativeLabel}:${finding.kind}`}>
                <strong>{finding.relativeLabel}</strong>
                <span>{finding.kind} ({finding.occurrenceCount})</span>
              </li>
            ))}</ul>
          ) : <p>No scanner findings recorded.</p>}
        </section>
      </div>

      <p className="context-egress-review__limitation">{review.redactionLimitation}</p>
      <div className="context-egress-review__actions">
        <Button
          isDisabled={phase !== "review_required"}
          onPress={onApprove}
          size="small"
          variant="secondary"
        >
          Approve context
        </Button>
        <Button
          isDisabled={interactionPending}
          onPress={onCancel}
          size="small"
          variant="quiet"
        >
          Cancel review
        </Button>
        <Button
          isDisabled={phase !== "approved"}
          onPress={onSend}
          size="small"
          variant="primary"
        >
          Send request
        </Button>
      </div>
    </section>
  );
}
