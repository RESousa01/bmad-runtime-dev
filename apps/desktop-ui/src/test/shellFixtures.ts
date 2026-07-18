import { vi } from "vitest";
import {
  DesktopHostClient,
  type BootstrapReply,
  type HostRuntime,
  type TauriInvoke,
  type WorkspaceProjection,
} from "../lib/hostClient";

export const primaryShellWorkspace: WorkspaceProjection = {
  workspaceId: "workspace_shell_primary",
  projectId: "project_shell_primary",
  displayName: "primary-workspace",
  grantEpoch: 7,
  permissions: "read_only",
};

export const secondaryShellWorkspace: WorkspaceProjection = {
  workspaceId: "workspace_shell_secondary",
  projectId: "project_shell_secondary",
  displayName: "secondary-workspace",
  grantEpoch: 11,
  permissions: "read_only",
};

function successfulReply(requestId: string, data: unknown) {
  return {
    schemaVersion: "desktop-dispatch-reply.v1",
    requestId,
    sequence: 19,
    status: "ok",
    receipt: {
      requestId,
      acceptedAt: 1_725_000_000_005,
      operationId: null,
    },
    data,
  };
}

export async function createReadyShellRuntime(
  workspaces: BootstrapReply["workspaces"] = [],
): Promise<{
  runtime: HostRuntime;
  invoke: ReturnType<typeof vi.fn<TauriInvoke>>;
}> {
  const bootstrap: BootstrapReply = {
    schemaVersion: "desktop-bootstrap.v1",
    rendererSessionId: "renderer_shell_integration",
    installationId: "install_shell_integration",
    windowLabel: "main",
    bootMode: "ready",
    supportedCommands: [
      "app.get_boot_state",
      "workspace.select_folder",
      "workspace.list",
      "workspace.list_entries",
      "workspace.read_text",
      "workspace.search",
      "bmad.scan",
      "bmad.library.snapshot",
      "context.preview",
    ],
    workspaces,
    projectionSequence: 18,
  };
  let request = 0;
  const invoke = vi.fn<TauriInvoke>(async (command, args) => {
    if (command === "host_bootstrap") {
      return bootstrap;
    }
    if (command === "host_projection_events") {
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId: bootstrap.rendererSessionId,
        status: "events",
        events: [],
      };
    }
    const envelope = JSON.parse(String(args?.body)) as {
      command: string;
      payload: { workspaceId?: string };
      requestId: string;
    };
    if (envelope.command === "workspace.select_folder") {
      return successfulReply(envelope.requestId, { kind: "no_selection" });
    }
    if (envelope.command === "workspace.list_entries") {
      return successfulReply(envelope.requestId, {
        kind: "workspace_entries",
        value: {
          workspaceId: envelope.payload.workspaceId,
          entries: [],
          nextCursor: null,
        },
      });
    }
    if (envelope.command === "bmad.scan") {
      return successfulReply(envelope.requestId, {
        kind: "bmad_scan",
        value: { status: "not_detected", assets: [], truncated: false },
      });
    }
    if (envelope.command === "bmad.library.snapshot") {
      return successfulReply(envelope.requestId, {
        kind: "bmad_library_snapshot",
        value: {
          schemaVersion: "bmad-library-snapshot.v2",
          scope: "installed_method",
          source: {
            sourceKind: "sealed_foundation",
            packageName: "bmad-method",
            packageVersion: "6.10.0",
          },
          installedSkills: [],
          helpActions: [],
          methodAgents: [],
          builderPackages: [],
          nextCursor: null,
        },
      });
    }
    throw new Error(`Unexpected shell fixture command ${envelope.command}`);
  });
  const client = new DesktopHostClient({
    invoke,
    requestId: () => `request_shell_${request += 1}`,
  });
  await client.bootstrap();
  return {
    runtime: { kind: "ready", client, bootstrap },
    invoke,
  };
}

export function dispatchedCommands(
  invoke: ReturnType<typeof vi.fn<TauriInvoke>>,
): string[] {
  return invoke.mock.calls.flatMap(([command, args]) => {
    if (command !== "host_dispatch") return [];
    return [(JSON.parse(String(args?.body)) as { command: string }).command];
  });
}
