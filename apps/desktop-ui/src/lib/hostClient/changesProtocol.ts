import {
  type ApprovalChoice,
  CHANGES_REVIEW_SCHEMA,
  type ChangesDecisionProjection,
  type ChangesDisposition,
  type ChangesExecutionFileProjection,
  type ChangesExecutionProjection,
  type ChangesHistoryEntryProjection,
  type ChangesHistoryProjection,
  type ChangesOpenJournalProjection,
  type ChangesProposalKind,
  type ChangesReviewEnvelopeProjection,
  type ChangesReviewFileProjection,
  type ChangesReviewOperation,
  type ChangesReviewProjection,
  type ChangesUndoConflictProjection,
  type ChangesUndoUnavailableProjection,
  localEditsLimits,
  type RollbackRequestResult,
  type WorkspaceProjection,
} from "./contracts";
import {
  asBmadIdentifier,
  asBoolean,
  asContractId,
  asRecord,
  asRelativePath,
  asRendererSafeMessage,
  assertExactKeys,
  assertUniqueIdentities,
  assertUniqueRelativePaths,
  asSha256,
  asSingleLineText,
  asTextContent,
  asUnsignedInteger,
  fail,
  utf8Length,
} from "./validation";
import { parseDispatchReply, parseWorkspace } from "./workspaceProtocol";

export function asChangesReviewOperation(
  value: unknown,
): ChangesReviewOperation {
  if (value !== "create" && value !== "modify" && value !== "delete") {
    return fail();
  }
  return value;
}

export function asChangesProposalKind(value: unknown): ChangesProposalKind {
  if (value !== "edit" && value !== "undo") {
    return fail();
  }
  return value;
}

export function asNullableSha256(value: unknown): string | null {
  return value === null ? null : asSha256(value);
}

export function parseChangesReviewFile(
  value: unknown,
): ChangesReviewFileProjection {
  const file = asRecord(value);
  assertExactKeys(file, [
    "relativePath",
    "operation",
    "beforeContent",
    "afterContent",
    "beforeHash",
    "afterHash",
    "beforeBytes",
    "afterBytes",
  ]);
  const operation = asChangesReviewOperation(file.operation);
  const beforeContent =
    file.beforeContent === null
      ? null
      : asTextContent(file.beforeContent, localEditsLimits.changeContentBytes);
  const afterContent =
    file.afterContent === null
      ? null
      : asTextContent(file.afterContent, localEditsLimits.changeContentBytes);
  const beforeHash = asNullableSha256(file.beforeHash);
  const afterHash = asNullableSha256(file.afterHash);
  const beforeBytes = asUnsignedInteger(file.beforeBytes);
  const afterBytes = asUnsignedInteger(file.afterBytes);
  if (
    (operation === "create") !== (beforeContent === null) ||
    (operation === "delete") !== (afterContent === null) ||
    (beforeContent === null) !== (beforeHash === null) ||
    (afterContent === null) !== (afterHash === null) ||
    (beforeContent === null
      ? beforeBytes !== 0
      : beforeBytes !== utf8Length(beforeContent)) ||
    (afterContent === null
      ? afterBytes !== 0
      : afterBytes !== utf8Length(afterContent))
  ) {
    return fail();
  }
  return {
    relativePath: asRelativePath(file.relativePath),
    operation,
    beforeContent,
    afterContent,
    beforeHash,
    afterHash,
    beforeBytes,
    afterBytes,
  };
}

export interface ExpectedChangesReview {
  workspaceId: string;
  workspaceGrantEpoch: number | null;
  proposalKind: ChangesProposalKind;
  sourceExecutionId: string | null;
}

