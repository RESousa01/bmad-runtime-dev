// @vitest-environment jsdom
import "./test/setup";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { WorkspaceExplorer } from "./components/WorkspaceExplorer";
import {
  DesktopHostClient,
  type BootstrapReply,
  type HostRuntime,
  type TauriInvoke,
} from "./lib/hostClient";
import type { ReadonlyWorkspaceSource } from "./lib/workspaceReadSource";

const digestA = `sha256:${"a".repeat(64)}`;
const digestB = `sha256:${"b".repeat(64)}`;
const digestC = `sha256:${"c".repeat(64)}`;

function successfulReply(requestId: string, data: unknown, sequence = 18) {
  return {
    schemaVersion: "desktop-dispatch-reply.v1",
    requestId,
    sequence,
    status: "ok",
    receipt: {
      requestId,
      acceptedAt: 1_725_000_000_005,
      operationId: null,
    },
    data,
  };
}

const recoveryBootstrap: BootstrapReply = {
  schemaVersion: "desktop-bootstrap.v1",
  rendererSessionId: "renderer_01K0Q6H3",
  installationId: "install_01K0Q6H3",
  windowLabel: "main",
  bootMode: "read_only_recovery",
  supportedCommands: ["app.get_boot_state", "workspace.list"],
  workspaces: [
    {
      workspaceId: "workspace_01K0Q6H3",
      projectId: "project_01K0Q6H3",
      displayName: "opaque-workspace-name",
      grantEpoch: 7,
      permissions: "read_only",
    },
  ],
  projectionSequence: 18,
};

async function recoveryRuntime(): Promise<HostRuntime> {
  const client = new DesktopHostClient({ invoke: async () => recoveryBootstrap });
  await client.bootstrap();
  return { kind: "read_only_recovery", client, bootstrap: recoveryBootstrap };
}

async function readyD1Runtime(): Promise<{ runtime: HostRuntime; invoke: ReturnType<typeof vi.fn<TauriInvoke>> }> {
  const content = "# Host projected readme\n\nLocal and read only.\n";
  const byteCount = new TextEncoder().encode(content).byteLength;
  const bootstrap: BootstrapReply = {
    ...recoveryBootstrap,
    bootMode: "ready",
    supportedCommands: [
      "app.get_boot_state",
      "workspace.select_folder",
      "workspace.list",
      "workspace.revoke",
      "workspace.list_entries",
      "workspace.read_text",
      "workspace.search",
      "bmad.scan",
      "context.preview",
    ],
  };
  let request = 0;
  const invoke = vi.fn<TauriInvoke>(async (command, args) => {
    if (command === "host_bootstrap") {
      return bootstrap;
    }
    const envelope = JSON.parse(String(args?.body)) as {
      command: string;
      requestId: string;
    };
    switch (envelope.command) {
      case "workspace.list_entries":
        return successfulReply(envelope.requestId, {
          kind: "workspace_entries",
          value: {
            workspaceId: bootstrap.workspaces[0]!.workspaceId,
            entries: [{
              relativePath: "README.md",
              kind: "text_file",
              sizeBytes: byteCount,
              childCursor: null,
            }],
            nextCursor: null,
          },
        });
      case "workspace.read_text":
        return successfulReply(envelope.requestId, {
          kind: "workspace_text",
          value: {
            relativePath: "README.md",
            content,
            contentHash: digestA,
            byteCount,
            truncated: false,
          },
        });
      case "workspace.search":
        return successfulReply(envelope.requestId, {
          kind: "search_results",
          value: [{ relativePath: "README.md", line: 1, preview: "# Host projected readme" }],
        });
      case "bmad.scan":
        return successfulReply(envelope.requestId, {
          kind: "bmad_scan",
          value: { status: "not_detected", assets: [], truncated: false },
        });
      case "context.preview":
        return successfulReply(envelope.requestId, {
          kind: "context_preview",
          value: {
            workspaceId: bootstrap.workspaces[0]!.workspaceId,
            manifestHash: digestB,
            items: [{
              relativePath: "README.md",
              startLine: 1,
              endLine: 3,
              reason: "Selected for this task",
              contentHash: digestA,
              classification: "source",
              redactions: [],
              byteCount,
              estimatedTokens: Math.floor((byteCount + 3) / 4),
              content,
            }],
            totalBytes: byteCount,
            estimatedTokens: Math.floor((byteCount + 3) / 4),
            modelTarget: null,
          },
        });
      default:
        throw new Error(`Unexpected command ${envelope.command}`);
    }
  });
  const client = new DesktopHostClient({
    invoke,
    requestId: () => `request_ui_${request += 1}`,
  });
  await client.bootstrap();
  return { runtime: { kind: "ready", client, bootstrap }, invoke };
}

