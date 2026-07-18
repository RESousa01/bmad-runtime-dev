import { describe, expect, it } from "vitest";
import {
  buildChangesRecoveryDecisionEnvelope,
  buildChangesRecoveryPrepareEnvelope,
} from "./commandEnvelopes";
import {
  parseChangesRecoveryDecisionReply,
  parseChangesRecoveryPreparedReply,
} from "./changesProtocol";
import { HostProtocolError, type HostBinding } from "./contracts";

const binding: HostBinding = {
  rendererSessionId: "renderer_01K0Q6H3",
  installationId: "install_01K0Q6H3",
  windowLabel: "main",
};
const digest = `sha256:${"a".repeat(64)}`;

function reply(requestId: string, kind: string, value: unknown) {
  return {
    schemaVersion: "desktop-dispatch-reply.v1",
    requestId,
    sequence: 13,
    status: "ok",
    receipt: { requestId, acceptedAt: 1_725_000_000_005, operationId: null },
    data: { kind, value },
  };
}

const reviewValue = {
  status: "review_required",
  recovery_approval_id: "recovery_approval_01K0Q6H3",
  displayed_recovery_hash: digest,
  journal_id: "journal_01K0Q6H3",
  execution_id: "execution_01K0Q6H3",
  operations: [{
    relativePath: "src/example.ts",
    operation: "replace",
    explanation: "Restore the file content saved before the interrupted change.",
  }],
  expires_at: 1_725_000_060_000,
};

