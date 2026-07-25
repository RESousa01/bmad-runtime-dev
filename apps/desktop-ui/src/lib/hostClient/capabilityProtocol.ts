import type {
  ArchitectureProgressProjection,
  CapabilityApprovedProjection,
  CapabilityCancelledProjection,
  CapabilityCompletedProjection,
  CapabilityDocumentArtifact,
  CapabilityReviewProjection,
  CapabilityRunLatestProjection,
} from "./contracts";
import {
  asBoolean,
  asBoundedString,
  asContractId,
  assertExactKeys,
  asRecord,
  asSha256,
  asSingleLineText,
  asTextContent,
  asUnsignedInteger,
  fail,
} from "./validation";
import { parseDispatchReply } from "./workspaceProtocol";

const CAPABILITY_ID_PATTERN = /^(?:bmm|builder):[a-z][a-z0-9._-]{2,80}$/u;
const RESULT_KIND_PATTERN = /^(?:document_artifact|governed_change_set|inactive_builder_draft)$/u;
/// The host's closed, ordered architecture progression. The renderer pins the
/// same set so a fabricated or renamed step cannot reach the UI.
const ARCHITECTURE_STEP_CODES = [
  "system_context",
  "components",
  "trust_boundaries",
  "data",
  "failure_handling",
  "observability",
  "deployment",
  "verification",
] as const;
const ARCHITECTURE_STEP_COUNT = ARCHITECTURE_STEP_CODES.length;
const MAX_SPINE_SECTIONS = 64;
const MAX_SPINE_QUESTIONS = 64;
const MAX_SECTION_BODY = 32_768;
const MAX_MERMAID = 16_384;

/// Spine bodies are multi-line by construction — each carries the decision-log
/// entries minted for one step — so they are bounded as text content rather
/// than as single-line strings.
function asSpineSection(value: unknown): { heading: string; body: string } {
  const section = asRecord(value);
  assertExactKeys(section, ["heading", "body"]);
  const body = asTextContent(section.body, MAX_SECTION_BODY);
  if (body.length === 0) return fail();
  return {
    heading: asSingleLineText(section.heading, 200),
    body,
  };
}

/// Bounded decode of the distilled spine. It arrives inline on the final
/// step's completion rather than as a stored run result, so it is validated
/// here to the same shape the host's artifact constructor enforces.
function asSpine(value: unknown): CapabilityDocumentArtifact {
  const spine = asRecord(value);
  const keys = Object.keys(spine);
  for (const key of keys) {
    if (
      !["schemaVersion", "title", "sections", "evidenceRefs", "openQuestions", "mermaidText"].includes(
        key,
      )
    ) {
      return fail();
    }
  }
  if (!Array.isArray(spine.sections) || spine.sections.length > MAX_SPINE_SECTIONS) return fail();
  const openQuestions = spine.openQuestions ?? [];
  if (!Array.isArray(openQuestions) || openQuestions.length > MAX_SPINE_QUESTIONS) return fail();
  const mermaidText = spine.mermaidText;
  if (mermaidText !== undefined && mermaidText !== null && typeof mermaidText !== "string") {
    return fail();
  }
  return {
    title: asSingleLineText(spine.title, 200),
    sections: spine.sections.map(asSpineSection),
    openQuestions: openQuestions.map((question) => asSingleLineText(question, 1_024)),
    mermaidText:
      typeof mermaidText === "string" ? asTextContent(mermaidText, MAX_MERMAID) : null,
  };
}

/// A progression block is accepted only when its step is one the host owns,
/// its numbering is inside the closed progression, and a spine is present
/// exactly when the progression says it finalized.
export function asArchitectureProgress(value: unknown): ArchitectureProgressProjection {
  const progress = asRecord(value);
  const keys = Object.keys(progress);
  for (const key of keys) {
    if (
      !["stepCode", "stepTitle", "stepNumber", "stepCount", "finalized", "spine"].includes(key)
    ) {
      return fail();
    }
  }
  const stepCode = asBoundedString(progress.stepCode, 64);
  const index = ARCHITECTURE_STEP_CODES.indexOf(
    stepCode as (typeof ARCHITECTURE_STEP_CODES)[number],
  );
  if (index < 0) return fail();
  const stepNumber = asUnsignedInteger(progress.stepNumber);
  const stepCount = asUnsignedInteger(progress.stepCount);
  if (stepNumber !== index + 1 || stepCount !== ARCHITECTURE_STEP_COUNT) return fail();
  const finalized = asBoolean(progress.finalized);
  const spine = progress.spine ?? null;
  if (finalized !== (spine !== null)) return fail();
  return {
    stepCode,
    stepTitle: asBoundedString(progress.stepTitle, 200),
    stepNumber,
    stepCount,
    finalized,
    spine: spine === null ? null : asSpine(spine),
  };
}

function asOptionalArchitectureProgress(
  container: Record<string, unknown>,
): ArchitectureProgressProjection | null {
  return "architecture" in container ? asArchitectureProgress(container.architecture) : null;
}

export function asCapabilityId(value: unknown): string {
  const capabilityId = asBoundedString(value, 96);
  if (!CAPABILITY_ID_PATTERN.test(capabilityId)) return fail();
  return capabilityId;
}

function asResultKind(value: unknown): string {
  const resultKind = asBoundedString(value, 64);
  if (!RESULT_KIND_PATTERN.test(resultKind)) return fail();
  return resultKind;
}

