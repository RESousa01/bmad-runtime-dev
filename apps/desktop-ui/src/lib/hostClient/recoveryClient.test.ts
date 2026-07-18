import { describe, expect, it, vi } from "vitest";
import { DesktopHostClient } from "./client";
import { HostCapabilityError, type BootstrapReply, type TauriInvoke } from "./contracts";

const digest = `sha256:${"a".repeat(64)}`;
const bootstrap: BootstrapReply = {
  schemaVersion: "desktop-bootstrap.v1",
  rendererSessionId: "renderer_01K0Q6H3",
  installationId: "install_01K0Q6H3",
  windowLabel: "main",
  bootMode: "ready",
  supportedCommands: [
    "workspace.list",
    "changes.history",
    "changes.recovery.prepare",
    "changes.recovery.decide",
  ],
  workspaces: [{
    workspaceId: "workspace_01K0Q6H3",
    projectId: "project_01K0Q6H3",
    displayName: "workspace",
    grantEpoch: 4,
    permissions: "governed_edits",
  }],
  projectionSequence: 12,
};

function success(requestId: string, kind: string, value: unknown, sequence = 13) {
  return {
    schemaVersion: "desktop-dispatch-reply.v1",
    requestId,
    sequence,
    status: "ok",
    receipt: { requestId, acceptedAt: 1_725_000_000_005, operationId: null },
    data: { kind, value },
  };
}

const prepared = {
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

describe("DesktopHostClient reviewed recovery", () => {
  it("dispatches strict prepare and consumes the retained review before one decision", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") return bootstrap;
      const envelope = JSON.parse(String(args?.body)) as {
        command: string;
        payload: Record<string, unknown>;
        requestId: string;
      };
      if (envelope.command === "changes.recovery.prepare") {
        return success(envelope.requestId, "changes_recovery_prepared", prepared, 12);
      }
      return success(envelope.requestId, "changes_recovery_decision", {
        recoveryApprovalId: prepared.recovery_approval_id,
        disposition: "recovered",
        journalId: prepared.journal_id,
        executionId: prepared.execution_id,
        restoredFiles: 1,
      }, 12);
    });
    let request = 0;
    const client = new DesktopHostClient({
      invoke,
      now: () => 1_725_000_000_000,
      requestId: () => `request_recovery_${++request}`,
    });
    await client.bootstrap();
    const review = await client.prepareChangesRecovery({
      workspaceId: "workspace_01K0Q6H3",
      workspaceGrantEpoch: 4,
      journalId: "journal_01K0Q6H3",
    });
    expect(review.status).toBe("review_required");
    await client.decideChangesRecovery({
      recoveryApprovalId: prepared.recovery_approval_id,
      displayedRecoveryHash: digest,
      choice: "restore",
    });
    await expect(client.decideChangesRecovery({
      recoveryApprovalId: prepared.recovery_approval_id,
      displayedRecoveryHash: digest,
      choice: "restore",
    })).rejects.toBeInstanceOf(HostCapabilityError);
    const envelopes = invoke.mock.calls.slice(1).map(([, args]) => {
      const envelope = JSON.parse(String(args?.body)) as { command: string; payload: unknown };
      return { command: envelope.command, payload: envelope.payload };
    });
    expect(envelopes).toEqual([
      {
        command: "changes.recovery.prepare",
        payload: {
          workspaceId: "workspace_01K0Q6H3",
          workspaceGrantEpoch: 4,
          journalId: "journal_01K0Q6H3",
        },
      },
      {
        command: "changes.recovery.decide",
        payload: {
          recoveryApprovalId: prepared.recovery_approval_id,
          displayedRecoveryHash: digest,
          choice: "restore",
        },
      },
    ]);
  });

  it("invalidates a retained review on refresh, bootstrap rebind, or any decision error", async () => {
    let dispatchMode: "prepare" | "error" = "prepare";
    let sequence = 12;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") return bootstrap;
      const envelope = JSON.parse(String(args?.body)) as { command: string; requestId: string };
      if (envelope.command === "changes.recovery.prepare") {
        return success(envelope.requestId, "changes_recovery_prepared", prepared, ++sequence);
      }
      if (dispatchMode === "error") throw new Error("transport failed");
      return success(envelope.requestId, "changes_history", {
        workspaceId: "workspace_01K0Q6H3",
        entries: [],
        openJournals: [],
      }, ++sequence);
    });
    const client = new DesktopHostClient({
      invoke,
      now: () => 1_725_000_000_000,
      requestId: () => "request_recovery_01K0Q6H3",
    });
    await client.bootstrap();
    const input = {
      recoveryApprovalId: prepared.recovery_approval_id,
      displayedRecoveryHash: digest,
      choice: "cancel" as const,
    };

    await client.prepareChangesRecovery({ workspaceId: "workspace_01K0Q6H3", workspaceGrantEpoch: 4, journalId: "journal_01K0Q6H3" });
    await client.changesHistory("workspace_01K0Q6H3", 4);
    await expect(client.decideChangesRecovery(input)).rejects.toBeInstanceOf(HostCapabilityError);

    await client.prepareChangesRecovery({ workspaceId: "workspace_01K0Q6H3", workspaceGrantEpoch: 4, journalId: "journal_01K0Q6H3" });
    await client.bootstrap();
    await expect(client.decideChangesRecovery(input)).rejects.toBeInstanceOf(HostCapabilityError);

    await client.prepareChangesRecovery({ workspaceId: "workspace_01K0Q6H3", workspaceGrantEpoch: 4, journalId: "journal_01K0Q6H3" });
    dispatchMode = "error";
    await expect(client.decideChangesRecovery(input)).rejects.toThrow("transport failed");
    await expect(client.decideChangesRecovery(input)).rejects.toBeInstanceOf(HostCapabilityError);
  });

  it("invalidates retained recovery when the workspace grant changes", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") return bootstrap;
      const envelope = JSON.parse(String(args?.body)) as { command: string; requestId: string };
      if (envelope.command === "changes.recovery.prepare") {
        return success(envelope.requestId, "changes_recovery_prepared", prepared, 12);
      }
      if (envelope.command === "workspace.list") {
        return success(envelope.requestId, "workspace_list", [{
          ...bootstrap.workspaces[0]!,
          grantEpoch: 5,
        }], 12);
      }
      throw new Error(`Unexpected command ${envelope.command}`);
    });
    const client = new DesktopHostClient({
      invoke,
      now: () => 1_725_000_000_000,
      requestId: () => "request_recovery_01K0Q6H3",
    });
    await client.bootstrap();
    await client.prepareChangesRecovery({
      workspaceId: "workspace_01K0Q6H3",
      workspaceGrantEpoch: 4,
      journalId: "journal_01K0Q6H3",
    });
    await client.listWorkspaces();
    await expect(client.decideChangesRecovery({
      recoveryApprovalId: prepared.recovery_approval_id,
      displayedRecoveryHash: digest,
      choice: "restore",
    })).rejects.toBeInstanceOf(HostCapabilityError);
  });
});