export function parseChangesReview(
  value: unknown,
  expected: ExpectedChangesReview,
): ChangesReviewProjection {
  const review = asRecord(value);
  assertExactKeys(review, [
    "schemaVersion",
    "proposalId",
    "candidateId",
    "candidateHash",
    "workspaceId",
    "workspaceGrantEpoch",
    "proposalKind",
    "sourceExecutionId",
    "files",
    "totalChangedBytes",
    "createdAt",
    "expiresAt",
  ]);
  if (
    review.schemaVersion !== CHANGES_REVIEW_SCHEMA ||
    review.workspaceId !== expected.workspaceId ||
    review.sourceExecutionId !== expected.sourceExecutionId ||
    !Array.isArray(review.files) ||
    review.files.length === 0 ||
    review.files.length > localEditsLimits.reviewFiles
  ) {
    return fail();
  }
  const proposalKind = asChangesProposalKind(review.proposalKind);
  if (proposalKind !== expected.proposalKind) {
    return fail();
  }
  const workspaceGrantEpoch = asUnsignedInteger(review.workspaceGrantEpoch);
  if (
    workspaceGrantEpoch < 1 ||
    (expected.workspaceGrantEpoch !== null &&
      workspaceGrantEpoch !== expected.workspaceGrantEpoch)
  ) {
    return fail();
  }
  const files = review.files.map(parseChangesReviewFile);
  assertUniqueRelativePaths(files.map(({ relativePath }) => relativePath));
  const createdAt = asUnsignedInteger(review.createdAt);
  const expiresAt = asUnsignedInteger(review.expiresAt);
  if (expiresAt <= createdAt) {
    return fail();
  }
  return {
    schemaVersion: CHANGES_REVIEW_SCHEMA,
    proposalId: asContractId(review.proposalId),
    candidateId: asContractId(review.candidateId),
    candidateHash: asSha256(review.candidateHash),
    workspaceId: asContractId(review.workspaceId),
    workspaceGrantEpoch,
    proposalKind,
    sourceExecutionId:
      expected.sourceExecutionId === null
        ? null
        : asContractId(review.sourceExecutionId),
    files,
    totalChangedBytes: asUnsignedInteger(review.totalChangedBytes),
    createdAt,
    expiresAt,
  };
}

export function parseChangesReviewEnvelope(
  value: unknown,
  expected: ExpectedChangesReview,
): ChangesReviewEnvelopeProjection {
  const envelope = asRecord(value);
  assertExactKeys(envelope, ["approvalId", "displayedDiffHash", "review"]);
  return {
    approvalId: asContractId(envelope.approvalId),
    displayedDiffHash: asSha256(envelope.displayedDiffHash),
    review: parseChangesReview(envelope.review, expected),
  };
}

export function parseChangesReviewReply(
  value: unknown,
  requestId: string,
  expected: ExpectedChangesReview,
): { projection: ChangesReviewEnvelopeProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "changes_review") {
    return fail();
  }
  return {
    projection: parseChangesReviewEnvelope(data.value, expected),
    sequence: parsed.sequence,
  };
}

export function parseWorkspaceEditsEnabledReply(
  value: unknown,
  requestId: string,
  expected: WorkspaceProjection,
): { projection: WorkspaceProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (
    data.kind !== "workspace_edits_enabled" ||
    expected.grantEpoch >= Number.MAX_SAFE_INTEGER
  ) {
    return fail();
  }
  const enabled = parseWorkspace(data.value);
  if (
    enabled.workspaceId !== expected.workspaceId ||
    enabled.projectId !== expected.projectId ||
    enabled.displayName !== expected.displayName ||
    enabled.grantEpoch !== expected.grantEpoch + 1 ||
    enabled.permissions !== "governed_edits"
  ) {
    return fail();
  }
  return { projection: enabled, sequence: parsed.sequence };
}

export function parseChangesExecution(
  value: unknown,
): ChangesExecutionProjection {
  const execution = asRecord(value);
  assertExactKeys(execution, [
    "executionId",
    "checkpointId",
    "completedAt",
    "undoable",
    "files",
  ]);
  if (
    !Array.isArray(execution.files) ||
    execution.files.length === 0 ||
    execution.files.length > localEditsLimits.reviewFiles
  ) {
    return fail();
  }
  const files = execution.files.map((value): ChangesExecutionFileProjection => {
    const file = asRecord(value);
    assertExactKeys(file, [
      "relativePath",
      "operation",
      "exists",
      "contentHash",
    ]);
    const exists = asBoolean(file.exists);
    const contentHash = asNullableSha256(file.contentHash);
    if (exists !== (contentHash !== null)) {
      return fail();
    }
    return {
      relativePath: asRelativePath(file.relativePath),
      operation: asChangesReviewOperation(file.operation),
      exists,
      contentHash,
    };
  });
  assertUniqueRelativePaths(files.map(({ relativePath }) => relativePath));
  return {
    executionId: asContractId(execution.executionId),
    checkpointId: asContractId(execution.checkpointId),
    completedAt: asUnsignedInteger(execution.completedAt),
    undoable: asBoolean(execution.undoable),
    files,
  };
}

export const changesDispositionByChoice: Record<
  ApprovalChoice,
  ChangesDisposition
> = {
  apply: "applied",
  revise: "revise_requested",
  discard: "discarded",
};

