// @vitest-environment jsdom
import "./test/setup";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
import type { BmadHelpRunCreatedProjection } from "./lib/bmadProjection";
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
      "bmad.library.snapshot",
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
      "bmad.library.snapshot",
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

async function bmadLibraryRuntime({
  dropBmadAfterRebind = false,
  emitInvalidation = false,
  expireFirstSession = false,
  holdOldProjection = false,
}: {
  dropBmadAfterRebind?: boolean;
  emitInvalidation?: boolean;
  expireFirstSession?: boolean;
  holdOldProjection?: boolean;
} = {}): Promise<{
  runtime: HostRuntime;
  invoke: ReturnType<typeof vi.fn<TauriInvoke>>;
  releaseInvalidation: () => void;
  releaseOldProjection: () => void;
}> {
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
      "bmad.library.snapshot",
      "context.preview",
    ],
  };
  const reboundBootstrap: BootstrapReply = {
    ...bootstrap,
    rendererSessionId: "renderer_01K0Q6H3_rebound",
    supportedCommands: dropBmadAfterRebind
      ? bootstrap.supportedCommands.filter((command) => command !== "bmad.library.snapshot")
      : bootstrap.supportedCommands,
  };
  let request = 0;
  let bootstrapCount = 0;
  let libraryAttemptCount = 0;
  let librarySnapshotCount = 0;
  let invalidationDelivered = false;
  let invalidationReleased = false;
  let releaseHeldProjection: () => void = () => undefined;
  const invoke = vi.fn<TauriInvoke>(async (command, args) => {
    if (command === "host_bootstrap") {
      bootstrapCount += 1;
      return expireFirstSession && bootstrapCount > 1 ? reboundBootstrap : bootstrap;
    }
    if (command === "host_projection_events") {
      if (holdOldProjection && bootstrapCount === 1) {
        await new Promise<void>((resolve) => {
          releaseHeldProjection = resolve;
        });
      }
      const shouldInvalidate = emitInvalidation
        && invalidationReleased
        && librarySnapshotCount > 0
        && !invalidationDelivered;
      invalidationDelivered ||= shouldInvalidate;
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId: expireFirstSession && bootstrapCount > 1
          ? reboundBootstrap.rendererSessionId
          : bootstrap.rendererSessionId,
        status: "events",
        events: shouldInvalidate ? [{
          sequence: 19,
          occurredAt: 1_725_000_000_010,
          event: {
            type: "bmad.projection_changed",
            projection: { scope: "library" },
          },
        }] : [],
      };
    }
    const envelope = JSON.parse(String(args?.body)) as {
      command: string;
      requestId: string;
    };
    if (envelope.command !== "bmad.library.snapshot") {
      throw new Error(`Unexpected command ${envelope.command}`);
    }
    libraryAttemptCount += 1;
    if (expireFirstSession && libraryAttemptCount === 1) {
      return {
        schemaVersion: "desktop-dispatch-reply.v1",
        requestId: null,
        sequence: 18,
        status: "error",
        error: {
          code: "renderer_session_expired",
          safeMessage: "The renderer session expired.",
          retryable: true,
          correlationId: null,
        },
      };
    }
    const refreshed = librarySnapshotCount > 0;
    librarySnapshotCount += 1;
    return successfulReply(envelope.requestId, {
      kind: "bmad_library_snapshot",
      value: {
        schemaVersion: "bmad-library-snapshot.v1",
        scope: "installed_method",
        source: {
          sourceKind: "sealed_foundation",
          packageName: "bmad-method",
          packageVersion: "6.10.0",
        },
        installedSkills: [{
          moduleCode: "bmm",
          skillName: "bmad-architecture",
          displayName: refreshed ? "Review Architecture" : "Create Architecture",
          description: "Create a bounded architecture spine.",
          actions: ["create"],
          entrypointKind: "step_jit",
          distributionProfile: "sapphirus_package",
          installProfile: "SapphirusManagedV1",
          validationProfile: "MethodStepWorkflowV6",
          availability: "capability_disabled",
          blockerCodes: ["bmad_capability_disabled"],
          hiddenFromHelp: false,
        }],
        helpActions: [{
          moduleCode: "bmm",
          skillName: "bmad-architecture",
          action: "create",
          displayName: "Architecture",
          menuCode: "CA",
          description: "Create the architecture spine.",
          requiredGuidance: true,
          expectedArtifacts: ["architecture"],
          availability: "capability_disabled",
          blockerCodes: ["bmad_capability_disabled"],
        }],
        methodAgents: [{
          moduleCode: "bmm",
          agentCode: "bmad-agent-architect",
          name: "Winston",
          title: "System Architect",
          icon: "A",
          team: "software-development",
          description: "Reviews architecture trade-offs.",
          availability: "capability_disabled",
          blockerCodes: ["bmad_capability_disabled"],
          menus: [{
            code: "CA",
            description: "Create architecture",
            targetKind: "skill_target",
            displayLabel: "Architecture",
            availability: "capability_disabled",
            availabilityReason: "bmad_capability_disabled",
          }],
        }],
        nextCursor: null,
      },
    }, refreshed ? 19 : 18);
  });
  const client = new DesktopHostClient({
    invoke,
    requestId: () => `request_bmad_ui_${request += 1}`,
  });
  await client.bootstrap();
  return {
    runtime: { kind: "ready", client, bootstrap },
    invoke,
    releaseInvalidation: () => { invalidationReleased = true; },
    releaseOldProjection: () => releaseHeldProjection(),
  };
}

