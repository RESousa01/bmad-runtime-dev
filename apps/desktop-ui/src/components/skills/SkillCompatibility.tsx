/**
 * The compatibility markers every skills surface shares.
 *
 * FE-25. These are presentation only: each takes a verdict already decided by
 * `features/skills/skillAvailability` and renders it. Nothing here decides
 * whether a skill runs, and nothing here may — three panels each deciding for
 * themselves is how a catalogue comes to list a skill as startable in one place
 * and refuse it in another.
 */

import { Button } from "@sapphirus/ui";
import {
  type SkillAvailability,
  type SkillCoverageNote,
  type SkillVerdict,
  availabilityLabel,
  blockerExplanation,
  compatibilityDecisionNote,
  selectSkill,
} from "../../features/skills/skillAvailability";
import "./skills-panel.css";

/**
 * The per-skill verdict, stated where the skill is listed.
 *
 * Present on every row including available ones. Availability that is only
 * visible by the absence of a control is availability the user has to infer,
 * and inferring it wrongly is the whole failure this surface was rebuilt for.
 */
export function SkillVerdictChip({ availability }: { readonly availability: SkillAvailability }) {
  return (
    <span
      className="skill-compat skill-compat__verdict"
      data-verdict={availability.kind}
    >
      {availabilityLabel(availability)}
    </span>
  );
}

/** The reason, for the two verdicts that must carry one. */
export function SkillVerdictReason({ availability }: { readonly availability: SkillAvailability }) {
  if (availability.kind === "available") return null;
  return <p className="skill-compat skill-compat__reason">{availability.reason}</p>;
}

/**
 * What the recorded coverage matrix says, and what it does not.
 *
 * Collapsed, because it qualifies a skill rather than gating it — but never
 * omitted, because "runs here" alone reads as an upstream-compatibility claim
 * and no such claim has been reviewed for any of the 48 upstream skills.
 */
export function SkillCoverageDisclosure({ coverage }: { readonly coverage: SkillCoverageNote }) {
  return (
    <details className="skill-compat skill-compat__coverage">
      <summary>Upstream coverage</summary>
      <div>
        <p>{coverage.summary}</p>
        {coverage.kind === "recorded" ? (
          <>
            <p>{coverage.sourceNote}</p>
            <p className="skill-compat skill-compat__unreviewed">
              {compatibilityDecisionNote(coverage.decision)}
            </p>
            <p className="skill-compat skill-compat__state">
              <code>{coverage.runtimeState}</code> · <code>{coverage.snapshotState}</code>
            </p>
          </>
        ) : null}
      </div>
    </details>
  );
}

/**
 * The blocker codes the host reported, as prose with the identifier underneath.
 *
 * The explanation leads and the code follows. A raw code is evidence, not a
 * reason, and leading with it made every blocked entry look like an internal
 * error the reader had done something to cause.
 */
export function SkillBlockers({ codes }: { readonly codes: readonly string[] }) {
  if (codes.length === 0) return null;
  return (
    <div className="bmad-blockers">
      <span>Why this is unavailable</span>
      <ul aria-label="Blockers">
        {codes.map((code) => (
          <li key={code}>
            <p className="bmad-blockers__explanation">{blockerExplanation(code)}</p>
            <code>{code}</code>
          </li>
        ))}
      </ul>
    </div>
  );
}

/**
 * Everything a row says about whether its skill runs.
 *
 * One component so the three panels cannot drift into saying it three ways —
 * and so a row can never render the chip without the reason beside it.
 */
export function SkillCompatibility({
  blockerCodes = [],
  verdict,
}: {
  readonly blockerCodes?: readonly string[] | undefined;
  readonly verdict: SkillVerdict;
}) {
  return (
    <>
      <SkillVerdictReason availability={verdict.availability} />
      <SkillBlockers codes={blockerCodes} />
      <SkillCoverageDisclosure coverage={verdict.coverage} />
    </>
  );
}

/**
 * The start control, present only for a verdict that permits starting.
 *
 * `selectSkill` is the gate and the capability id lives only inside the answer
 * it returns, so there is no id in scope on the refused branch to start with.
 * That is deliberate: this repo prefers an unrepresentable illegal state to a
 * second runtime check a later edit can drift away from. The refusal itself is
 * already on screen — `SkillCompatibility` renders the reason for every verdict
 * that is not available.
 */
export function SkillStartControl({
  describedBy,
  label,
  onStart,
  verdict,
}: {
  readonly describedBy?: string | undefined;
  readonly label: string;
  readonly onStart?: ((capabilityId: string, label: string) => void) | undefined;
  readonly verdict: SkillVerdict;
}) {
  if (onStart === undefined) return null;
  const selection = selectSkill(verdict, label);
  if (selection.kind === "refused") return null;
  return (
    <Button
      // Spread rather than passed as possibly-undefined: with
      // `exactOptionalPropertyTypes` an explicit `undefined` is not the same as
      // an absent attribute, and react-aria's prop type refuses the former.
      {...(describedBy === undefined ? {} : { "aria-describedby": describedBy })}
      onPress={() => onStart(selection.capabilityId, selection.label)}
      size="small"
      variant="secondary"
    >
      Start reviewed run
    </Button>
  );
}