export function parseChangesDecisionReply(
  value: unknown,
  requestId: string,
  expected: { approvalId: string; choice: ApprovalChoice },
): { projection: ChangesDecisionProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "changes_decision") {
    return fail();
  }
  const decision = asRecord(data.value);
  assertExactKeys(decision, ["approvalId", "disposition", "execution"]);
  const disposition = changesDispositionByChoice[expected.choice];
  if (
    decision.approvalId !== expected.approvalId ||
    decision.disposition !== disposition ||
    (disposition === "applied") !== (decision.execution !== null)
  ) {
    return fail();
  }
  return {
    projection: {
      approvalId: asContractId(decision.approvalId),
      disposition,
      execution:
        decision.execution === null
          ? null
          : parseChangesExecution(decision.execution),
    },
    sequence: parsed.sequence,
  };
}

export function parseChangesUndoUnavailable(
  value: unknown,
  expectedExecutionId: string,
): ChangesUndoUnavailableProjection {
  const unavailable = asRecord(value);
  assertExactKeys(unavailable, ["executionId", "reason", "conflicts"]);
  if (
    unavailable.executionId !== expectedExecutionId ||
    !Array.isArray(unavailable.conflicts) ||
    unavailable.conflicts.length > localEditsLimits.undoConflicts
  ) {
    return fail();
  }
  const conflicts = unavailable.conflicts.map(
    (value): ChangesUndoConflictProjection => {
      const conflict = asRecord(value);
      assertExactKeys(conflict, [
        "relativePath",
        "expectedExists",
        "currentExists",
      ]);
      return {
        relativePath: asRelativePath(conflict.relativePath),
        expectedExists: asBoolean(conflict.expectedExists),
        currentExists: asBoolean(conflict.currentExists),
      };
    },
  );
  assertUniqueRelativePaths(conflicts.map(({ relativePath }) => relativePath));
  return {
    executionId: asContractId(unavailable.executionId),
    reason: asRendererSafeMessage(unavailable.reason),
    conflicts,
  };
}

export function parseRollbackRequestReply(
  value: unknown,
  requestId: string,
  executionId: string,
): { result: RollbackRequestResult; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind === "changes_undo_unavailable") {
    return {
      result: {
        kind: "unavailable",
        value: parseChangesUndoUnavailable(data.value, executionId),
      },
      sequence: parsed.sequence,
    };
  }
  if (data.kind !== "changes_review") {
    return fail();
  }
  const envelope = asRecord(data.value);
  assertExactKeys(envelope, ["approvalId", "displayedDiffHash", "review"]);
  const review = asRecord(envelope.review);
  const workspaceId = asContractId(review.workspaceId);
  return {
    result: {
      kind: "review",
      value: parseChangesReviewEnvelope(data.value, {
        workspaceId,
        workspaceGrantEpoch: null,
        proposalKind: "undo",
        sourceExecutionId: executionId,
      }),
    },
    sequence: parsed.sequence,
  };
}

export function parseChangesHistoryReply(
  value: unknown,
  requestId: string,
  expectedWorkspaceId: string,
): { projection: ChangesHistoryProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "changes_history") {
    return fail();
  }
  const history = asRecord(data.value);
  assertExactKeys(history, ["workspaceId", "entries", "openJournals"]);
  if (
    history.workspaceId !== expectedWorkspaceId ||
    !Array.isArray(history.entries) ||
    history.entries.length > localEditsLimits.historyEntries ||
    !Array.isArray(history.openJournals) ||
    history.openJournals.length > localEditsLimits.openJournals
  ) {
    return fail();
  }
  const entries = history.entries.map(
    (value): ChangesHistoryEntryProjection => {
      const entry = asRecord(value);
      assertExactKeys(entry, [
        "executionId",
        "journalState",
        "fileCount",
        "completedAt",
        "undoable",
      ]);
      return {
        executionId: asContractId(entry.executionId),
        journalState: asBmadIdentifier(entry.journalState),
        fileCount: asUnsignedInteger(entry.fileCount),
        completedAt: asSingleLineText(entry.completedAt, 64),
        undoable: asBoolean(entry.undoable),
      };
    },
  );
  assertUniqueIdentities(entries.map(({ executionId }) => executionId));
  const openJournals = history.openJournals.map(
    (value): ChangesOpenJournalProjection => {
      const journal = asRecord(value);
      assertExactKeys(journal, [
        "journalId",
        "executionId",
        "state",
        "updatedAt",
      ]);
      return {
        journalId: asContractId(journal.journalId),
        executionId: asContractId(journal.executionId),
        state: asBmadIdentifier(journal.state),
        updatedAt: asSingleLineText(journal.updatedAt, 64),
      };
    },
  );
  assertUniqueIdentities(openJournals.map(({ journalId }) => journalId));
  return {
    projection: {
      workspaceId: asContractId(history.workspaceId),
      entries,
      openJournals,
    },
    sequence: parsed.sequence,
  };
}
