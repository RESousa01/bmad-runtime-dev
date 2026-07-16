import { describe, expect, it } from "vitest";
import type { BmadHelpRunCreatedProjection } from "./bmadProjection";
import {
  bmadRequestAuthorityIsCurrent,
  initialBmadRequestState,
  transitionBmadRequest,
  type BmadAuthoritySnapshot,
  type BmadHelpApprovedProjection,
  type BmadHelpContextReviewProjection,
  type BmadRequestState,
} from "./bmadModelProjection";

const run: BmadHelpRunCreatedProjection = {
  schemaVersion: "bmad-help-run.v1",
  runKind: "bmad_help",
  lifecycle: "created_unbound",
  workspaceId: "workspace_test",
  runId: "run_test",
  sessionId: "session_test",
  currentIntent: "Review architecture readiness",
  runnable: false,
  completionClaimed: false,
  recommendation: {
    schemaVersion: "bmad-help-recommendation.v1",
    displayName: "BMad Help",
    moduleCode: "core",
    skillName: "bmad-help",
    action: null,
    confidence: "unknown",
    source: {
      sourceKind: "sealed_foundation",
      packageName: "bmad-method",
      packageVersion: "6.10.0",
    },
    reason: "The sealed Help capability matches this intent.",
    requiredGuidance: true,
    expectedArtifacts: [],
    availability: "capability_disabled",
    blockerCodes: ["bmad_capability_disabled"],
    completionClaimed: false,
  },
};

const review: BmadHelpContextReviewProjection = {
  workspaceId: run.workspaceId,
  workspaceGrantEpoch: 7,
  runId: run.runId,
  sessionId: run.sessionId,
  manifestHash: `sha256:${"a".repeat(64)}`,
  purpose: "bmad_help",
  destinationLabel: "Deterministic local model",
  region: "localdev",
  retentionMode: "transient_no_store",
  expiresAt: 2_000,
  items: [{
    relativeLabel: "method/current-intent.txt",
    semanticRole: "current_intent",
    language: "text",
    outboundByteCount: 29,
    tokenEstimate: 8,
    classification: "internal",
    redactions: [],
    outboundContent: "Review architecture readiness",
  }],
  exclusions: [],
  secretFindings: [],
  totalOutboundBytes: 29,
  totalTokenEstimate: 8,
  redactionLimitation: "Redaction reduces risk but cannot prove every secret was detected.",
  consentDisclosure: "Send only the exact context shown below for one Help request.",
  developmentOnly: true,
};

const authority: BmadAuthoritySnapshot = {
  workspaceId: run.workspaceId,
  workspaceGrantEpoch: review.workspaceGrantEpoch,
  runId: run.runId,
  authEpoch: 3,
  rendererGeneration: 4,
  now: 1_000,
};

const approval: BmadHelpApprovedProjection = {
  manifestHash: review.manifestHash,
  decisionId: "decision_test",
  expiresAt: 1_900,
  sendEligible: true,
};

function approvedState(): BmadRequestState {
  let state = transitionBmadRequest(initialBmadRequestState, { type: "create_started" });
  state = transitionBmadRequest(state, { type: "review_ready", run, review, authority });
  state = transitionBmadRequest(state, { type: "approve_started" });
  return transitionBmadRequest(state, { type: "approved", approval });
}

describe("BMAD request state machine", () => {
  it("represents create, review, approval, and one-shot submit without retaining send authority", () => {
    let state = approvedState();
    expect(state.kind).toBe("approved");

    state = transitionBmadRequest(state, { type: "submit_started" });
    expect(state.kind).toBe("submitting");
    expect(state).not.toHaveProperty("approval");

    const duplicate = transitionBmadRequest(state, { type: "submit_started" });
    expect(duplicate).toBe(state);
  });

  it("never resurrects a decision after cancellation, invalidation, or error", () => {
    const approved = approvedState();
    const cancelled = transitionBmadRequest(approved, {
      type: "terminal",
      reason: "cancelled",
    });
    expect(cancelled.kind).toBe("terminal");
    expect(transitionBmadRequest(cancelled, { type: "approved", approval })).toBe(cancelled);

    const invalidated = transitionBmadRequest(approved, {
      type: "authority_invalidated",
      reason: "authority_changed",
    });
    expect(invalidated.kind).toBe("terminal");
    expect(invalidated).not.toHaveProperty("approval");

    const failed = transitionBmadRequest(approved, {
      type: "unavailable",
      message: "The request could not be sent. Review again to retry.",
    });
    expect(failed.kind).toBe("unavailable");
    expect(failed).not.toHaveProperty("approval");
  });

  it.each([
    ["workspace", { workspaceId: "workspace_other" }],
    ["grant", { workspaceGrantEpoch: 8 }],
    ["run", { runId: "run_other" }],
    ["auth", { authEpoch: 4 }],
    ["renderer", { rendererGeneration: 5 }],
    ["expiry", { now: review.expiresAt }],
  ])("invalidates approval after %s drift", (_name, drift) => {
    expect(bmadRequestAuthorityIsCurrent(approvedState(), { ...authority, ...drift })).toBe(false);
  });

  it("accepts only the exact live authority snapshot", () => {
    expect(bmadRequestAuthorityIsCurrent(approvedState(), authority)).toBe(true);
    expect(bmadRequestAuthorityIsCurrent(initialBmadRequestState, authority)).toBe(false);
  });
});
