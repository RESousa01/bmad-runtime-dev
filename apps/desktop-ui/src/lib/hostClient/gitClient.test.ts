import { describe, expect, it, vi } from "vitest";

import { DesktopHostClient } from "./client";
import {
  DISPATCH_REPLY_SCHEMA,
  type BootstrapReply,
  type TauriInvoke,
} from "./contracts";

const bootstrap: BootstrapReply = {
  schemaVersion: "desktop-bootstrap.v1",
  rendererSessionId: "renderer_forge_review",
  installationId: "install_forge_review",
  windowLabel: "main",
  bootMode: "ready",
  supportedCommands: [
    "app.get_boot_state",
    "workspace.git_status",
    "workspace.git_diff",
  ],
  workspaces: [
    {
      workspaceId: "workspace_forge_review",
      projectId: "project_forge_review",
      displayName: "forge",
      grantEpoch: 1,
      permissions: "read_only",
      contextReadEpoch: 1,
      governedEditEpoch: 1,
    },
  ],
  projectionSequence: 4,
};

function success(requestId: string, data: unknown, sequence: number): unknown {
  return {
    schemaVersion: DISPATCH_REPLY_SCHEMA,
    requestId,
    sequence,
    status: "ok",
    receipt: { requestId, acceptedAt: 10_000, operationId: null },
    data,
  };
}

describe("DesktopHostClient Git Review", () => {
  it("dispatches status and a selected baseline through the closed host boundary", async () => {
    const envelopes: Array<{ command: string; payload: unknown }> = [];
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") return bootstrap;
      const envelope = JSON.parse(String(args?.body)) as {
        requestId: string;
        command: string;
        payload: unknown;
      };
      envelopes.push(envelope);
      if (envelope.command === "workspace.git_status") {
        return success(envelope.requestId, {
          kind: "git_status",
          value: {
            branch: "main",
            entries: [
              {
                relativePath: "src/App.tsx",
                indexState: "modified",
                worktreeState: "modified",
              },
            ],
            truncated: false,
          },
        }, 5);
      }
      return success(envelope.requestId, {
        kind: "git_diff",
        value: {
          relativePath: "src/App.tsx",
          comparison: "head_to_index",
          beforeContent: "before\n",
          afterContent: "staged\n",
          truncated: false,
        },
      }, 6);
    });
    let request = 0;
    const client = new DesktopHostClient({
      invoke,
      now: () => 10_000,
      requestId: () => `request_git_${request += 1}`,
    });
    await client.bootstrap();

    await expect(client.readGitStatus("workspace_forge_review")).resolves.toMatchObject({
      branch: "main",
      entries: [{ relativePath: "src/App.tsx" }],
    });
    await expect(
      client.readGitDiff(
        "workspace_forge_review",
        "src/App.tsx",
        "head_to_index",
      ),
    ).resolves.toMatchObject({ afterContent: "staged\n" });
    expect(envelopes.map(({ command, payload }) => ({ command, payload }))).toEqual([
      {
        command: "workspace.git_status",
        payload: { workspaceId: "workspace_forge_review" },
      },
      {
        command: "workspace.git_diff",
        payload: {
          workspaceId: "workspace_forge_review",
          relativePath: "src/App.tsx",
          comparison: "head_to_index",
        },
      },
    ]);
  });
});
