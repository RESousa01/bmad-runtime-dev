import { describe, expect, it } from "vitest";
import {
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