function bmadHelpRun(
  workspaceId: string,
  label = "BMad Help",
  suffix = "01K0Q6H3",
): BmadHelpRunCreatedProjection {
  return {
    schemaVersion: "bmad-help-run.v1",
    runKind: "bmad_help",
    lifecycle: "created_unbound",
    workspaceId,
    runId: `run_${suffix}`,
    sessionId: `session_${suffix}`,
    runnable: false,
    completionClaimed: false,
    recommendation: {
      schemaVersion: "bmad-help-recommendation.v1",
      displayName: label,
      moduleCode: "core",
      skillName: "bmad-help",
      action: null,
      confidence: "unknown",
      source: {
        sourceKind: "sealed_foundation",
        packageName: "bmad-method",
        packageVersion: "6.10.0",
      },
      reason: "The current intent most closely matches the catalog entry BMad Help.",
      requiredGuidance: true,
      expectedArtifacts: [],
      availability: "capability_disabled",
      blockerCodes: ["bmad_capability_disabled"],
      completionClaimed: false,
    },
  };
}

type BmadHelpLatestFixture =
  | BmadHelpRunCreatedProjection
  | "projection_unavailable"
  | null;

async function bmadHelpRuntime({
  createOutcome = "success",
  createRun,
  holdCreate = false,
  holdLatestWorkspaceId = null,
  latestRuns = {},
  workspaces = recoveryBootstrap.workspaces,
}: {
  createOutcome?: "success" | "failure" | "recovery" | "renderer_expired";
  createRun?: BmadHelpRunCreatedProjection;
  holdCreate?: boolean;
  holdLatestWorkspaceId?: string | null;
  latestRuns?: Readonly<Record<string, BmadHelpLatestFixture>>;
  workspaces?: BootstrapReply["workspaces"];
} = {}): Promise<{
  runtime: HostRuntime;
  invoke: ReturnType<typeof vi.fn<TauriInvoke>>;
  createStarted: Promise<void>;
  releaseCreate: () => void;
  releaseLatest: () => void;
}> {
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
      "bmad.library.snapshot",
      "bmad.help.latest",
      "run.create",
      "context.preview",
    ],
    workspaces,
  };
  let request = 0;
  let resolveCreateStarted!: () => void;
  let resolveCreate!: () => void;
  let resolveLatest!: () => void;
  const createStarted = new Promise<void>((resolve) => { resolveCreateStarted = resolve; });
  const heldCreate = new Promise<void>((resolve) => { resolveCreate = resolve; });
  const heldLatest = new Promise<void>((resolve) => { resolveLatest = resolve; });
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
      payload: {
        currentIntent?: string;
        workspaceGrantEpoch?: number;
        workspaceId?: string;
      };
      requestId: string;
    };
    if (envelope.command === "bmad.help.latest") {
      const workspaceId = envelope.payload.workspaceId!;
      if (workspaceId === holdLatestWorkspaceId) {
        await heldLatest;
      }
      const retained = latestRuns[workspaceId] ?? null;
      return successfulReply(
        envelope.requestId,
        retained === "projection_unavailable"
          ? { kind: "bmad_help_projection_unavailable" }
          : retained === null
          ? { kind: "no_bmad_help_run" }
          : { kind: "bmad_help_run_created", value: retained },
      );
    }
    if (envelope.command === "run.create") {
      resolveCreateStarted();
      if (holdCreate) {
        await heldCreate;
      }
      if (createOutcome !== "success") {
        return {
          schemaVersion: "desktop-dispatch-reply.v1",
          requestId: envelope.requestId,
          sequence: 19,
          status: "error",
          error: {
            code: createOutcome === "recovery"
              ? "integrity_failure"
              : createOutcome === "renderer_expired"
                ? "renderer_session_expired"
                : "temporarily_unavailable",
            safeMessage: createOutcome === "recovery"
              ? "Workspace authority needs recovery."
              : createOutcome === "renderer_expired"
                ? "The renderer session expired. Method guidance was not created."
                : "Method guidance could not be created. Nothing was changed.",
            retryable: createOutcome === "failure" || createOutcome === "renderer_expired",
            correlationId: envelope.requestId,
          },
        };
      }
      const projection = createRun
        ?? bmadHelpRun(envelope.payload.workspaceId!, "BMad Help", "01K0Q6H4");
      const reply = successfulReply(envelope.requestId, {
        kind: "bmad_help_run_created",
        value: projection,
      }, 19);
      return {
        ...reply,
        receipt: { ...reply.receipt, operationId: projection.runId },
      };
    }
    if (envelope.command === "bmad.library.snapshot") {
      return successfulReply(envelope.requestId, {
        kind: "bmad_library_snapshot",
        value: {
          schemaVersion: "bmad-library-snapshot.v1",
          scope: "installed_method",
          source: {
            sourceKind: "sealed_foundation",
            packageName: "bmad-method",
            packageVersion: "6.10.0",
          },
          installedSkills: [],
          helpActions: [],
          methodAgents: [],
          nextCursor: null,
        },
      }, 19);
    }
    throw new Error(`Unexpected command ${envelope.command}`);
  });
  const client = new DesktopHostClient({
    invoke,
    requestId: () => `request_help_ui_${request += 1}`,
  });
  await client.bootstrap();
  return {
    runtime: { kind: "ready", client, bootstrap },
    invoke,
    createStarted,
    releaseCreate: resolveCreate,
    releaseLatest: resolveLatest,
  };
}

