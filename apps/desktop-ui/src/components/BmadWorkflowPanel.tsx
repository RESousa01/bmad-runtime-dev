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
  // A failed read used to replace the whole panel. Since the host reports the
  // failure and the last good state independently, that threw away every phase,
  // gate and evidence path already on screen to say one sentence — so a refresh
  // that failed looked identical to a workspace with no method progress at all.
  const failureNotice = error === null ? null : (
    <div className="workflow-panel__failure">
      <p role="alert">{error}</p>
      <button disabled={loading} onClick={onRefresh} type="button">
        {loading ? "Reading…" : "Try again"}
      </button>
    </div>
  );

  if (error !== null && state === null) {
    return (
      <section aria-label="Method progress" className="workflow-panel">
        {failureNotice}
      </section>
    );
  }
  if (state === null) {
    // The load control has to live here too: `onRefresh` is the only caller of
    // the host's workflow-state command, and without a button in the unloaded
    // branch the panel was a dead end — method progress could never be read.
    return (
      <section aria-label="Method progress" className="workflow-panel">
        <p role="status">
          {loading
            ? "Reading method progress…"
            : "Method progress has not been read for this workspace yet."}
        </p>
        <button disabled={loading} onClick={onRefresh} type="button">
          {loading ? "Reading…" : "Read method progress"}
        </button>
      </section>
    );
  }
  if (!state.methodInstalled) {
    return (
      <section aria-label="Method progress" className="workflow-panel">
        {failureNotice}
        <h3>No BMAD install found</h3>
        <p>
          This workspace has no <code>_bmad</code> folder, so there is no method
          progress to report. Chat and skills still work; install BMAD in the
          project to track phases and gates.
        </p>
        {/* Without this the branch was terminal: installing BMAD afterwards
            left the panel asserting there was no install, with nothing on
            screen able to ask the host again. */}
        <button disabled={loading} onClick={onRefresh} type="button">
          {loading ? "Checking…" : "Check again"}
        </button>
      </section>
    );
  }

  const phases = [...PHASE_ORDER, "anytime"];
  const nextRequired = state.steps.find((step) => step.key === state.nextRequired) ?? null;

  return (
    <section aria-busy={loading || undefined} aria-label="Method progress" className="workflow-panel">
      {failureNotice}
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
                  ) : onStartStep === undefined ? null : step.available ? (
                    <button onClick={() => onStartStep(step)} type="button">
                      Start
                    </button>
                  ) : (
                    // A step with no control and no explanation reads as
                    // broken. `available` is false exactly when a step this one
                    // is preceded by is still incomplete, so say that.
                    <span className="workflow-panel__waiting">
                      {" "}
                      waiting on an earlier step
                    </span>
                  )}
                </li>
              ))}
            </ul>
          </section>
        );
      })}

      {/* Disabled while a read is in flight. It used to stay live and unlabelled
          as busy, so the only feedback for a slow read was a word swap on a
          control that still accepted clicks. */}
      <button disabled={loading} onClick={onRefresh} type="button">
        {loading ? "Refreshing…" : "Refresh"}
      </button>
    </section>
  );
}