async function workspaceManagementRuntime(
  revokeOutcome: "success" | "retryable_failure" | "recovery" = "success",
): Promise<{ runtime: HostRuntime; invoke: ReturnType<typeof vi.fn<TauriInvoke>> }> {
  const primary = {
    ...recoveryBootstrap.workspaces[0]!,
    displayName: "primary-workspace",
  };
  const secondary = {
    workspaceId: "workspace_01K0Q6H4",
    projectId: "project_01K0Q6H4",
    displayName: "secondary-workspace",
    grantEpoch: 11,
    permissions: "read_only" as const,
  };
  const bootstrap: BootstrapReply = {
    ...recoveryBootstrap,
    bootMode: "ready",
    supportedCommands: [
      "app.get_boot_state",
      "workspace.select_folder",
      "workspace.list",
      "workspace.revoke",
      "workspace.list_entries",
      "workspace.read_text",
      "workspace.search",
      "bmad.scan",
      "context.preview",
    ],
    workspaces: [primary, secondary],
  };
  let request = 0;
  const invoke = vi.fn<TauriInvoke>(async (command, args) => {
    if (command === "host_bootstrap") {
      return bootstrap;
    }
    const envelope = JSON.parse(String(args?.body)) as {
      command: string;
      payload: { workspaceId: string };
      requestId: string;
    };
    if (envelope.command !== "workspace.revoke") {
      throw new Error(`Unexpected command ${envelope.command}`);
    }
    if (revokeOutcome !== "success") {
      return {
        schemaVersion: "desktop-dispatch-reply.v1",
        requestId: envelope.requestId,
        sequence: 19,
        status: "error",
        error: {
          code: revokeOutcome === "recovery" ? "recovery_required" : "temporarily_unavailable",
          safeMessage: revokeOutcome === "recovery"
            ? "Workspace authority needs recovery."
            : "Workspace access could not be removed. Try again.",
          retryable: revokeOutcome === "retryable_failure",
          correlationId: envelope.requestId,
        },
      };
    }
    const workspace = bootstrap.workspaces.find(
      ({ workspaceId }) => workspaceId === envelope.payload.workspaceId,
    );
    if (!workspace) {
      throw new Error("Unknown opaque workspace in test host.");
    }
    return successfulReply(envelope.requestId, {
      kind: "workspace_revoked",
      value: { ...workspace, grantEpoch: workspace.grantEpoch + 1 },
    }, 19);
  });
  const client = new DesktopHostClient({
    invoke,
    requestId: () => `request_workspace_ui_${request += 1}`,
  });
  await client.bootstrap();
  return { runtime: { kind: "ready", client, bootstrap }, invoke };
}

