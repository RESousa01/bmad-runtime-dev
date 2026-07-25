import {
  type GitComparison,
  type GitFileComparisonProjection,
  type GitFileState,
  type GitStatusProjection,
  gitReviewLimits,
} from "./contracts";
import {
  asBoolean,
  asRecord,
  asRelativePath,
  asSingleLineText,
  asTextContent,
  assertExactKeys,
  assertUniqueRelativePaths,
  fail,
} from "./validation";
import { parseDispatchReply } from "./workspaceProtocol";

const fileStates = new Set<GitFileState>([
  "modified",
  "added",
  "deleted",
  "untracked",
  "renamed",
]);

const comparisons = new Set<GitComparison>([
  "head_to_worktree",
  "head_to_index",
  "index_to_worktree",
]);

function asNullableFileState(value: unknown): GitFileState | null {
  if (value === null) return null;
  if (typeof value !== "string" || !fileStates.has(value as GitFileState)) {
    return fail();
  }
  return value as GitFileState;
}

export function asGitComparison(value: unknown): GitComparison {
  if (typeof value !== "string" || !comparisons.has(value as GitComparison)) {
    return fail();
  }
  return value as GitComparison;
}

function asNullableContent(value: unknown): string | null {
  return value === null
    ? null
    : asTextContent(value, gitReviewLimits.diffBytesPerSide);
}

export function parseGitStatusReply(
  value: unknown,
  requestId: string,
): { projection: GitStatusProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const { data } = parsed;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "git_status") return fail();

  const projection = asRecord(data.value);
  assertExactKeys(projection, ["branch", "entries", "truncated"]);
  if (
    !Array.isArray(projection.entries) ||
    projection.entries.length > gitReviewLimits.statusEntries
  ) {
    return fail();
  }
  const branch =
    projection.branch === null
      ? null
      : asSingleLineText(
          projection.branch,
          gitReviewLimits.branchCharacters,
        );
  if (branch === "") return fail();

  const entries = projection.entries.map((value) => {
    const entry = asRecord(value);
    assertExactKeys(entry, [
      "relativePath",
      "indexState",
      "worktreeState",
    ]);
    const indexState = asNullableFileState(entry.indexState);
    const worktreeState = asNullableFileState(entry.worktreeState);
    if (indexState === null && worktreeState === null) return fail();
    return {
      relativePath: asRelativePath(entry.relativePath),
      indexState,
      worktreeState,
    };
  });
  assertUniqueRelativePaths(entries.map(({ relativePath }) => relativePath));

  return {
    projection: {
      branch,
      entries,
      truncated: asBoolean(projection.truncated),
    },
    sequence: parsed.sequence,
  };
}

export function parseGitDiffReply(
  value: unknown,
  requestId: string,
  expectedPath: string,
  expectedComparison: GitComparison,
): { projection: GitFileComparisonProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const { data } = parsed;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "git_diff") return fail();

  const projection = asRecord(data.value);
  assertExactKeys(projection, [
    "relativePath",
    "comparison",
    "beforeContent",
    "afterContent",
    "truncated",
  ]);
  const relativePath = asRelativePath(projection.relativePath);
  const comparison = asGitComparison(projection.comparison);
  if (relativePath !== expectedPath || comparison !== expectedComparison) {
    return fail();
  }

  return {
    projection: {
      relativePath,
      comparison,
      beforeContent: asNullableContent(projection.beforeContent),
      afterContent: asNullableContent(projection.afterContent),
      truncated: asBoolean(projection.truncated),
    },
    sequence: parsed.sequence,
  };
}