export function parseCapabilityReview(value: unknown): CapabilityReviewProjection {
  const review = asRecord(value);
  const allowed = ["capabilityId", "runId", "manifestHash", "expiresAt", "architecture"];
  for (const key of Object.keys(review)) {
    if (!allowed.includes(key)) return fail();
  }
  for (const key of allowed.slice(0, 4)) {
    if (!(key in review)) return fail();
  }
  const architecture = asOptionalArchitectureProgress(review);
  // A review has not run yet, so it can never be finalized or carry a spine.
  if (architecture !== null && architecture.finalized) return fail();
  return {
    capabilityId: asCapabilityId(review.capabilityId),
    runId: asContractId(review.runId),
    manifestHash: asSha256(review.manifestHash),
    expiresAt: asUnsignedInteger(review.expiresAt),
    architecture,
  };
}

export function parseCapabilityReviewReply(
  value: unknown,
  requestId: string,
): { projection: CapabilityReviewProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "capability_review") return fail();
  return {
    projection: parseCapabilityReview(parsed.data.value),
    sequence: parsed.sequence,
  };
}

export function parseCapabilityApproved(value: unknown): CapabilityApprovedProjection {
  const approved = asRecord(value);
  assertExactKeys(approved, ["capabilityId", "manifestHash", "decisionId", "expiresAt"]);
  return {
    capabilityId: asCapabilityId(approved.capabilityId),
    manifestHash: asSha256(approved.manifestHash),
    decisionId: asContractId(approved.decisionId),
    expiresAt: asUnsignedInteger(approved.expiresAt),
  };
}

export function parseCapabilityApprovedReply(
  value: unknown,
  requestId: string,
): { projection: CapabilityApprovedProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "capability_approved") return fail();
  return {
    projection: parseCapabilityApproved(parsed.data.value),
    sequence: parsed.sequence,
  };
}

export function parseCapabilityCancelled(value: unknown): CapabilityCancelledProjection {
  const cancelled = asRecord(value);
  assertExactKeys(cancelled, ["capabilityId", "manifestHash", "cancelled"]);
  if (asBoolean(cancelled.cancelled) !== true) return fail();
  return {
    capabilityId: asCapabilityId(cancelled.capabilityId),
    manifestHash: asSha256(cancelled.manifestHash),
    cancelled: true,
  };
}

export function parseCapabilityCancelledReply(
  value: unknown,
  requestId: string,
): { projection: CapabilityCancelledProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "capability_cancelled") return fail();
  return {
    projection: parseCapabilityCancelled(parsed.data.value),
    sequence: parsed.sequence,
  };
}

export function parseCapabilityCompleted(value: unknown): CapabilityCompletedProjection {
  const completed = asRecord(value);
  const allowed = ["capabilityId", "runId", "resultKind", "architecture"];
  for (const key of Object.keys(completed)) {
    if (!allowed.includes(key)) return fail();
  }
  for (const key of allowed.slice(0, 3)) {
    if (!(key in completed)) return fail();
  }
  return {
    capabilityId: asCapabilityId(completed.capabilityId),
    runId: asContractId(completed.runId),
    resultKind: asResultKind(completed.resultKind),
    architecture: asOptionalArchitectureProgress(completed),
  };
}

export function parseCapabilityCompletedReply(
  value: unknown,
  requestId: string,
): { projection: CapabilityCompletedProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "capability_completed") return fail();
  return {
    projection: parseCapabilityCompleted(parsed.data.value),
    sequence: parsed.sequence,
  };
}

const MAX_RESULT_JSON_BYTES = 1_048_576;

export function parseCapabilityRunLatest(value: unknown): CapabilityRunLatestProjection {
  const latest = asRecord(value);
  const keys = Object.keys(latest);
  for (const key of keys) {
    if (!["capabilityId", "found", "runId", "resultKind", "resultJson"].includes(key)) {
      return fail();
    }
  }
  const found = asBoolean(latest.found);
  if (!found) {
    if ("runId" in latest || "resultKind" in latest || "resultJson" in latest) return fail();
    return {
      capabilityId: asCapabilityId(latest.capabilityId),
      found: false,
      runId: null,
      resultKind: null,
      resultJson: null,
    };
  }
  const resultJson =
    "resultJson" in latest ? asBoundedString(latest.resultJson, MAX_RESULT_JSON_BYTES) : null;
  return {
    capabilityId: asCapabilityId(latest.capabilityId),
    found: true,
    runId: asContractId(latest.runId),
    resultKind: "resultKind" in latest ? asResultKind(latest.resultKind) : null,
    resultJson,
  };
}

export function parseCapabilityRunLatestReply(
  value: unknown,
  requestId: string,
): { projection: CapabilityRunLatestProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "capability_run_latest") return fail();
  return {
    projection: parseCapabilityRunLatest(parsed.data.value),
    sequence: parsed.sequence,
  };
}

/**
 * `latestCapabilityRun` is keyed by capability (newest run wins), so when the
 * same capability runs concurrently on more than one task a *different* run can
 * be "latest". Trust the payload only when it belongs to the run just submitted;
 * otherwise the completion should carry no result content.
 */
export function latestResultForRun(
  latest: CapabilityRunLatestProjection,
  submittedRunId: string,
): string | null {
  return latest.found && latest.runId === submittedRunId ? latest.resultJson : null;
}