describe("Sapphirus desktop workbench", () => {
  it("drops a stale Explorer page when the workspace source changes", async () => {
    let resolveStalePage!: (value: Awaited<ReturnType<ReadonlyWorkspaceSource["listEntries"]>>) => void;
    const stalePage = new Promise<Awaited<ReturnType<ReadonlyWorkspaceSource["listEntries"]>>>(
      (resolve) => {
        resolveStalePage = resolve;
      },
    );
    const source = (
      workspaceId: string,
      listEntries: ReadonlyWorkspaceSource["listEntries"],
    ): ReadonlyWorkspaceSource => ({
      provenance: "local_host",
      workspaceId,
      listEntries,
      previewContext: async () => { throw new Error("Not used by this test."); },
      readText: async () => { throw new Error("Not used by this test."); },
      scanBmad: async () => ({ status: "not_detected", assets: [], truncated: false }),
      search: async () => [],
    });
    const oldSource = source("workspace_old", async () => stalePage);
    const newSource = source("workspace_new", async () => ({
      workspaceId: "workspace_new",
      entries: [{
        relativePath: "new.ts",
        kind: "text_file",
        sizeBytes: 12,
        childCursor: null,
      }],
      nextCursor: null,
    }));
    const onContextReview = vi.fn();
    const { rerender } = render(
      <WorkspaceExplorer
        availabilityMessage="Unavailable"
        onContextReview={onContextReview}
        source={oldSource}
        workspaceName="Old workspace"
      />,
    );

    rerender(
      <WorkspaceExplorer
        availabilityMessage="Unavailable"
        onContextReview={onContextReview}
        source={newSource}
        workspaceName="New workspace"
      />,
    );
    expect(await screen.findByRole("button", { name: /new\.ts/i })).toBeTruthy();

    await act(async () => {
      resolveStalePage({
        workspaceId: "workspace_old",
        entries: [{
          relativePath: "old.ts",
          kind: "text_file",
          sizeBytes: 12,
          childCursor: null,
        }],
        nextCursor: null,
      });
      await stalePage;
    });

    expect(screen.queryByRole("button", { name: /old\.ts/i })).toBeNull();
    expect(screen.getByRole("heading", { name: "New workspace" })).toBeTruthy();
  });

  it("uses the approved vocabulary while clearly blocking preview-only effects", async () => {
    render(<App />);

    expect(screen.getByRole("navigation", { name: "Primary" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "New session" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Add a safe workspace scan" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Review changes" })).toBeTruthy();
    expect((screen.getByRole("button", { name: "Apply changes" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole("button", { name: "Review context" })).toBeTruthy();
    expect((screen.getByRole("button", { name: "Revise" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Discard" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getAllByText("Preview demo").length).toBeGreaterThan(0);
    expect((await screen.findAllByText("Browser preview")).length).toBeGreaterThan(0);

    expect(screen.queryByText(/^Chat$/i)).toBeNull();
    expect(screen.queryByText(/^Command$/i)).toBeNull();
    expect(screen.queryByText(/Approve & apply locally/i)).toBeNull();
    expect(screen.queryByText(/^Execute$/i)).toBeNull();
    expect(screen.queryByText(/^Auto$/i)).toBeNull();
    expect(screen.queryByText(/^Connected$/i)).toBeNull();
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\/);
  });

  it("offers native folder selection from Workspaces but keeps browser QA inert", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findAllByText("Browser preview");

    const workspaceTrigger = screen.getByRole("button", { name: "Workspaces" });
    await user.click(workspaceTrigger);

    expect(screen.getByRole("dialog", { name: "Local workspaces" })).toBeTruthy();
    const closeButton = screen.getByRole("button", { name: "Close workspaces" });
    expect(document.activeElement).toBe(closeButton);
    expect(document.querySelector(".app-surface")?.hasAttribute("inert")).toBe(true);
    expect(screen.getAllByText("bmad-runtime-dev").length).toBeGreaterThan(0);
    expect(screen.getByText(/absolute paths never enter renderer state/i)).toBeTruthy();
    expect((screen.getByRole("button", { name: "Choose local workspace" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: /remove workspace bmad-runtime-dev/i }) as HTMLButtonElement).disabled)
      .toBe(true);

    await user.tab();
    expect(document.activeElement).toBe(closeButton);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Local workspaces" })).toBeNull();
    expect(document.activeElement).toBe(workspaceTrigger);
  });

  it("keeps responsive drawers inert while closed and modal while open", async () => {
    const originalMatchMedia = window.matchMedia;
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: (query: string) => ({
        addEventListener: () => undefined,
        dispatchEvent: () => false,
        matches: query.includes("max-width"),
        media: query,
        onchange: null,
        removeEventListener: () => undefined,
      }),
    });
    const user = userEvent.setup();
    const view = render(<App />);
    try {
      await screen.findAllByText("Browser preview");
      const inspector = document.querySelector<HTMLElement>(".inspector")!;
      const sessions = document.querySelector<HTMLElement>(".session-rail")!;
      expect(inspector.getAttribute("aria-hidden")).toBe("true");
      expect(inspector.hasAttribute("inert")).toBe(true);
      expect(sessions.getAttribute("aria-hidden")).toBe("true");
      expect(sessions.hasAttribute("inert")).toBe(true);

      const inspectorTrigger = screen.getByRole("button", { name: "Open inspector" });
      await user.click(inspectorTrigger);
      expect(inspector.getAttribute("aria-hidden")).toBeNull();
      expect(inspector.getAttribute("aria-modal")).toBe("true");
      expect(inspector.hasAttribute("inert")).toBe(false);
      expect(screen.getByRole("navigation", { name: "Primary", hidden: true }).hasAttribute("inert"))
        .toBe(true);
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close inspector" }));

      await user.keyboard("{Escape}");
      expect(inspector.getAttribute("aria-hidden")).toBe("true");
      expect(document.activeElement).toBe(inspectorTrigger);

      const sessionsTrigger = screen.getByRole("button", { name: "Open sessions" });
      await user.click(sessionsTrigger);
      expect(sessions.getAttribute("aria-hidden")).toBeNull();
      expect(sessions.getAttribute("aria-modal")).toBe("true");
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close sessions" }));
      await user.keyboard("{Escape}");
      expect(sessions.getAttribute("aria-hidden")).toBe("true");
      expect(document.activeElement).toBe(sessionsTrigger);
    } finally {
      view.unmount();
      Object.defineProperty(window, "matchMedia", {
        configurable: true,
        value: originalMatchMedia,
      });
    }
  });

  it("moves focus into the non-modal settings dialog and returns it on close", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findAllByText("Browser preview");
    const settingsTrigger = screen.getByRole("button", { name: "Settings" });

    await user.click(settingsTrigger);
    const settingsDialog = screen.getByRole("dialog", { name: "Settings" });
    expect(settingsDialog.getAttribute("aria-modal")).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close settings" }));

    await user.click(screen.getByRole("button", { name: "Close settings" }));
    expect(screen.queryByRole("dialog", { name: "Settings" })).toBeNull();
    expect(document.activeElement).toBe(settingsTrigger);
  });

  it("provides an interactive but unmistakable browser-demo Explorer", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findAllByText("Browser preview");

    await user.click(screen.getByRole("button", { name: "Explorer" }));
    expect(await screen.findByRole("heading", { name: "bmad-runtime-dev" })).toBeTruthy();
    expect(screen.getByText("Browser demo data")).toBeTruthy();
    expect(screen.getByText(/No folder, host grant, or local file has been read/i)).toBeTruthy();
    expect(await screen.findByRole("button", { name: /README\.md/i })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: /README\.md/i }));
    expect(await screen.findByLabelText("Read-only preview of README.md")).toHaveProperty(
      "textContent",
      expect.stringContaining("governed Windows workspace companion"),
    );
    await user.click(screen.getByRole("checkbox", { name: "Include README.md in context" }));
    await user.click(screen.getByRole("button", { name: "Review context" }));

    expect(await screen.findByRole("heading", { name: "Review selected context" })).toBeTruthy();
    expect(screen.getByText(/Browser demo data · 1 item/)).toBeTruthy();
    expect(screen.getByText("No model request")).toBeTruthy();
    expect(screen.getAllByText(digestA).length).toBeGreaterThan(0);
    expect(screen.getByText(digestC)).toBeTruthy();
    expect(screen.getByText("No request sent")).toBeTruthy();

    await user.click(screen.getByRole("tab", { name: "Search" }));
    await user.type(screen.getByRole("searchbox", { name: "Search visible text" }), "governed");
    await user.click(screen.getByRole("button", { name: "Search" }));
    expect((await screen.findAllByText(/governed Windows workspace companion/i)).length)
      .toBeGreaterThan(0);

    await user.click(screen.getByRole("tab", { name: "BMAD" }));
    expect(screen.getByText("Builder Build draft")).toBeTruthy();
    expect(screen.getByText("Inactive draft")).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\/);
  });

  it("renders read surfaces and exact context review only from validated host projections", async () => {
    const { runtime, invoke } = await readyD1Runtime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "Explorer" }));
    expect(await screen.findByText("Validated local projection")).toBeTruthy();
    expect(screen.queryByText(/Browser demo data/i)).toBeNull();
    await user.click(await screen.findByRole("button", { name: /README\.md/i }));
    expect(await screen.findByLabelText("Read-only preview of README.md")).toHaveProperty(
      "textContent",
      expect.stringContaining("Host projected readme"),
    );
    await user.click(screen.getByRole("checkbox", { name: "Include README.md in context" }));
    await user.click(screen.getByRole("button", { name: "Review context" }));

    expect(await screen.findByRole("heading", { name: "Review selected context" })).toBeTruthy();
    expect(screen.getByText(/Validated local projection · 1 item/)).toBeTruthy();
    expect(screen.getByText(digestB)).toBeTruthy();
    const dispatchedCommands = invoke.mock.calls
      .filter(([command]) => command === "host_dispatch")
      .map(([, args]) => (JSON.parse(String(args?.body)) as { command: string }).command);
    expect(dispatchedCommands).toEqual(expect.arrayContaining([
      "workspace.list_entries",
      "workspace.read_text",
      "bmad.scan",
      "context.preview",
    ]));
    expect(dispatchedCommands).not.toEqual(expect.arrayContaining([
      "session.create",
      "task.submit",
      "approval.decide",
    ]));
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\/);
  });

  it("switches between validated opaque workspaces and removes only the selected grant", async () => {
    const { runtime, invoke } = await workspaceManagementRuntime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await screen.findAllByText("primary-workspace");
    await user.click(screen.getByRole("button", { name: "Workspaces" }));
    const dialog = screen.getByRole("dialog", { name: "Local workspaces" });
    const primaryRow = within(dialog).getByText("primary-workspace")
      .closest<HTMLElement>(".workspace-panel__row")!;
    const secondaryRow = within(dialog).getByText("secondary-workspace")
      .closest<HTMLElement>(".workspace-panel__row")!;
    expect(within(primaryRow).getByLabelText("Current workspace")).toBeTruthy();

    await user.click(within(secondaryRow).getByRole("button", {
      name: "Switch to workspace secondary-workspace",
    }));
    expect(within(secondaryRow).getByLabelText("Current workspace")).toBeTruthy();
    expect(within(primaryRow).getByRole("button", {
      name: "Switch to workspace primary-workspace",
    })).toBeTruthy();

    await user.click(within(secondaryRow).getByRole("button", {
      name: "Remove workspace secondary-workspace",
    }));
    await waitFor(() => {
      expect(within(dialog).queryByText("secondary-workspace")).toBeNull();
    });
    expect(within(primaryRow).getByLabelText("Current workspace")).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close workspaces" }));

    const revokeCalls = invoke.mock.calls.filter(([command, args]) => {
      if (command !== "host_dispatch") {
        return false;
      }
      return (JSON.parse(String(args?.body)) as { command: string }).command === "workspace.revoke";
    });
    expect(revokeCalls).toHaveLength(1);
    expect(JSON.parse(String(revokeCalls[0]![1]?.body))).toMatchObject({
      command: "workspace.revoke",
      payload: { workspaceId: "workspace_01K0Q6H4" },
    });
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\/);
  });

  it("keeps a workspace visible and actionable when grant removal fails safely", async () => {
    const { runtime } = await workspaceManagementRuntime("retryable_failure");
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await screen.findAllByText("primary-workspace");
    await user.click(screen.getByRole("button", { name: "Workspaces" }));
    const dialog = screen.getByRole("dialog", { name: "Local workspaces" });
    const remove = screen.getByRole("button", { name: "Remove workspace primary-workspace" });
    await user.click(remove);

    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      expect.stringContaining("Workspace access could not be removed. Try again."),
    );
    expect(within(dialog).getByText("primary-workspace")).toBeTruthy();
    expect((screen.getByRole("button", {
      name: "Remove workspace primary-workspace",
    }) as HTMLButtonElement).disabled).toBe(false);
  });

  it("enters inert recovery without dropping a workspace after a recovery-required removal", async () => {
    const { runtime } = await workspaceManagementRuntime("recovery");
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await screen.findAllByText("primary-workspace");
    await user.click(screen.getByRole("button", { name: "Workspaces" }));
    const dialog = screen.getByRole("dialog", { name: "Local workspaces" });
    await user.click(screen.getByRole("button", { name: "Remove workspace primary-workspace" }));

    expect(await screen.findByText(/workspace changes are blocked/i)).toBeTruthy();
    expect(within(dialog).getByText("primary-workspace")).toBeTruthy();
    expect((screen.getByRole("button", {
      name: "Remove workspace primary-workspace",
    }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Choose local workspace" }) as HTMLButtonElement).disabled)
      .toBe(true);
  });

  it("shows validated opaque workspace state and a calm recovery banner", async () => {
    const runtime = await recoveryRuntime();
    const user = userEvent.setup();
    render(<App hostRuntimeLoader={async () => runtime} />);

    expect(await screen.findByRole("status")).toHaveProperty("textContent", expect.stringContaining("Read-only recovery"));
    expect(screen.getAllByText("opaque-workspace-name").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "Workspaces" }));
    expect(screen.getByText(/workspace changes are blocked/i)).toBeTruthy();
    expect((screen.getByRole("button", { name: "Choose local workspace" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: /remove workspace opaque-workspace-name/i }) as HTMLButtonElement).disabled)
      .toBe(true);
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\/);
  });

  it("downgrades from ready when authoritative projections enter recovery", async () => {
    const readyBootstrap: BootstrapReply = {
      ...recoveryBootstrap,
      bootMode: "ready",
      supportedCommands: [
        "app.get_boot_state",
        "workspace.select_folder",
        "workspace.list",
        "workspace.revoke",
        "workspace.list_entries",
        "workspace.read_text",
        "workspace.search",
        "bmad.scan",
        "context.preview",
      ],
      projectionSequence: 18,
    };
    const invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      if (command === "host_projection_events") {
        const request = JSON.parse(String(args?.body)) as { afterSequence: number };
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId: readyBootstrap.rendererSessionId,
          status: "events",
          events: request.afterSequence === 18 ? [
            {
              sequence: 19,
              occurredAt: 1_725_000_000_010,
              event: {
                type: "workspace_changed",
                projection: { workspaceId: "workspace_01K0Q9N4" },
              },
            },
            {
              sequence: 20,
              occurredAt: 1_725_000_000_011,
              event: {
                type: "boot_state_changed",
                projection: { mode: "read_only_recovery" },
              },
            },
          ] : [],
        };
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return {
        schemaVersion: "desktop-dispatch-reply.v1",
        requestId: envelope.requestId,
        sequence: 20,
        status: "ok",
        receipt: {
          requestId: envelope.requestId,
          acceptedAt: 1_725_000_000_012,
          operationId: null,
        },
        data: {
          kind: "workspace_list",
          value: [{
            workspaceId: "workspace_01K0Q9N4",
            projectId: "project_01K0Q9N4",
            displayName: "refreshed-workspace",
            grantEpoch: 8,
            permissions: "read_only",
          }],
        },
      };
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => "request_01K0Q9N4",
    });
    await client.bootstrap();
    const runtime: HostRuntime = { kind: "ready", client, bootstrap: readyBootstrap };
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={5}
      />,
    );

    expect(await screen.findByRole("status")).toHaveProperty(
      "textContent",
      expect.stringContaining("Read-only recovery"),
    );
    expect((await screen.findAllByText("refreshed-workspace")).length).toBeGreaterThan(0);

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Workspaces" }));
    expect((screen.getByRole("button", { name: "Choose local workspace" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("changes preview inspector sections with keyboard-accessible tabs", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("tab", { name: "Context" }));
    expect(screen.getByRole("heading", { name: "Context review" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "No context selected" })).toBeTruthy();
    expect(screen.queryByText(/3,440 tokens/)).toBeNull();

    await user.click(screen.getByRole("tab", { name: "Evidence" }));
    expect(screen.getByRole("heading", { name: "Evidence" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "No evidence yet" })).toBeTruthy();
  });

  it("has no automated accessibility violations in the default state", async () => {
    const { container } = render(<App />);
    await screen.findAllByText("Browser preview");
    const results = await axe.run(container, {
      rules: {
        "color-contrast": { enabled: false },
      },
    });

    expect(results.violations).toEqual([]);
  });

  it("has no automated accessibility violations in the populated Explorer state", async () => {
    const user = userEvent.setup();
    const { container } = render(<App />);
    await screen.findAllByText("Browser preview");
    await user.click(screen.getByRole("button", { name: "Explorer" }));
    await screen.findByRole("button", { name: /README\.md/i });
    const results = await axe.run(container, {
      rules: {
        "color-contrast": { enabled: false },
      },
    });

    expect(results.violations).toEqual([]);
  });
});
