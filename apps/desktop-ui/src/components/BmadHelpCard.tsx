import { useId } from "react";
import type {
  BmadAvailability,
  BmadHelpConfidence,
  BmadHelpRecommendationProjection,
  BmadHelpRunCreatedProjection,
} from "../lib/bmadProjection";
import type { BmadRequestState } from "../lib/bmadModelProjection";
import { BmadHelpResultCard } from "./BmadHelpResultCard";
import { ContextEgressReview } from "./ContextEgressReview";

export interface BmadHelpCardProps {
  readonly developmentOnly?: boolean;
  readonly onApprove?: () => void;
  readonly onCancel?: () => void;
  readonly onSend?: () => void;
  readonly state: BmadRequestState;
}

function availabilityLabel(availability: BmadAvailability): string {
  switch (availability) {
    case "available":
      return "Available";
    case "capability_disabled":
      return "Capability disabled";
    case "dependency_unavailable":
      return "Dependency unavailable";
    case "orphan_skill":
      return "Catalog entry unavailable";
    case "network_unavailable":
      return "Network reference unavailable";
    case "source_prompt_unavailable":
      return "Source prompt unavailable";
  }
}

function confidenceLabel(confidence: BmadHelpConfidence): string {
  switch (confidence) {
    case "authoritative":
      return "Authoritative";
    case "user_asserted":
      return "User asserted";
    case "heuristic":
      return "Heuristic";
    case "contextual":
      return "Contextual";
    case "unknown":
      return "Unknown";
  }
}

function ReadyRecommendation({
  recommendation,
}: {
  readonly recommendation: BmadHelpRecommendationProjection;
}) {
  const identity = `${recommendation.moduleCode} / ${recommendation.skillName} / ${recommendation.action ?? "no action"}`;
  const expectedArtifactsHeadingId = useId();
  const blockersHeadingId = useId();

  return (
    <div className="bmad-help-card__recommendation">
      <h3>{recommendation.displayName}</h3>
      <dl className="bmad-help-card__facts">
        <div>
          <dt>Confidence</dt>
          <dd>{confidenceLabel(recommendation.confidence)}</dd>
        </div>
        <div>
          <dt>Availability</dt>
          <dd>{availabilityLabel(recommendation.availability)}</dd>
        </div>
        <div>
          <dt>Source</dt>
          <dd>
            <span>{recommendation.source.packageName} {recommendation.source.packageVersion}</span>
            <code>{identity}</code>
          </dd>
        </div>
        <div>
          <dt>Reason</dt>
          <dd>{recommendation.reason}</dd>
        </div>
        <div>
          <dt>Guidance</dt>
          <dd>
            <span>
              {recommendation.requiredGuidance
                ? "Required by BMAD skill guidance"
                : "Optional BMAD skill guidance"}
            </span>
            <span>This guidance does not grant platform permission.</span>
          </dd>
        </div>
      </dl>

      <section aria-labelledby={expectedArtifactsHeadingId}>
        <h3 id={expectedArtifactsHeadingId}>Expected artifacts</h3>
        {recommendation.expectedArtifacts.length > 0 ? (
          <ul>
            {recommendation.expectedArtifacts.map((artifact) => <li key={artifact}>{artifact}</li>)}
          </ul>
        ) : <p>No expected artifacts recorded.</p>}
      </section>

      <section aria-labelledby={blockersHeadingId}>
        <h3 id={blockersHeadingId}>Blockers</h3>
        {recommendation.blockerCodes.length > 0 ? (
          <ul>
            {recommendation.blockerCodes.map((code) => <li key={code}><code>{code}</code></li>)}
          </ul>
        ) : <p>No blockers reported.</p>}
      </section>
    </div>
  );
}

function ReadyHelpRun({ run }: { readonly run: BmadHelpRunCreatedProjection }) {
  return (
    <div className="bmad-help-card__run">
      <div className="bmad-help-card__run-status" role="status">
        <strong>Created</strong>
        <span>Unbound</span>
        <span className="sr-only">Created · Unbound</span>
        <span>No model request</span>
        <span>Execution unavailable</span>
      </div>
      <p>
        This local BMAD Help session is source-grounded but has no model or execution
        binding. It cannot change the workspace or claim completion.
      </p>
      <ReadyRecommendation recommendation={run.recommendation} />
    </div>
  );
}

function HelpBody({
  developmentOnly = false,
  onApprove = () => undefined,
  onCancel = () => undefined,
  onSend = () => undefined,
  state,
}: BmadHelpCardProps) {
  switch (state.kind) {
    case "idle":
      if (state.run) {
        return <ReadyHelpRun run={state.run} />;
      }
      return (
        <div className="bmad-help-card__empty">
          <strong>No recommendation yet</strong>
          <p>No active governed session is available to ground a next step.</p>
        </div>
      );
    case "creating":
      return (
        <p aria-live="polite" role="status">
          {state.activity === "recovering"
            ? "Checking for a retained BMAD Help recommendation…"
            : "Preparing a BMAD Help request review…"}
        </p>
      );
    case "review_required":
    case "approving":
    case "approved":
    case "submitting":
      return (
        <>
          {state.runProjection ? <ReadyHelpRun run={state.runProjection} /> : null}
          <ContextEgressReview
            onApprove={onApprove}
            onCancel={onCancel}
            onSend={onSend}
            phase={state.kind}
            review={state.review}
          />
        </>
      );
    case "completed":
      return (
        <BmadHelpResultCard
          developmentOnly={developmentOnly}
          result={state.result}
        />
      );
    case "interrupted":
      return (
        <div className="bmad-help-card__empty" role="alert">
          <strong>Request interrupted</strong>
          <p>This BMAD Help request cannot be resumed or sent again. Start a fresh review.</p>
        </div>
      );
    case "terminal":
      return (
        <div className="bmad-help-card__empty" role="status">
          <strong>{state.reason === "cancelled" ? "Review cancelled" : "Review ended"}</strong>
          <p>
            {state.reason === "consent_expired"
              ? "The context approval expired. Start a fresh review before sending."
              : state.reason === "authority_changed"
                ? "The active authority changed. Start a fresh review before sending."
                : state.reason === "failed"
                  ? "The request failed closed. Start a fresh review to try again."
                  : "No request was sent."}
          </p>
        </div>
      );
    case "unavailable":
      return <p role="alert">{state.message}</p>;
  }
}

export function BmadHelpCard({
  developmentOnly = false,
  onApprove = () => undefined,
  onCancel = () => undefined,
  onSend = () => undefined,
  state,
}: BmadHelpCardProps) {
  const headingId = useId();
  return (
    <article
      aria-busy={state.kind === "creating" || state.kind === "approving" || state.kind === "submitting" || undefined}
      aria-labelledby={headingId}
      className="bmad-help-card"
    >
      <h2 id={headingId}>Suggested next step</h2>
      <HelpBody
        developmentOnly={developmentOnly}
        onApprove={onApprove}
        onCancel={onCancel}
        onSend={onSend}
        state={state}
      />
    </article>
  );
}