describe("reviewed recovery protocol", () => {
  it("builds exact prepare and decision envelopes", () => {
    expect(buildChangesRecoveryPrepareEnvelope(
      binding,
      "request_prepare_01K0Q6H3",
      1_725_000_000_000,
      "workspace_01K0Q6H3",
      4,
      "journal_01K0Q6H3",
    )).toMatchObject({
      command: "changes.recovery.prepare",
      payload: {
        workspaceId: "workspace_01K0Q6H3",
        workspaceGrantEpoch: 4,
        journalId: "journal_01K0Q6H3",
      },
    });
    expect(buildChangesRecoveryDecisionEnvelope(
      binding,
      "request_decide_01K0Q6H3",
      1_725_000_000_001,
      "recovery_approval_01K0Q6H3",
      digest,
      "restore",
    )).toMatchObject({
      command: "changes.recovery.decide",
      payload: {
        recoveryApprovalId: "recovery_approval_01K0Q6H3",
        displayedRecoveryHash: digest,
        choice: "restore",
      },
    });
  });

  it("rejects invalid prepare and decision authority before dispatch", () => {
    expect(() => buildChangesRecoveryPrepareEnvelope(
      binding,
      "request_prepare_01K0Q6H3",
      1_725_000_000_000,
      "workspace_01K0Q6H3",
      0,
      "journal_01K0Q6H3",
    )).toThrow(HostProtocolError);
    expect(() => buildChangesRecoveryPrepareEnvelope(
      binding,
      "request_prepare_01K0Q6H3",
      Number.MAX_SAFE_INTEGER + 1,
      "workspace_01K0Q6H3",
      4,
      "bad id",
    )).toThrow(HostProtocolError);
    expect(() => buildChangesRecoveryDecisionEnvelope(
      binding,
      "request_decide_01K0Q6H3",
      1_725_000_000_001,
      "recovery_approval_01K0Q6H3",
      "sha256:nope",
      "restore",
    )).toThrow(HostProtocolError);
    expect(() => buildChangesRecoveryDecisionEnvelope(
      binding,
      "request_decide_01K0Q6H3",
      1_725_000_000_001,
      "recovery_approval_01K0Q6H3",
      digest,
      "apply" as "restore",
    )).toThrow(HostProtocolError);
  });

  it("parses the three strict preparation outcomes without leaking private fields", () => {
    expect(parseChangesRecoveryPreparedReply(
      reply("request_prepare_01K0Q6H3", "changes_recovery_prepared", reviewValue),
      "request_prepare_01K0Q6H3",
      "journal_01K0Q6H3",
    ).projection).toEqual({
      status: "review_required",
      recoveryApprovalId: "recovery_approval_01K0Q6H3",
      displayedRecoveryHash: digest,
      journalId: "journal_01K0Q6H3",
      executionId: "execution_01K0Q6H3",
      operations: reviewValue.operations,
      expiresAt: 1_725_000_060_000,
    });

    expect(parseChangesRecoveryPreparedReply(
      reply("request_already_01K0Q6H3", "changes_recovery_prepared", {
        status: "already_recovered",
        journal_id: "journal_01K0Q6H3",
        execution_id: "execution_01K0Q6H3",
      }),
      "request_already_01K0Q6H3",
      "journal_01K0Q6H3",
    ).projection.status).toBe("already_recovered");

    expect(parseChangesRecoveryPreparedReply(
      reply("request_manual_01K0Q6H3", "changes_recovery_prepared", {
        status: "manual_review",
        journal_id: "journal_01K0Q6H3",
        execution_id: "execution_01K0Q6H3",
        reason: "Recovery requires manual review on this device.",
      }),
      "request_manual_01K0Q6H3",
      "journal_01K0Q6H3",
    ).projection.status).toBe("manual_review");
  });

  it("rejects malformed, oversized, absolute-path, and extra-key preparation data", () => {
    const malformed = [
      { ...reviewValue, unknown: true },
      { ...reviewValue, status: "ready" },
      { ...reviewValue, recovery_approval_id: "bad id" },
      { ...reviewValue, displayed_recovery_hash: "sha256:nope" },
      { ...reviewValue, expires_at: Number.MAX_SAFE_INTEGER + 1 },
      { ...reviewValue, operations: [] },
      (({ operations: _operations, ...missingOperations }) => missingOperations)(reviewValue),
      { ...reviewValue, operations: Array.from({ length: 21 }, () => reviewValue.operations[0]) },
      { ...reviewValue, operations: [{ ...reviewValue.operations[0], relativePath: "C:\\private.txt" }] },
      { ...reviewValue, operations: [{ ...reviewValue.operations[0], privateHash: digest }] },
      { ...reviewValue, operations: [{ ...reviewValue.operations[0], explanation: "C:\\private\\checkpoint" }] },
      { ...reviewValue, operations: [{ ...reviewValue.operations[0], explanation: `Unexpected ${digest}` }] },
    ];
    for (const value of malformed) {
      expect(() => parseChangesRecoveryPreparedReply(
        reply("request_prepare_01K0Q6H3", "changes_recovery_prepared", value),
        "request_prepare_01K0Q6H3",
        "journal_01K0Q6H3",
      )).toThrow(HostProtocolError);
    }
  });

  it("parses strict decisions and rejects mismatches or unsafe values", () => {
    const value = {
      recoveryApprovalId: "recovery_approval_01K0Q6H3",
      disposition: "recovered",
      journalId: "journal_01K0Q6H3",
      executionId: "execution_01K0Q6H3",
      restoredFiles: 1,
    };
    expect(parseChangesRecoveryDecisionReply(
      reply("request_decide_01K0Q6H3", "changes_recovery_decision", value),
      "request_decide_01K0Q6H3",
      {
        recoveryApprovalId: "recovery_approval_01K0Q6H3",
        journalId: "journal_01K0Q6H3",
        executionId: "execution_01K0Q6H3",
        choice: "restore",
      },
    ).projection).toEqual(value);

    for (const malformed of [
      { ...value, disposition: "cancelled" },
      { ...value, recoveryApprovalId: "other_01K0Q6H3" },
      { ...value, journalId: "journal_other_01K0Q6H3" },
      { ...value, executionId: "execution_other_01K0Q6H3" },
      { ...value, restoredFiles: Number.MAX_SAFE_INTEGER + 1 },
      { ...value, nativeError: "C:\\private\\checkpoint" },
    ]) {
      expect(() => parseChangesRecoveryDecisionReply(
        reply("request_decide_01K0Q6H3", "changes_recovery_decision", malformed),
        "request_decide_01K0Q6H3",
        {
          recoveryApprovalId: "recovery_approval_01K0Q6H3",
          journalId: "journal_01K0Q6H3",
          executionId: "execution_01K0Q6H3",
          choice: "restore",
        },
      )).toThrow(HostProtocolError);
    }
  });
});
