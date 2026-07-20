import type {
  CapabilityApprovedProjection,
  CapabilityCancelledProjection,
  CapabilityCompletedProjection,
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
  asUnsignedInteger,
  fail,
} from "./validation";
import { parseDispatchReply } from "./workspaceProtocol";

const CAPABILITY_ID_PATTERN = /^(?:bmm|builder):[a-z][a-z0-9._-]{2,80}$/u;
const RESULT_KIND_PATTERN = /^(?:document_artifact|governed_change_set|inactive_builder_draft)$/u;

function asCapabilityId(value: unknown): string {
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
  assertExactKeys(review, ["capabilityId", "runId", "manifestHash", "expiresAt"]);
  return {
    capabilityId: asCapabilityId(review.capabilityId),
    runId: asContractId(review.runId),
    manifestHash: asSha256(review.manifestHash),
    expiresAt: asUnsignedInteger(review.expiresAt),
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
  assertExactKeys(completed, ["capabilityId", "runId", "resultKind"]);
  return {
    capabilityId: asCapabilityId(completed.capabilityId),
    runId: asContractId(completed.runId),
    resultKind: asResultKind(completed.resultKind),
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
