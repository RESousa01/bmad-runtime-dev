import { describe, expect, it } from "vitest";
import type { CapabilityRunLatestProjection } from "./contracts";
import {
  latestResultForRun,
  parseCapabilityApprovedReply,
  parseCapabilityCancelledReply,
  parseCapabilityCompletedReply,
  parseCapabilityReviewReply,
  parseCapabilityRunLatestReply,
} from "./capabilityProtocol";

const requestId = "req_01ARZ3NDEKTSV4RRFFQ69G5FAV";
const manifestHash = `sha256:${"a".repeat(64)}`;

function dispatchReply(data: unknown) {
  return {
    schemaVersion: "desktop-dispatch-reply.v1",
    requestId,
    sequence: 9,
    status: "ok",
    receipt: {
      requestId,
      acceptedAt: 1_725_000_000_005,
      operationId: null as string | null,
    },
    data,
  };
}

describe("capability protocol validators", () => {
  it("accepts each lifecycle reply exactly", () => {
    const review = parseCapabilityReviewReply(
      dispatchReply({
        kind: "capability_review",
        value: {
          capabilityId: "bmm:bmad-product-brief",
          runId: "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV",
          manifestHash,
          expiresAt: 1_725_000_600_000,
        },
      }),
      requestId,
    );
    expect(review.projection.capabilityId).toBe("bmm:bmad-product-brief");
    expect(review.sequence).toBe(9);

    const approved = parseCapabilityApprovedReply(
      dispatchReply({
        kind: "capability_approved",
        value: {
          capabilityId: "bmm:bmad-product-brief",
          manifestHash,
          decisionId: "decision_01ARZ3NDEKTSV4RRFFQ69G5FAV",
          expiresAt: 1_725_000_300_000,
        },
      }),
      requestId,
    );
    expect(approved.projection.decisionId).toContain("decision_");

    const cancelled = parseCapabilityCancelledReply(
      dispatchReply({
        kind: "capability_cancelled",
        value: {
          capabilityId: "bmm:bmad-product-brief",
          manifestHash,
          cancelled: true,
        },
      }),
      requestId,
    );
    expect(cancelled.projection.cancelled).toBe(true);

    const completed = parseCapabilityCompletedReply(
      dispatchReply({
        kind: "capability_completed",
        value: {
          capabilityId: "bmm:bmad-dev-story",
          runId: "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV",
          resultKind: "governed_change_set",
        },
      }),
      requestId,
    );
    expect(completed.projection.resultKind).toBe("governed_change_set");

    const latest = parseCapabilityRunLatestReply(
      dispatchReply({
        kind: "capability_run_latest",
        value: {
          capabilityId: "bmm:bmad-dev-story",
          found: true,
          runId: "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV",
          resultKind: "governed_change_set",
          resultJson: '{"resultKind":"governed_change_set"}',
        },
      }),
      requestId,
    );
    expect(latest.projection.found).toBe(true);

    const empty = parseCapabilityRunLatestReply(
      dispatchReply({
        kind: "capability_run_latest",
        value: { capabilityId: "bmm:bmad-dev-story", found: false },
      }),
      requestId,
    );
    expect(empty.projection.found).toBe(false);
    expect(empty.projection.runId).toBeNull();
  });

  it("rejects forged capability identifiers and result kinds", () => {
    for (const capabilityId of [
      "shell:rm",
      "bmad-product-brief",
      "bmm:UPPER",
      "bmm:../escape",
      "",
    ]) {
      expect(() =>
        parseCapabilityReviewReply(
          dispatchReply({
            kind: "capability_review",
            value: {
              capabilityId,
              runId: "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV",
              manifestHash,
              expiresAt: 1,
            },
          }),
          requestId,
        ),
      ).toThrow();
    }
    expect(() =>
      parseCapabilityCompletedReply(
        dispatchReply({
          kind: "capability_completed",
          value: {
            capabilityId: "bmm:bmad-dev-story",
            runId: "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV",
            resultKind: "arbitrary_effect",
          },
        }),
        requestId,
      ),
    ).toThrow();
  });

  it("rejects kind substitution, extra fields, and dishonest emptiness", () => {
    expect(() =>
      parseCapabilityReviewReply(
        dispatchReply({
          kind: "capability_approved",
          value: {
            capabilityId: "bmm:bmad-product-brief",
            runId: "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV",
            manifestHash,
            expiresAt: 1,
          },
        }),
        requestId,
      ),
    ).toThrow();
    expect(() =>
      parseCapabilityApprovedReply(
        dispatchReply({
          kind: "capability_approved",
          value: {
            capabilityId: "bmm:bmad-product-brief",
            manifestHash,
            decisionId: "decision_01ARZ3NDEKTSV4RRFFQ69G5FAV",
            expiresAt: 1,
            sendEligible: true,
          },
        }),
        requestId,
      ),
    ).toThrow();
    // A not-found latest reply cannot smuggle result fields.
    expect(() =>
      parseCapabilityRunLatestReply(
        dispatchReply({
          kind: "capability_run_latest",
          value: {
            capabilityId: "bmm:bmad-dev-story",
            found: false,
            resultJson: "{}",
          },
        }),
        requestId,
      ),
    ).toThrow();
  });
});

