import type {
  BmadWorkflowStateProjection,
  BmadWorkflowStepProjection,
} from "../lib/hostClient";

export interface BmadWorkflowPanelProps {
  readonly state: BmadWorkflowStateProjection | null;
  readonly loading: boolean;
  readonly error: string | null;
  readonly onRefresh: () => void;
  /// Starts a step. Absent when the step has no runnable capability bound.
  readonly onStartStep?: ((step: BmadWorkflowStepProjection) => void) | undefined;
}

const PHASE_ORDER = ["1-analysis", "2-planning", "3-solutioning", "4-implementation"] as const;

const PHASE_TITLES: Readonly<Record<string, string>> = {
  anytime: "Anytime",
  "1-analysis": "Analysis",
  "2-planning": "Planning",
  "3-solutioning": "Solutioning",
  "4-implementation": "Implementation",
};

/// Where this project stands in the BMAD method.
///
/// Every value here was computed by the host from the sealed workflow graph
/// plus governed workspace reads. No model was asked, so this panel states
/// facts rather than a recommendation — and says plainly when it cannot tell.
export function BmadWorkflowPanel({
  state,
  loading,
  error,
  onRefresh,
  onStartStep,
}: BmadWorkflowPanelProps) {
  if (error !== null) {
    return (
      <section aria-label="Method progress" className="workflow-panel">
        <p role="alert">{error}</p>
        <button onClick={onRefresh} type="button">
          Try again
        </button>
      </section>
    );
  }
  if (state === null) {
    return (
      <section aria-label="Method progress" className="workflow-panel">
        <p role="status">{loading ? "Reading method progress…" : "Method progress not loaded."}</p>
      </section>
    );
  }
  if (!state.methodInstalled) {
    return (
      <section aria-label="Method progress" className="workflow-panel">
        <h3>No BMAD install found</h3>
        <p>
          This workspace has no <code>_bmad</code> folder, so there is no method
          progress to report. Chat and skills still work; install BMAD in the
          project to track phases and gates.
        </p>
      </section>
    );
  }

  const phases = [...PHASE_ORDER, "anytime"];
  const nextRequired = state.steps.find((step) => step.key === state.nextRequired) ?? null;

  return (
    <section aria-label="Method progress" className="workflow-panel">
      <header>
        {/* Qualified so the current-phase heading is never confusable with the
            phase group of the same name further down. */}
        <h3>Current phase: {state.phaseTitle}</h3>
        {nextRequired === null ? (
          <p role="status">Every required gate is complete.</p>
        ) : (
          <p role="status">
            Next required: <strong>{nextRequired.displayName}</strong>{" "}
            <code>[{nextRequired.menuCode}]</code>
          </p>
        )}
      </header>

      {state.unresolvedLocations.length > 0 ? (
        <p className="workflow-panel__note" role="note">
          Some steps cannot be checked: the method config does not define{" "}
          {state.unresolvedLocations.join(", ")}. Those steps show as incomplete
          rather than guessed.
        </p>
      ) : null}

      {phases.map((phase) => {
        const steps = state.steps.filter((step) => step.phaseCode === phase);
        if (steps.length === 0) return null;
        return (
          <section aria-label={PHASE_TITLES[phase] ?? phase} key={phase}>
            <h4>{PHASE_TITLES[phase] ?? phase}</h4>
            <ul>
              {steps.map((step) => (
                <li data-completed={step.completed} key={step.key}>
                  <span className="workflow-panel__code">[{step.menuCode}]</span>{" "}
                  <span>{step.displayName}</span>
                  {step.required ? <span className="workflow-panel__gate"> required</span> : null}
                  {step.completed ? (
                    // The evidence is shown because it is *why* this reads
                    // complete; a claim with no artifact behind it would be a
                    // guess, and this panel does not guess.
                    <span className="workflow-panel__evidence">
                      {" "}
                      done — <code>{step.evidencePath}</code>
                    </span>
                  ) : onStartStep && step.available ? (
                    <button onClick={() => onStartStep(step)} type="button">
                      Start
                    </button>
                  ) : null}
                </li>
              ))}
            </ul>
          </section>
        );
      })}

      <button onClick={onRefresh} type="button">
        {loading ? "Refreshing…" : "Refresh"}
      </button>
    </section>
  );
}