async function readyMethodGuidanceComposer(): Promise<HTMLTextAreaElement> {
  const composer = await screen.findByLabelText<HTMLTextAreaElement>(
    "Describe what you want Method guidance for",
  );
  await waitFor(() => expect(composer).toHaveProperty("disabled", false));
  return composer;
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
    expect(screen.queryByRole("button", { name: "Method library" })).toBeNull();
    expect(screen.queryByRole("tab", { name: "Method library" })).toBeNull();
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
        "bmad.library.snapshot",
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

  it("restores the latest retained Help run for the exact active workspace grant", async () => {
    const workspace = recoveryBootstrap.workspaces[0]!;
    const retained = bmadHelpRun(workspace.workspaceId, "Restored Method guidance");
    const { runtime, invoke } = await bmadHelpRuntime({
      latestRuns: { [workspace.workspaceId]: retained },
    });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await waitFor(() => {
      const latestCalls = invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
        && (JSON.parse(String(args?.body)) as { command: string }).command === "bmad.help.latest");
      expect(latestCalls).toHaveLength(1);
    });
    await user.click(await screen.findByRole("button", { name: "Method library" }));

    expect(await screen.findByText("Restored Method guidance")).toBeTruthy();
    expect(screen.getByText("Created")).toBeTruthy();
    expect(screen.getByText("Unbound")).toBeTruthy();
    expect(screen.getByText("No model request")).toBeTruthy();
    const latestCall = invoke.mock.calls.find(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "bmad.help.latest")!;
    expect(JSON.parse(String(latestCall[1]?.body))).toMatchObject({
      command: "bmad.help.latest",
      payload: {
        workspaceId: workspace.workspaceId,
        workspaceGrantEpoch: workspace.grantEpoch,
      },
    });
  });

  it("blocks Help creation until the retained run lookup finishes", async () => {
    const workspace = recoveryBootstrap.workspaces[0]!;
    const { runtime, releaseLatest } = await bmadHelpRuntime({
      holdLatestWorkspaceId: workspace.workspaceId,
    });
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    const composer = await screen.findByLabelText(
      "Describe what you want Method guidance for",
    );
    expect(composer).toHaveProperty("disabled", true);
    expect(
      screen.getByRole("button", { name: "Request Method guidance" }),
    ).toHaveProperty("disabled", true);

    releaseLatest();
    await waitFor(() => expect(composer).toHaveProperty("disabled", false));
  });

  it("warns about an unrestorable legacy projection but permits a fresh retained run", async () => {
    const workspace = recoveryBootstrap.workspaces[0]!;
    const { runtime, invoke } = await bmadHelpRuntime({
      latestRuns: { [workspace.workspaceId]: "projection_unavailable" },
    });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await waitFor(() => {
      const latestCalls = invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
        && (JSON.parse(String(args?.body)) as { command: string }).command === "bmad.help.latest");
      expect(latestCalls).toHaveLength(1);
    });
    await user.click(await screen.findByRole("button", { name: "Method library" }));

    const legacyWarning = await screen.findByRole("alert");
    expect(legacyWarning).toHaveProperty(
      "textContent",
      expect.stringContaining("retained Method session"),
    );
    expect(legacyWarning).toHaveProperty(
      "textContent",
      expect.stringContaining("You can create a new local Method session"),
    );
    const composer = screen.getByLabelText("Describe what you want Method guidance for");
    expect(composer).toHaveProperty("disabled", false);
    expect(screen.queryByText(/^Created$/)).toBeNull();

    await user.type(composer, "Create a newly retained Method session");
    await user.click(screen.getByRole("button", { name: "Request Method guidance" }));

    expect(await screen.findByText("Created · Unbound")).toBeTruthy();
    expect(invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "run.create"))
      .toHaveLength(1);
  });

  it("creates one truthful unbound Help run from the exact submitted intent", async () => {
    const workspace = recoveryBootstrap.workspaces[0]!;
    const created = bmadHelpRun(workspace.workspaceId, "Source-grounded Help", "01K0Q6H4");
    const { runtime, invoke } = await bmadHelpRuntime({ createRun: created });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    const composer = await readyMethodGuidanceComposer();
    await user.type(composer, "  Choose the next safe architecture step.  ");
    await user.click(screen.getByRole("button", { name: "Request Method guidance" }));

    expect(await screen.findByText("Source-grounded Help")).toBeTruthy();
    expect(screen.getAllByText("Created").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Unbound").length).toBeGreaterThan(0);
    expect(screen.getAllByText("No model request").length).toBeGreaterThan(0);
    expect(screen.queryByText("Demo response")).toBeNull();
    expect(screen.queryByText("Ready for review")).toBeNull();
    const createCalls = invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "run.create");
    expect(createCalls).toHaveLength(1);
    const createEnvelope = JSON.parse(String(createCalls[0]![1]?.body)) as {
      command: string;
      payload: unknown;
    };
    expect(createEnvelope.command).toBe("run.create");
    expect(createEnvelope.payload).toEqual({
      workspaceId: workspace.workspaceId,
      workspaceGrantEpoch: workspace.grantEpoch,
      runKind: "bmad_help",
      currentIntent: "Choose the next safe architecture step.",
    });
  });

  it("suppresses duplicate Help creation while the first native request is pending", async () => {
    const { runtime, invoke, createStarted, releaseCreate } = await bmadHelpRuntime({
      holdCreate: true,
    });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await user.type(
      await readyMethodGuidanceComposer(),
      "Recommend one safe Method step",
    );
    const submit = screen.getByRole("button", { name: "Request Method guidance" });
    fireEvent.click(submit);
    fireEvent.click(submit);
    await createStarted;

    const createCalls = () => invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "run.create");
    expect(createCalls()).toHaveLength(1);
    expect(submit).toHaveProperty("disabled", true);

    releaseCreate();
    expect(await screen.findByText("Created · Unbound")).toBeTruthy();
    expect(createCalls()).toHaveLength(1);
  });

  it("never claims creation when the native Help command fails", async () => {
    const { runtime, invoke } = await bmadHelpRuntime({ createOutcome: "failure" });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await user.type(
      await readyMethodGuidanceComposer(),
      "Recommend a safe next step",
    );
    await user.click(screen.getByRole("button", { name: "Request Method guidance" }));
    await waitFor(() => {
      const createCalls = invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
        && (JSON.parse(String(args?.body)) as { command: string }).command === "run.create");
      expect(createCalls).toHaveLength(1);
    });
    await user.click(screen.getByRole("button", { name: "Method library" }));

    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      expect.stringContaining("Method guidance could not be created. Nothing was changed."),
    );
    expect(screen.queryByText("Created · Unbound")).toBeNull();
    expect(screen.queryByText(/^Created$/)).toBeNull();
    expect(screen.queryByText("Demo response")).toBeNull();
    expect(
      screen.getByLabelText("Describe what you want Method guidance for"),
    ).toHaveProperty("disabled", true);
    const createCalls = () => invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "run.create");
    fireEvent.submit(document.querySelector<HTMLFormElement>(".composer")!);
    expect(createCalls()).toHaveLength(1);
  });

  it("does not rebind or retry a Help mutation after renderer-session expiry", async () => {
    const { runtime, invoke } = await bmadHelpRuntime({
      createOutcome: "renderer_expired",
    });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await user.type(
      await readyMethodGuidanceComposer(),
      "Recommend a safe next step",
    );
    await user.click(screen.getByRole("button", { name: "Request Method guidance" }));
    await waitFor(() => {
      expect(
        screen.getByLabelText("Describe what you want Method guidance for"),
      ).toHaveProperty("disabled", true);
    });

    const dispatches = invoke.mock.calls
      .filter(([command]) => command === "host_dispatch")
      .map(([, args]) => (JSON.parse(String(args?.body)) as { command: string }).command);
    expect(dispatches.filter((command) => command === "run.create")).toHaveLength(1);
    expect(invoke.mock.calls.filter(([command]) => command === "host_bootstrap")).toHaveLength(1);
    expect(screen.queryByText("Created · Unbound")).toBeNull();
  });

  it("rejects a stale Help creation result after the native host binding changes", async () => {
    const oldHost = await bmadHelpRuntime({ holdCreate: true });
    const currentHost = await bmadHelpRuntime();
    const user = userEvent.setup();
    const view = render(
      <App
        hostRuntimeLoader={async () => oldHost.runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await user.type(
      await readyMethodGuidanceComposer(),
      "Recommend a safe next step",
    );
    await user.click(screen.getByRole("button", { name: "Request Method guidance" }));
    await oldHost.createStarted;

    view.rerender(
      <App
        hostRuntimeLoader={async () => currentHost.runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await waitFor(() => {
      const latestCalls = currentHost.invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
        && (JSON.parse(String(args?.body)) as { command: string }).command === "bmad.help.latest");
      expect(latestCalls).toHaveLength(1);
    });

    await act(async () => {
      oldHost.releaseCreate();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.queryByText("Creating · Local only")).toBeNull();
    });
    expect(screen.queryByText("Created · Unbound")).toBeNull();
    expect(screen.queryByText(/^Created$/)).toBeNull();
    expect(screen.queryByText("Demo response")).toBeNull();
    expect(
      screen.getByLabelText("Describe what you want Method guidance for"),
    ).toHaveProperty("disabled", false);
  });

  it("enters read-only recovery when Help creation reports an integrity failure", async () => {
    const { runtime } = await bmadHelpRuntime({ createOutcome: "recovery" });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await user.type(
      await readyMethodGuidanceComposer(),
      "Recommend a safe next step",
    );
    await user.click(screen.getByRole("button", { name: "Request Method guidance" }));

    expect((await screen.findAllByText("Read-only recovery")).length).toBeGreaterThan(0);
    expect(
      screen.getByLabelText("Describe what you want Method guidance for"),
    ).toHaveProperty("disabled", true);
    expect(screen.queryByText("Created · Unbound")).toBeNull();
  });

  it("drops a retained Help result that arrives after the active workspace changes", async () => {
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
    const primaryRun = bmadHelpRun(primary.workspaceId, "Stale primary guidance", "01K0Q6H5");
    const secondaryRun = bmadHelpRun(secondary.workspaceId, "Current secondary guidance", "01K0Q6H6");
    const { runtime, releaseLatest } = await bmadHelpRuntime({
      holdLatestWorkspaceId: primary.workspaceId,
      latestRuns: {
        [primary.workspaceId]: primaryRun,
        [secondary.workspaceId]: secondaryRun,
      },
      workspaces: [primary, secondary],
    });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await screen.findAllByText("primary-workspace");
    await user.click(screen.getByRole("button", { name: "Workspaces" }));
    await user.click(screen.getByRole("button", {
      name: "Switch to workspace secondary-workspace",
    }));
    await user.click(screen.getByRole("button", { name: "Close workspaces" }));
    await user.click(await screen.findByRole("button", { name: "Method library" }));
    expect(await screen.findByText("Current secondary guidance")).toBeTruthy();

    await act(async () => {
      releaseLatest();
      await Promise.resolve();
    });
    expect(screen.getByText("Current secondary guidance")).toBeTruthy();
    expect(screen.queryByText("Stale primary guidance")).toBeNull();
  });

  it("loads the native Method library once from the Agent workbench", async () => {
    const { runtime, invoke } = await bmadLibraryRuntime();
    const user = userEvent.setup();
    const { container } = render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    const trigger = await screen.findByRole("button", { name: "Method library" });
    await user.click(trigger);
    expect(await screen.findByRole("heading", { name: "Method library" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Installed skills" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Available actions" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Method agents" })).toBeTruthy();
    expect(screen.getByText("Create Architecture")).toBeTruthy();
    expect(screen.getByText("Winston")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Suggested next step" })).toBeTruthy();
    expect(screen.getByText(/No active governed session/i)).toBeTruthy();

    const bmadCalls = invoke.mock.calls.filter(([command, args]) => {
      if (command !== "host_dispatch") return false;
      return (JSON.parse(String(args?.body)) as { command: string }).command
        === "bmad.library.snapshot";
    });
    expect(bmadCalls).toHaveLength(1);
    expect(JSON.parse(String(bmadCalls[0]![1]?.body))).toMatchObject({
      command: "bmad.library.snapshot",
      payload: { scope: "installed_method", cursor: null },
    });
    const methodTab = screen.getByRole("tab", { name: "Method library" });
    methodTab.focus();
    await user.keyboard("{ArrowLeft}");
    expect(screen.getByRole("tab", { name: "Evidence" }).getAttribute("aria-selected")).toBe("true");
    await user.keyboard("{ArrowRight}");
    expect(methodTab.getAttribute("aria-selected")).toBe("true");
    const accessibility = await axe.run(container, {
      rules: { "color-contrast": { enabled: false } },
    });
    expect(accessibility.violations).toEqual([]);
  });

  it("does not fabricate a Method library in browser preview or recovery", async () => {
    const browser = render(<App />);
    await screen.findAllByText("Browser preview");
    expect(screen.queryByRole("button", { name: "Method library" })).toBeNull();
    expect(screen.queryByRole("tab", { name: "Method library" })).toBeNull();
    expect(screen.getByLabelText("Describe a task")).toHaveProperty("disabled", true);
    browser.unmount();

    const runtime = await recoveryRuntime();
    render(<App hostRuntimeLoader={async () => runtime} />);
    await screen.findAllByText("Read-only recovery");
    expect(screen.queryByRole("button", { name: "Method library" })).toBeNull();
    expect(screen.queryByRole("tab", { name: "Method library" })).toBeNull();
    expect(screen.getByLabelText("Describe a task")).toHaveProperty("disabled", true);
  });

  it("replaces a requested Method library after native projection invalidation", async () => {
    const { runtime, invoke, releaseInvalidation } = await bmadLibraryRuntime({ emitInvalidation: true });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={5}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "Method library" }));
    expect(await screen.findByText("Create Architecture")).toBeTruthy();
    releaseInvalidation();
    expect(await screen.findByText("Review Architecture")).toBeTruthy();
    expect(screen.queryByText("Create Architecture")).toBeNull();

    await waitFor(() => {
      const calls = invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
        && (JSON.parse(String(args?.body)) as { command: string }).command === "bmad.library.snapshot");
      expect(calls).toHaveLength(2);
    });
  });

  it("re-establishes an expired renderer session before retrying the Method snapshot", async () => {
    const { runtime, invoke } = await bmadLibraryRuntime({ expireFirstSession: true });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "Method library" }));
    expect(await screen.findByText("Create Architecture")).toBeTruthy();

    const bootstrapCalls = invoke.mock.calls.filter(([command]) => command === "host_bootstrap");
    expect(bootstrapCalls).toHaveLength(2);
    const snapshotCalls = invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "bmad.library.snapshot");
    expect(snapshotCalls).toHaveLength(2);
    expect(JSON.parse(String(snapshotCalls[1]![1]?.body))).toMatchObject({
      rendererSessionId: "renderer_01K0Q6H3_rebound",
      payload: { scope: "installed_method", cursor: null },
    });
  });

  it("hides only Method controls when a rebound ready host removes the BMAD capability", async () => {
    const { runtime } = await bmadLibraryRuntime({
      dropBmadAfterRebind: true,
      expireFirstSession: true,
    });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "Method library" }));
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Method library" })).toBeNull();
      expect(screen.queryByRole("tab", { name: "Method library" })).toBeNull();
    });
    expect(screen.getAllByText("Local host ready").length).toBeGreaterThan(0);
    expect(screen.queryByText("Read-only recovery")).toBeNull();
    expect(screen.getByRole("button", { name: "Explorer" })).toBeTruthy();
  });

  it("ignores a stale projection poll that completes after renderer rebind", async () => {
    const { runtime, invoke, releaseOldProjection } = await bmadLibraryRuntime({
      expireFirstSession: true,
      holdOldProjection: true,
    });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={5}
      />,
    );

    await waitFor(() => {
      expect(invoke.mock.calls.some(([command]) => command === "host_projection_events")).toBe(true);
    });
    await user.click(await screen.findByRole("button", { name: "Method library" }));
    expect(await screen.findByText("Create Architecture")).toBeTruthy();

    await act(async () => {
      releaseOldProjection();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.getAllByText("Local host ready").length).toBeGreaterThan(0);
      expect(screen.getByText("Create Architecture")).toBeTruthy();
    });
    expect(screen.queryByText("Host unavailable")).toBeNull();
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