describe("latestResultForRun", () => {
  const runId = "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV";
  const otherRunId = "caprun_01ARZ3NDEKTSV4RRFFQ69G5FBW";
  const found = (id: string): CapabilityRunLatestProjection => ({
    capabilityId: "bmm:bmad-product-brief",
    found: true,
    runId: id,
    resultKind: "document_artifact",
    resultJson: '{"resultKind":"document_artifact"}',
  });

  it("returns the payload when the latest run is the submitted run", () => {
    expect(latestResultForRun(found(runId), runId)).toBe(
      '{"resultKind":"document_artifact"}',
    );
  });

  it("drops the payload when the latest run is a different concurrent run", () => {
    // The C2 guard: a concurrent same-capability run on another task is 'latest'.
    expect(latestResultForRun(found(otherRunId), runId)).toBeNull();
  });

  it("returns null when no run was found", () => {
    expect(
      latestResultForRun(
        {
          capabilityId: "bmm:bmad-product-brief",
          found: false,
          runId: null,
          resultKind: null,
          resultJson: null,
        },
        runId,
      ),
    ).toBeNull();
  });
});

describe("architecture progression protocol", () => {
  const capabilityId = "bmm:bmad-architecture";
  const runId = "caprun_01ARZ3NDEKTSV4RRFFQ69G5FAV";
  const spine = {
    schemaVersion: "sapphirus.bmad-document-artifact.v1",
    title: "Architecture spine",
    sections: [{ heading: "1. System context", body: "AD-1 — Scope\nBounded." }],
    evidenceRefs: [],
    openQuestions: ["Which tenant model?"],
  };

  function reviewReply(architecture: unknown) {
    return dispatchReply({
      kind: "capability_review",
      value: {
        capabilityId,
        runId,
        manifestHash,
        expiresAt: 1_725_000_600_000,
        architecture,
      },
    });
  }

  function completedReply(architecture: unknown) {
    return dispatchReply({
      kind: "capability_completed",
      value: { capabilityId, runId, resultKind: "document_artifact", architecture },
    });
  }

  const step = (overrides: Record<string, unknown> = {}) => ({
    stepCode: "trust_boundaries",
    stepTitle: "Trust boundaries",
    stepNumber: 3,
    stepCount: 8,
    finalized: false,
    spine: null,
    ...overrides,
  });

  it("accepts the host-selected step on a review", () => {
    const parsed = parseCapabilityReviewReply(reviewReply(step()), requestId);
    expect(parsed.projection.architecture?.stepCode).toBe("trust_boundaries");
    expect(parsed.projection.architecture?.stepNumber).toBe(3);
    expect(parsed.projection.architecture?.finalized).toBe(false);
  });

  it("accepts the distilled spine on the final completion", () => {
    const parsed = parseCapabilityCompletedReply(
      completedReply(
        step({ stepCode: "verification", stepTitle: "Verification", stepNumber: 8, finalized: true, spine }),
      ),
      requestId,
    );
    expect(parsed.projection.architecture?.finalized).toBe(true);
    expect(parsed.projection.architecture?.spine?.title).toBe("Architecture spine");
    expect(parsed.projection.architecture?.spine?.openQuestions).toEqual(["Which tenant model?"]);
  });

  it("keeps one-shot capabilities free of any progression", () => {
    const parsed = parseCapabilityCompletedReply(
      dispatchReply({
        kind: "capability_completed",
        value: {
          capabilityId: "bmm:bmad-product-brief",
          runId,
          resultKind: "document_artifact",
        },
      }),
      requestId,
    );
    expect(parsed.projection.architecture).toBeNull();
  });

  it("rejects a fabricated step, mismatched numbering, or a resized progression", () => {
    for (const forged of [
      step({ stepCode: "finalize" }),
      step({ stepNumber: 4 }),
      step({ stepCount: 3 }),
      step({ stepCode: "system_context", stepNumber: 3 }),
    ]) {
      expect(() => parseCapabilityReviewReply(reviewReply(forged), requestId)).toThrow();
    }
  });

  it("rejects a spine that does not match the finalized flag", () => {
    // A spine without finalization, or finalization without a spine, would let
    // a partial progression present itself as a completed architecture.
    expect(() =>
      parseCapabilityCompletedReply(completedReply(step({ spine })), requestId),
    ).toThrow();
    expect(() =>
      parseCapabilityCompletedReply(completedReply(step({ finalized: true })), requestId),
    ).toThrow();
  });

  it("rejects a review that claims the progression already finalized", () => {
    expect(() =>
      parseCapabilityReviewReply(reviewReply(step({ finalized: true, spine })), requestId),
    ).toThrow();
  });

  it("rejects unknown keys smuggled into the progression or the spine", () => {
    expect(() =>
      parseCapabilityReviewReply(reviewReply({ ...step(), approved: true }), requestId),
    ).toThrow();
    expect(() =>
      parseCapabilityCompletedReply(
        completedReply(
          step({
            stepCode: "verification",
            stepNumber: 8,
            finalized: true,
            spine: { ...spine, activate: true },
          }),
        ),
        requestId,
      ),
    ).toThrow();
  });
});
