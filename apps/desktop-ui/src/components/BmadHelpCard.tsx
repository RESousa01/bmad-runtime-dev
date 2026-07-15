import { useId } from "react";
import type {
  BmadAvailability,
  BmadHelpConfidence,
  BmadHelpRecommendationProjection,
  BmadHelpRunCreatedProjection,
  BmadHelpUiState,
} from "../lib/bmadProjection";

export interface BmadHelpCardProps {
  readonly state: BmadHelpUiState;
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
                ? "Required by Method guidance"
                : "Optional Method guidance"}
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
        <span>No model request</span>
        <span>Execution unavailable</span>
      </div>
      <p>
        This local Method session is source-grounded but has no model or execution
        binding. It cannot change the workspace or claim completion.
      </p>
      <ReadyRecommendation recommendation={run.recommendation} />
    </div>
  );
}

function HelpBody({ state }: BmadHelpCardProps) {
  switch (state.kind) {
    case "no_evidence":
      return (
        <div className="bmad-help-card__empty">
          <strong>No recommendation yet</strong>
          <p>No active governed session is available to ground a next step.</p>
        </div>
      );
    case "loading":
      return (
        <p aria-live="polite" role="status">
          Finding a source-grounded recommendation…
        </p>
      );
    case "legacy_projection_unavailable":
    case "unavailable":
      return <p role="alert">{state.message}</p>;
    case "ready":
      return <ReadyHelpRun run={state.run} />;
  }
}

export function BmadHelpCard({ state }: BmadHelpCardProps) {
  const headingId = useId();
  return (
    <article
      aria-busy={state.kind === "loading" || undefined}
      aria-labelledby={headingId}
      className="bmad-help-card"
    >
      <h2 id={headingId}>Suggested next step</h2>
      <HelpBody state={state} />
    </article>
  );
}
