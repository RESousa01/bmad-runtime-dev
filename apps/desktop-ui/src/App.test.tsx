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
        schemaVersion: "bmad-library-snapshot.v2",
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
        builderPackages: [],
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
  currentIntent = "Help me choose the next Method step",
): BmadHelpRunCreatedProjection {
  return {
    schemaVersion: "bmad-help-run.v1",
    runKind: "bmad_help",
    lifecycle: "created_unbound",
    workspaceId,
    runId: `run_${suffix}`,
    sessionId: `session_${suffix}`,
    currentIntent,
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
  modelStatus = "development_ready",
  workspaces = recoveryBootstrap.workspaces,
}: {
  createOutcome?: "success" | "failure" | "recovery" | "renderer_expired";
  createRun?: BmadHelpRunCreatedProjection;
  holdCreate?: boolean;
  holdLatestWorkspaceId?: string | null;
  latestRuns?: Readonly<Record<string, BmadHelpLatestFixture>>;
  modelStatus?: "development_ready" | "unavailable";
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
      "model.auth.status",
      "model.auth.sign_in",
      "model.auth.sign_out",
      "bmad.help.prepare",
      "bmad.help.approve",
      "bmad.help.cancel",
      "bmad.help.submit",
      "bmad.help.latest",
      "run.create",
      "context.preview",
    ],
    workspaces,
  };
  let request = 0;
  let responseSequence = bootstrap.projectionSequence;
  const bmadReply = (requestId: string, data: unknown) => successfulReply(
    requestId,
    data,
    ++responseSequence,
  );
  let resolveCreateStarted!: () => void;
  let resolveCreate!: () => void;
  let resolveLatest!: () => void;
  const createStarted = new Promise<void>((resolve) => { resolveCreateStarted = resolve; });
  const heldCreate = new Promise<void>((resolve) => { resolveCreate = resolve; });
  const heldLatest = new Promise<void>((resolve) => { resolveLatest = resolve; });
  const createdRuns = new Map<string, BmadHelpRunCreatedProjection>();
  const createdIntents = new Map<string, string>();
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
    if (envelope.command === "model.auth.status") {
      return bmadReply(envelope.requestId, {
        kind: "model_auth_status",
        value: {
          status: modelStatus,
          mode: modelStatus === "development_ready" ? "deterministic_development" : "offline",
          authEpoch: 5,
          developmentOnly: modelStatus === "development_ready",
          destinationLabel: "Deterministic local model",
          signInAvailable: false,
          signOutAvailable: true,
        },
  });
}

    if (envelope.command === "bmad.help.latest") {
      const workspaceId = envelope.payload.workspaceId!;
      if (workspaceId === holdLatestWorkspaceId) {
        await heldLatest;
      }
      const retained = latestRuns[workspaceId] ?? null;
      return bmadReply(
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
                ? "The renderer session expired. BMAD Help was not created."
                : "BMAD Help could not be created. Nothing was changed.",
            retryable: createOutcome === "failure" || createOutcome === "renderer_expired",
            correlationId: envelope.requestId,
          },
        };
      }
      const projection = {
        ...(createRun
          ?? bmadHelpRun(envelope.payload.workspaceId!, "BMad Help", "01K0Q6H4")),
        currentIntent: envelope.payload.currentIntent ?? "",
      };
      createdRuns.set(projection.workspaceId, projection);
      createdIntents.set(projection.workspaceId, envelope.payload.currentIntent ?? "");
      const reply = bmadReply(envelope.requestId, {
        kind: "bmad_help_run_created",
        value: projection,
      });
      return {
        ...reply,
        receipt: { ...reply.receipt, operationId: projection.runId },
      };
    }
    if (envelope.command === "bmad.help.prepare") {
      const workspaceId = envelope.payload.workspaceId!;
      const run = createdRuns.get(workspaceId)
        ?? createRun
        ?? bmadHelpRun(workspaceId, "BMad Help", "01K0Q6H4");
      const currentIntent = createdIntents.get(workspaceId) ?? "Choose the next safe Method step.";
      const outboundByteCount = new TextEncoder().encode(currentIntent).byteLength;
      return bmadReply(envelope.requestId, {
        kind: "bmad_help_review",
        value: {
          workspaceId,
          workspaceGrantEpoch: envelope.payload.workspaceGrantEpoch ?? 1,
          runId: run.runId,
          sessionId: run.sessionId,
          destinationLabel: "Deterministic local model",
          developmentOnly: true,
          consentDisclosure: "Send the exact reviewed context once for this Help request.",
          manifestHash: digestA,
          purpose: "bmad_help",
          region: "localdev",
          retentionMode: "transient_no_store",
          expiresAt: Date.now() + 60_000,
          items: [{
            relativeLabel: "method/current-intent.txt",
            semanticRole: "current_intent",
            language: "text",
            outboundByteCount,
            tokenEstimate: Math.max(1, Math.ceil(outboundByteCount / 4)),
            classification: "internal",
            redactions: [],
            outboundContent: currentIntent,
          }],
          exclusions: [],
          secretFindings: [],
          totalOutboundBytes: outboundByteCount,
          totalTokenEstimate: Math.max(1, Math.ceil(outboundByteCount / 4)),
          redactionLimitation: "Redaction reduces risk but cannot prove every secret was detected.",
        },
      });
    }
    if (envelope.command === "bmad.library.snapshot") {
      return bmadReply(envelope.requestId, {
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
    "Describe what you want skill guidance for",
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

  it("starts on current-product onboarding without seeded demo sessions or effects", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findAllByText("Browser preview");

    expect(screen.getByRole("banner", { name: "Sapphirus application" })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Sidebar" })).toBeTruthy();
    expect(screen.getByRole("complementary", { name: "Task navigation" })).toBeTruthy();
    expect(screen.getByRole("main")).toBeTruthy();
    expect(screen.queryByRole("complementary", { name: /Files|Changes|Run details|Skills and agents/ })).toBeNull();
    expect(screen.getByRole("button", { name: "New task" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "New task" })).toBeTruthy();
    expect(screen.queryByText("Add a safe workspace scan")).toBeNull();
    expect(screen.queryByText("Refactor config loader")).toBeNull();
    expect(screen.queryByText("Demo response")).toBeNull();
    expect(screen.queryByRole("button", { name: "Review changes" })).toBeNull();
    expect(screen.getByRole("button", { name: "Attach files" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Changes" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Run details" })).toBeTruthy();
    // Governed edits stay fail-closed without a real reviewed proposal.
    await user.click(screen.getByRole("button", { name: "Changes" }));
    expect(screen.getByRole("heading", { name: "Changes" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Apply changes" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Revise" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Discard" })).toBeNull();
    expect(screen.getByText("No proposed changes")).toBeTruthy();
    expect(screen.getAllByText("Browser preview").length).toBeGreaterThan(0);
    expect(
      (await screen.findAllByText(/Governed edits require the signed Windows desktop host/i)).length,
    ).toBeGreaterThan(0);

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

    const workspaceTrigger = screen.getByRole("button", { name: /Manage workspace/ });
    await user.click(workspaceTrigger);

    expect(screen.getByRole("dialog", { name: "Local workspaces" })).toBeTruthy();
    const closeButton = screen.getByRole("button", { name: "Close workspaces" });
    expect(document.activeElement).toBe(closeButton);
    expect(document.querySelector(".task-shell-layout__main")?.hasAttribute("inert")).toBe(true);
    expect(screen.getAllByText("bmad-runtime-dev").length).toBeGreaterThan(0);
    expect(screen.getByText(/absolute paths never enter renderer state/i)).toBeTruthy();
    expect((screen.getByRole("button", { name: "Choose local workspace" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByRole("button", { name: "Skills and agents" })).toBeNull();
    expect((screen.getByRole("button", { name: /remove workspace bmad-runtime-dev/i }) as HTMLButtonElement).disabled)
      .toBe(true);

    await user.tab();
    expect(document.activeElement).toBe(closeButton);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "Local workspaces" })).toBeNull();
    expect(document.activeElement).toBe(workspaceTrigger);
  });

  it("keeps responsive task navigation and Files drawer inert while closed and modal while open", async () => {
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
      const taskNavigation = document.querySelector<HTMLElement>(".task-shell-layout__sidebar")!;
      expect(taskNavigation.getAttribute("role")).toBe("dialog");
      expect(taskNavigation.getAttribute("aria-label")).toBe("Task navigation");
      expect(taskNavigation.getAttribute("aria-hidden")).toBe("true");
      expect(taskNavigation.hasAttribute("inert")).toBe(true);

      const navigationTrigger = screen.getByRole("button", { name: "Open task navigation" });
      await user.click(navigationTrigger);
      expect(screen.getByRole("dialog", { name: "Task navigation" })).toBe(taskNavigation);
      expect(taskNavigation.getAttribute("aria-modal")).toBe("true");
      expect(taskNavigation.hasAttribute("inert")).toBe(false);
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close task navigation" }));

      await user.keyboard("{Escape}");
      expect(taskNavigation.getAttribute("aria-hidden")).toBe("true");

      const filesTrigger = screen.getByRole("button", { name: "Attach files" });
      await user.click(filesTrigger);
      const filesDrawer = screen.getByRole("dialog", { name: "Files" });
      expect(filesDrawer.getAttribute("aria-modal")).toBe("true");
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close Files" }));
      expect(screen.getByRole("main", { hidden: true }).closest(".task-shell-layout__main")?.hasAttribute("inert"))
        .toBe(true);
      await user.keyboard("{Escape}");
      expect(screen.queryByRole("dialog", { name: "Files" })).toBeNull();
    } finally {
      view.unmount();
      Object.defineProperty(window, "matchMedia", {
        configurable: true,
        value: originalMatchMedia,
      });
    }
  });

  it("moves focus into the modal settings dialog and returns it on close", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findAllByText("Browser preview");
    const settingsTrigger = screen.getByRole("button", { name: "Settings" });

    await user.click(settingsTrigger);
    const settingsDialog = screen.getByRole("dialog", { name: "Settings" });
    expect(settingsDialog.getAttribute("aria-modal")).toBe("true");
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close settings" }));

    await user.click(screen.getByRole("button", { name: "Close settings" }));
    expect(screen.queryByRole("dialog", { name: "Settings" })).toBeNull();
    expect(document.activeElement).toBe(settingsTrigger);
  });

  it("keeps the agent selector self-contained without a settings shortcut", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Browser preview");

    const agentTrigger = screen.getByRole("button", { name: "Agent and model settings" });
    await user.click(agentTrigger);
    const agentRegion = screen.getByRole("region", { name: "Agent and model" });
    expect(within(agentRegion).getByText("BMAD Help")).toBeTruthy();
    expect(within(agentRegion).getByText("Review before send")).toBeTruthy();
    expect(within(agentRegion).queryByRole("button", { name: "Open settings" })).toBeNull();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: "Agent and model" })).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(agentTrigger));
  });

  it("provides an interactive but unmistakable browser-demo Files drawer", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findAllByText("Browser preview");

    await user.click(screen.getByRole("button", { name: "Attach files" }));
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

    expect(screen.queryByRole("heading", { name: "Files" })).toBeNull();
    expect(screen.getByText("Local context preview")).toBeTruthy();
    expect(screen.getByText("README.md")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Attach files" })).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\/);
  });

  it("returns focus to the task action after closing a side-by-side drawer", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByText("Browser preview");

    const attachFiles = screen.getByRole("button", { name: "Attach files" });
    await user.click(attachFiles);
    await user.click(screen.getByRole("button", { name: "Close Files" }));

    await waitFor(() => expect(document.activeElement).toBe(attachFiles));
  });

  it("renders read surfaces and local context preview only from validated host projections", async () => {
    const { runtime, invoke } = await readyD1Runtime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await user.click(await screen.findByRole("button", { name: "Attach files" }));
    expect(await screen.findByText("Validated local projection")).toBeTruthy();
    expect(screen.queryByText(/Browser demo data/i)).toBeNull();
    await user.click(await screen.findByRole("button", { name: /README\.md/i }));
    expect(await screen.findByLabelText("Read-only preview of README.md")).toHaveProperty(
      "textContent",
      expect.stringContaining("Host projected readme"),
    );
    await user.click(screen.getByRole("checkbox", { name: "Include README.md in context" }));
    await user.click(screen.getByRole("button", { name: "Review context" }));

    expect(screen.queryByRole("heading", { name: "Files" })).toBeNull();
    expect(screen.getByText("Local context preview")).toBeTruthy();
    expect(screen.getByText("README.md")).toBeTruthy();
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
    await user.click(screen.getByRole("button", { name: /Manage workspace/ }));
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
    await user.click(screen.getByRole("button", { name: /Manage workspace/ }));
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
    await user.click(screen.getByRole("button", { name: /Manage workspace/ }));
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

    await user.click(screen.getByRole("button", { name: /Manage workspace/ }));
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
    await user.click(screen.getByRole("button", { name: /Manage workspace/ }));
    expect((screen.getByRole("button", { name: "Choose local workspace" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("opens each task-scoped context surface through its canonical drawer trigger", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findAllByText("Browser preview");
    await user.click(screen.getByRole("button", { name: "Attach files" }));
    expect(screen.getByRole("heading", { name: "Files" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close Files" }));

    await user.click(screen.getByRole("button", { name: "Changes" }));
    expect(screen.getByRole("heading", { name: "Changes" })).toBeTruthy();
    expect(screen.getByText("No proposed changes")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close Changes" }));

    await user.click(screen.getByRole("button", { name: "Run details" }));
    expect(screen.getByRole("heading", { name: "Activity" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "No activity yet" })).toBeTruthy();
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
    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));

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
    const { runtime, invoke, releaseLatest } = await bmadHelpRuntime({
      holdLatestWorkspaceId: workspace.workspaceId,
    });
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

    const composer = await screen.findByLabelText(
      "Describe what you want skill guidance for",
    );
    expect(composer).toHaveProperty("disabled", true);
    expect(
      screen.getByRole("button", { name: "Review request" }),
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
    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));

    const legacyWarning = (await screen.findAllByRole("alert"))[0]!;
    expect(legacyWarning).toHaveProperty(
      "textContent",
      expect.stringContaining("retained BMAD Help session"),
    );
    expect(legacyWarning).toHaveProperty(
      "textContent",
      expect.stringContaining("You can create a new local skill-guidance session"),
    );
    const composer = screen.getByLabelText("Describe what you want skill guidance for");
    expect(composer).toHaveProperty("disabled", false);
    expect(screen.queryByText(/^Created$/)).toBeNull();

    await user.type(composer, "Create a newly retained Method session");
    await user.click(screen.getByRole("button", { name: "Review request" }));

    expect(await screen.findByText("Created · Unbound")).toBeTruthy();
    expect(invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "run.create"))
      .toHaveLength(1);
  });

  it("keeps the composer closed and creates no Help run when model access is unavailable", async () => {
    const { runtime, invoke } = await bmadHelpRuntime({ modelStatus: "unavailable" });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await waitFor(() => {
      const commands = invoke.mock.calls
        .filter(([command]) => command === "host_dispatch")
        .map(([, args]) => (JSON.parse(String(args?.body)) as { command: string }).command);
      expect(commands).toContain("model.auth.status");
    });
    expect(screen.getByRole("textbox")).toHaveProperty("disabled", true);
    await user.click(screen.getByRole("button", { name: "Agent and model settings" }));
    expect(await within(screen.getByRole("region", { name: "Agent and model" })).findByText("Model access unavailable")).toBeTruthy();
    fireEvent.submit(screen.getByRole("form", { name: "Task composer" }));

    const commands = invoke.mock.calls
      .filter(([command]) => command === "host_dispatch")
      .map(([, args]) => (JSON.parse(String(args?.body)) as { command: string }).command);
    expect(commands).not.toContain("run.create");
    expect(commands).not.toContain("bmad.help.prepare");
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
    await user.click(screen.getByRole("button", { name: "Review request" }));

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
    const submit = screen.getByRole("button", { name: "Review request" });
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
    await user.click(screen.getByRole("button", { name: "Review request" }));
    await waitFor(() => {
      const createCalls = invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
        && (JSON.parse(String(args?.body)) as { command: string }).command === "run.create");
      expect(createCalls).toHaveLength(1);
    });
    await user.click(screen.getByRole("button", { name: "Skills and agents" }));

    expect((await screen.findAllByRole("alert"))[0]).toHaveProperty(
      "textContent",
      expect.stringContaining("BMAD Help could not be created. Nothing was changed."),
    );
    expect(screen.queryByText("Created · Unbound")).toBeNull();
    expect(screen.queryByText(/^Created$/)).toBeNull();
    expect(screen.queryByText("Demo response")).toBeNull();
    expect(
      screen.getByLabelText("Describe what you want skill guidance for"),
    ).toHaveProperty("disabled", false);
    expect(screen.getByLabelText("Describe what you want skill guidance for"))
      .toHaveProperty("value", "Recommend a safe next step");
    const createCalls = () => invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "run.create");
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
    await user.click(screen.getByRole("button", { name: "Review request" }));
    expect((await screen.findAllByRole("alert"))[0]).toHaveProperty(
      "textContent",
      expect.stringContaining("The renderer session expired. BMAD Help was not created."),
    );
    expect(screen.getByLabelText("Describe what you want skill guidance for"))
      .toHaveProperty("disabled", false);
    expect(screen.getByLabelText("Describe what you want skill guidance for"))
      .toHaveProperty("value", "Recommend a safe next step");

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
    await user.click(screen.getByRole("button", { name: "Review request" }));
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
      screen.getByLabelText("Describe what you want skill guidance for"),
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
    await user.click(screen.getByRole("button", { name: "Review request" }));

    expect((await screen.findAllByText("Read-only recovery")).length).toBeGreaterThan(0);
    expect(
      screen.getByLabelText("Describe a task"),
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
    await user.click(screen.getByRole("button", { name: /Manage workspace/ }));
    await user.click(screen.getByRole("button", {
      name: "Switch to workspace secondary-workspace",
    }));
    await user.click(screen.getByRole("button", { name: "Close workspaces" }));
    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));
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

    const trigger = await screen.findByRole("button", { name: "Skills and agents" });
    await user.click(trigger);
    expect(await screen.findByRole("heading", { name: "Skills and agents" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Installed skills" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Available actions" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Agents" })).toBeTruthy();
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
    expect(screen.getByRole("heading", { name: "Skills and agents" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Close Skills and agents" })).toBeTruthy();
    const accessibility = await axe.run(container, {
      rules: { "color-contrast": { enabled: false } },
    });
    expect(accessibility.violations).toEqual([]);
  });

  it("does not fabricate a Method library in browser preview or recovery", async () => {
    const browser = render(<App />);
    await screen.findAllByText("Browser preview");
    expect(screen.queryByRole("button", { name: "Skills and agents" })).toBeNull();
    expect(screen.getByLabelText("Describe a task")).toHaveProperty("disabled", true);
    browser.unmount();

    const runtime = await recoveryRuntime();
    render(<App hostRuntimeLoader={async () => runtime} />);
    await screen.findAllByText("Read-only recovery");
    expect(screen.queryByRole("button", { name: "Skills and agents" })).toBeNull();
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

    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));
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

  it("moves focus from Settings into Skills and agents and restores the stable trigger", async () => {
    const { runtime } = await bmadLibraryRuntime();
    const user = userEvent.setup();
    render(<App hostRuntimeLoader={async () => runtime} projectionPollIntervalMs={60_000} />);

    const settingsTrigger = await screen.findByRole("button", { name: "Settings" });
    await user.click(settingsTrigger);
    await user.click(screen.getByRole("button", { name: "Skills & agents" }));
    await user.click(screen.getByRole("button", { name: "Open Skills and agents" }));

    expect(screen.getByRole("heading", { name: "Skills and agents" })).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close Skills and agents" }));
    await user.click(screen.getByRole("button", { name: "Close Skills and agents" }));
    await waitFor(() => expect(document.activeElement).toBe(settingsTrigger));
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

    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));
    await waitFor(() => {
      expect(invoke.mock.calls.filter(([command]) => command === "host_bootstrap")).toHaveLength(2);
      expect(screen.queryByRole("heading", { name: "Skills and agents" })).toBeNull();
    });
    await user.click(screen.getByRole("button", { name: "Skills and agents" }));
    expect(await screen.findByText("Create Architecture")).toBeTruthy();

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

    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Skills and agents" })).toBeNull();
    });
    expect(screen.getAllByText("Local host ready").length).toBeGreaterThan(0);
    expect(screen.queryByText("Read-only recovery")).toBeNull();
    expect(screen.getByRole("button", { name: "Attach files" })).toBeTruthy();
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
    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));
    await waitFor(() => {
      expect(invoke.mock.calls.filter(([command]) => command === "host_bootstrap")).toHaveLength(2);
      expect(screen.queryByRole("heading", { name: "Skills and agents" })).toBeNull();
    });
    await user.click(screen.getByRole("button", { name: "Skills and agents" }));
    expect(await screen.findByText("Create Architecture")).toBeTruthy();

    await act(async () => {
      releaseOldProjection();
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.getAllByText("Local host ready").length).toBeGreaterThan(0);
      expect(screen.getByText("Create Architecture")).toBeTruthy();
    });
    const snapshotCalls = invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "bmad.library.snapshot");
    expect(snapshotCalls).toHaveLength(2);
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

  it("has no automated accessibility violations in the populated Files drawer", async () => {
    const user = userEvent.setup();
    const { container } = render(<App />);
    await screen.findAllByText("Browser preview");
    await user.click(screen.getByRole("button", { name: "Attach files" }));
    await screen.findByRole("button", { name: /README\.md/i });
    const results = await axe.run(container, {
      rules: {
        "color-contrast": { enabled: false },
      },
    });

    expect(results.violations).toEqual([]);
  });
});
