import { describe, expect, it } from "vitest";

import { DISPATCH_REPLY_SCHEMA, HostProtocolError } from "./contracts";
import { parseGitDiffReply, parseGitStatusReply } from "./gitProtocol";

function reply(requestId: string, data: unknown): unknown {
  return {
    schemaVersion: DISPATCH_REPLY_SCHEMA,
    requestId,
    sequence: 7,
    status: "ok",
    receipt: { requestId, acceptedAt: 10_000, operationId: null },
    data,
  };
}

describe("Forge Git Review protocol", () => {
  it("parses one deduplicated status projection with staged and unstaged state", () => {
    const requestId = "req_git_status_1";
    const parsed = parseGitStatusReply(
      reply(requestId, {
        kind: "git_status",
        value: {
          branch: "main",
          entries: [
            {
              relativePath: "src/App.tsx",
              indexState: "modified",
              worktreeState: "modified",
            },
            {
              relativePath: "README.md",
              indexState: null,
              worktreeState: "untracked",
            },
          ],
          truncated: false,
        },
      }),
      requestId,
    );

    expect(parsed.projection.entries).toEqual([
      {
        relativePath: "src/App.tsx",
        indexState: "modified",
        worktreeState: "modified",
      },
      {
        relativePath: "README.md",
        indexState: null,
        worktreeState: "untracked",
      },
    ]);
  });

  it("parses a bounded file comparison and binds it to the request", () => {
    const requestId = "req_git_diff_1";
    const parsed = parseGitDiffReply(
      reply(requestId, {
        kind: "git_diff",
        value: {
          relativePath: "src/App.tsx",
          comparison: "head_to_worktree",
          beforeContent: "before\n",
          afterContent: "after\n",
          truncated: false,
        },
      }),
      requestId,
      "src/App.tsx",
      "head_to_worktree",
    );

    expect(parsed.projection.beforeContent).toBe("before\n");
    expect(parsed.projection.afterContent).toBe("after\n");
  });

  it("fails closed on duplicate status paths, unknown fields, and baseline drift", () => {
    const requestId = "req_git_bad_1";
    const duplicate = reply(requestId, {
      kind: "git_status",
      value: {
        branch: null,
        entries: [
          { relativePath: "a.ts", indexState: "modified", worktreeState: null },
          { relativePath: "A.ts", indexState: null, worktreeState: "modified" },
        ],
        truncated: false,
      },
    });
    expect(() => parseGitStatusReply(duplicate, requestId)).toThrow(
      HostProtocolError,
    );

    const extra = reply(requestId, {
      kind: "git_diff",
      value: {
        relativePath: "a.ts",
        comparison: "head_to_index",
        beforeContent: "a",
        afterContent: "b",
        truncated: false,
        patch: "private unbounded patch",
      },
    });
    expect(() =>
      parseGitDiffReply(extra, requestId, "a.ts", "head_to_index"),
    ).toThrow(HostProtocolError);

    const drift = reply(requestId, {
      kind: "git_diff",
      value: {
        relativePath: "a.ts",
        comparison: "index_to_worktree",
        beforeContent: "a",
        afterContent: "b",
        truncated: false,
      },
    });
    expect(() =>
      parseGitDiffReply(drift, requestId, "a.ts", "head_to_worktree"),
    ).toThrow(HostProtocolError);
  });
});
