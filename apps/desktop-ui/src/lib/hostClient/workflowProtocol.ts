import type {
  BmadWorkflowStateProjection,
  BmadWorkflowStepProjection,
} from "./contracts";
import {
  asBoolean,
  asBoundedString,
  assertExactKeys,
  asRecord,
  asRelativePath,
  fail,
} from "./validation";
import { parseDispatchReply } from "./workspaceProtocol";

/// The closed phase vocabulary from the sealed method graph. A phase outside
/// it is a host the renderer does not understand, not something to display.
const PHASE_CODES = [
  "anytime",
  "1-analysis",
  "2-planning",
  "3-solutioning",
  "4-implementation",
] as const;
const MAX_STEPS = 128;

function asPhaseCode(value: unknown): string {
  const code = asBoundedString(value, 32);
  if (!PHASE_CODES.includes(code as (typeof PHASE_CODES)[number])) return fail();
  return code;
}

function asStep(value: unknown): BmadWorkflowStepProjection {
  const step = asRecord(value);
  assertExactKeys(step, [
    "key",
    "displayName",
    "menuCode",
    "phaseCode",
    "required",
    "completed",
    "available",
    "evidencePath",
  ]);
  const completed = asBoolean(step.completed);
  const evidence = step.evidencePath;
  if (evidence !== null && typeof evidence !== "string") return fail();
  // Evidence is the *reason* a step reads complete. A completion with no
  // evidence, or evidence on an incomplete step, is an inconsistent claim.
  if (completed !== (evidence !== null)) return fail();
  return {
    key: asBoundedString(step.key, 128),
    displayName: asBoundedString(step.displayName, 128),
    menuCode: asBoundedString(step.menuCode, 16),
    phaseCode: asPhaseCode(step.phaseCode),
    required: asBoolean(step.required),
    completed,
    available: asBoolean(step.available),
    evidencePath: evidence === null ? null : asRelativePath(evidence),
  };
}

export function parseWorkflowState(value: unknown): BmadWorkflowStateProjection {
  const state = asRecord(value);
  assertExactKeys(state, [
    "schemaVersion",
    "methodInstalled",
    "phaseCode",
    "phaseTitle",
    "steps",
    "nextRequired",
    "unresolvedLocations",
  ]);
  if (state.schemaVersion !== "sapphirus.bmad-workflow-state.v1") return fail();
  if (!Array.isArray(state.steps) || state.steps.length > MAX_STEPS) return fail();
  if (!Array.isArray(state.unresolvedLocations)) return fail();
  const steps = state.steps.map(asStep);
  const nextRequired = state.nextRequired;
  if (nextRequired !== null && typeof nextRequired !== "string") return fail();
  // The next required gate must be a step in this projection, and must not
  // already be complete.
  if (nextRequired !== null) {
    const target = steps.find((step) => step.key === nextRequired);
    if (target === undefined || target.completed || !target.required) return fail();
  }
  return {
    schemaVersion: "sapphirus.bmad-workflow-state.v1",
    methodInstalled: asBoolean(state.methodInstalled),
    phaseCode: asPhaseCode(state.phaseCode),
    phaseTitle: asBoundedString(state.phaseTitle, 64),
    steps,
    nextRequired,
    unresolvedLocations: state.unresolvedLocations.map((entry) =>
      asBoundedString(entry, 128),
    ),
  };
}

export function parseWorkflowStateReply(
  value: unknown,
  requestId: string,
): { projection: BmadWorkflowStateProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "bmad_workflow_state") return fail();
  return { projection: parseWorkflowState(parsed.data.value), sequence: parsed.sequence };
}
