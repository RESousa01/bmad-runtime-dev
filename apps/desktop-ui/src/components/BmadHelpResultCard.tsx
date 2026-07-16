import { useId } from "react";
import type {
  BmadHelpEvidenceClass,
  BmadHelpNoRecommendationReason,
  BmadHelpRunCompletedProjection,
} from "../lib/bmadModelProjection";

export interface BmadHelpResultCardProps {
  readonly developmentOnly: boolean;
  readonly result: BmadHelpRunCompletedProjection;
}

function formatUtcTime(timestamp: number): string {
  return new Intl.DateTimeFormat("en-GB", {
    dateStyle: "medium",
    timeStyle: "short",
    timeZone: "UTC",
  }).format(new Date(timestamp));
}

function Timestamp({ value }: { readonly value: number }) {
  return <time dateTime={new Date(value).toISOString()}>{formatUtcTime(value)} UTC</time>;
}

function evidenceLabel(value: BmadHelpEvidenceClass): string {
  switch (value) {
    case "authoritative": return "Authoritative";
    case "user_asserted": return "User asserted";
    case "heuristic": return "Heuristic";
    case "contextual": return "Contextual";
  }
}

function reasonLabel(value: BmadHelpNoRecommendationReason): string {
  switch (value) {
    case "catalog_evidence_absent": return "No catalog evidence matched the reviewed intent.";
    case "completion_evidence_ambiguous": return "The reviewed completion evidence was ambiguous.";
    case "dependency_unavailable": return "A required Method dependency was unavailable.";
  }
}

export function BmadHelpResultCard({ developmentOnly, result }: BmadHelpResultCardProps) {
  const receiptHeadingId = useId();
  const recommendation = result.recommendation;

  return (
    <div className="bmad-help-result">
      {developmentOnly ? (
        <p className="development-only-label">Deterministic local model — development only</p>
      ) : null}
      {recommendation.recommendationKind === "recommended_capability" ? (
        <section className="bmad-help-result__recommendation">
          <h3>{recommendation.displayName}</h3>
          <p><code>{`${recommendation.moduleCode} / ${recommendation.skillName} / ${recommendation.action ?? "no action"}`}</code></p>
          <dl>
            <div><dt>Evidence</dt><dd>{evidenceLabel(recommendation.evidenceClass)}</dd></div>
            <div>
              <dt>Guidance</dt>
              <dd>{recommendation.guidanceRequired ? "Required" : "Optional"}</dd>
            </div>
            <div><dt>Created</dt><dd><Timestamp value={recommendation.createdAt} /></dd></div>
          </dl>
          <p>{recommendation.rationaleSummary}</p>
        </section>
      ) : (
        <section className="bmad-help-result__recommendation">
          <h3>No recommendation</h3>
          <p>{reasonLabel(recommendation.reasonCode)}</p>
          <p>Created <Timestamp value={recommendation.createdAt} /></p>
        </section>
      )}

      <section aria-labelledby={receiptHeadingId} className="bmad-help-result__receipt">
        <h3 id={receiptHeadingId}>Request receipt</h3>
        <dl>
          <div><dt>Receipt</dt><dd><code>{result.receipt.receiptId}</code></dd></div>
          <div><dt>Status</dt><dd>Succeeded</dd></div>
          <div><dt>Retention</dt><dd>Transient — no store</dd></div>
          <div><dt>Region</dt><dd>{result.receipt.region}</dd></div>
          <div><dt>Input</dt><dd>{result.receipt.inputBytes} bytes</dd></div>
          <div><dt>Output</dt><dd>{result.receipt.outputBytes} bytes</dd></div>
          <div><dt>Started</dt><dd><Timestamp value={result.receipt.startedAt} /></dd></div>
          <div><dt>Completed</dt><dd><Timestamp value={result.receipt.completedAt} /></dd></div>
        </dl>
      </section>
    </div>
  );
}
