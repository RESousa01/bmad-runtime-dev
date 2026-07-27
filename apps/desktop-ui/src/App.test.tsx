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

function dispatchedCommandNames(invoke: ReturnType<typeof vi.fn<TauriInvoke>>): string[] {
  return invoke.mock.calls.flatMap(([command, args]) => {
    if (command !== "host_dispatch") return [];
    return [(JSON.parse(String(args?.body)) as { command: string }).command];
  });
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
      contextReadEpoch: 1,
      governedEditEpoch: 1,
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

async function reviewedRecoveryRuntime(options: {
  decisionGate?: Promise<void>;
  expiresAt?: number;
  manualReasonCode?: "checkpoint_incomplete_or_inconsistent";
  now?: () => number;
  onDecisionStarted?: () => void;
  onReboundProjectionPoll?: (afterSequence: number) => void;
  projectionGapGate?: Promise<void>;
} = {}): Promise<{
  runtime: HostRuntime;
  invoke: ReturnType<typeof vi.fn<TauriInvoke>>;
}> {
  const bootstrap: BootstrapReply = {
    ...recoveryBootstrap,
    bootMode: "ready",
    supportedCommands: [
      "workspace.enable_edits",
      "changes.propose",
      "approval.decide",
      "rollback.request",
      "changes.history",
      "changes.recovery.prepare",
      "changes.recovery.decide",
    ],
    workspaces: [{
      ...recoveryBootstrap.workspaces[0]!,
      grantEpoch: 8,
      permissions: "governed_edits",
    }],
    projectionSequence: 18,
  };
  let request = 0;
  let sequence = 18;
  let recoveryCompleted = false;
  let bootstrapCount = 0;
  let projectionGapDelivered = false;
  const invoke = vi.fn<TauriInvoke>(async (command, args) => {
    if (command === "host_bootstrap") {
      bootstrapCount += 1;
      return bootstrapCount > 1
        ? {
            ...bootstrap,
            rendererSessionId: "renderer_01K0Q6H3_rebound",
            projectionSequence: sequence,
          }
        : bootstrap;
    }
    if (command === "host_projection_snapshot" && options.projectionGapGate) {
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId: bootstrap.rendererSessionId,
        status: "snapshot",
        snapshot: {
          sequence,
          generatedAt: 1_800_000_000_000,
          bootMode: "ready",
          workspaceCount: bootstrap.workspaces.length,
          activeSessionId: null,
        },
      };
    }
    if (command === "host_projection_events" && options.projectionGapGate) {
      const requestBody = JSON.parse(String(args?.body)) as { afterSequence: number };
      if (!projectionGapDelivered) {
        await options.projectionGapGate;
        projectionGapDelivered = true;
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId: bootstrap.rendererSessionId,
          status: "error",
          error: {
            code: "conflict",
            safeMessage: "The projection cursor expired; request a fresh snapshot.",
            retryable: false,
            correlationId: null,
          },
        };
      }
      options.onReboundProjectionPoll?.(requestBody.afterSequence);
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId: "renderer_01K0Q6H3_rebound",
        status: "events",
        events: [],
      };
    }
    const envelope = JSON.parse(String(args?.body)) as {
      command: string;
      payload: Record<string, unknown>;
      requestId: string;
    };
    sequence += 1;
    if (envelope.command === "changes.history") {
      return successfulReply(envelope.requestId, {
        kind: "changes_history",
        value: {
          workspaceId: bootstrap.workspaces[0]!.workspaceId,
          entries: [],
          openJournals: recoveryCompleted ? [] : [{
            journalId: "journal_01K0Q6H3",
            executionId: "execution_01K0Q6H3",
            state: "recovery_required",
            updatedAt: "2026-07-18T00:00:00Z",
            recoveryAvailability: "review_available",
          }],
        },
      }, sequence);
    }
    if (envelope.command === "changes.recovery.prepare") {
      if (options.manualReasonCode) {
        return successfulReply(envelope.requestId, {
          kind: "changes_recovery_prepared",
          value: {
            status: "manual_review",
            journal_id: "journal_01K0Q6H3",
            execution_id: "execution_01K0Q6H3",
            reason_code: options.manualReasonCode,
          },
        }, sequence);
      }
      return successfulReply(envelope.requestId, {
        kind: "changes_recovery_prepared",
        value: {
          status: "review_required",
          recovery_approval_id: "recovery_approval_01K0Q6H3",
          displayed_recovery_hash: digestA,
          journal_id: "journal_01K0Q6H3",
          execution_id: "execution_01K0Q6H3",
          operations: [{
            relativePath: "src/example.ts",
            operation: "replace",
            explanation: "Restore the file content saved before the interrupted change.",
          }],
          expires_at: options.expiresAt ?? 1_900_000_000_000,
        },
      }, sequence);
    }
    if (envelope.command === "changes.recovery.decide") {
      options.onDecisionStarted?.();
      if (options.decisionGate) await options.decisionGate;
      recoveryCompleted = envelope.payload.choice === "restore";
      return successfulReply(envelope.requestId, {
        kind: "changes_recovery_decision",
        value: {
          recoveryApprovalId: "recovery_approval_01K0Q6H3",
          disposition: envelope.payload.choice === "restore" ? "recovered" : "cancelled",
          journalId: "journal_01K0Q6H3",
          executionId: "execution_01K0Q6H3",
          restoredFiles: envelope.payload.choice === "restore" ? 1 : 0,
        },
      }, sequence);
    }
    throw new Error(`Unexpected command ${envelope.command}`);
  });
  const client = new DesktopHostClient({
    invoke,
    now: options.now ?? (() => 1_800_000_000_000),
    requestId: () => `request_recovery_ui_${request += 1}`,
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
    contextReadEpoch: 1,
    governedEditEpoch: 1,
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
  expireProjectionCursor = false,
  holdOldProjection = false,
  projectionSnapshotBootMode = "ready",
}: {
  dropBmadAfterRebind?: boolean;
  emitInvalidation?: boolean;
  expireFirstSession?: boolean;
  expireProjectionCursor?: boolean;
  holdOldProjection?: boolean;
  projectionSnapshotBootMode?: "ready" | "read_only_recovery";
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
    projectionSequence: expireProjectionCursor ? 20 : bootstrap.projectionSequence,
    supportedCommands: dropBmadAfterRebind
      ? bootstrap.supportedCommands.filter((command) => command !== "bmad.library.snapshot")
      : bootstrap.supportedCommands,
    workspaces: expireProjectionCursor
      ? bootstrap.workspaces.map((workspace) => ({
          ...workspace,
          displayName: "reconciled-workspace",
        }))
      : bootstrap.workspaces,
  };
  let request = 0;
  let bootstrapCount = 0;
  let libraryAttemptCount = 0;
  let librarySnapshotCount = 0;
  let projectionConflictDelivered = false;
  let invalidationDelivered = false;
  let invalidationReleased = false;
  let releaseHeldProjection: () => void = () => undefined;
  const invoke = vi.fn<TauriInvoke>(async (command, args) => {
    if (command === "host_bootstrap") {
      bootstrapCount += 1;
      return (expireFirstSession || expireProjectionCursor) && bootstrapCount > 1
        ? reboundBootstrap
        : bootstrap;
    }
    if (command === "host_projection_snapshot") {
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId: bootstrap.rendererSessionId,
        status: "snapshot",
        snapshot: {
          sequence: 20,
          generatedAt: 1_725_000_000_020,
          bootMode: projectionSnapshotBootMode,
          workspaceCount: bootstrap.workspaces.length,
          activeSessionId: null,
        },
      };
    }
    if (command === "host_projection_events") {
      if (holdOldProjection && bootstrapCount === 1) {
        await new Promise<void>((resolve) => {
          releaseHeldProjection = resolve;
        });
      }
      if (expireProjectionCursor && !projectionConflictDelivered) {
        projectionConflictDelivered = true;
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId: bootstrap.rendererSessionId,
          status: "error",
          error: {
            code: "conflict",
            safeMessage: "The projection cursor expired; request a fresh snapshot.",
            retryable: false,
            correlationId: null,
          },
        };
      }
      const shouldInvalidate = emitInvalidation
        && invalidationReleased
        && librarySnapshotCount > 0
        && !invalidationDelivered;
      invalidationDelivered ||= shouldInvalidate;
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId: (expireFirstSession || expireProjectionCursor) && bootstrapCount > 1
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
    if (envelope.command === "workspace.list") {
      return successfulReply(envelope.requestId, {
        kind: "workspace_list",
        value: expireProjectionCursor
          ? bootstrap.workspaces.map((workspace) => ({
              ...workspace,
              displayName: "reconciled-workspace",
            }))
          : bootstrap.workspaces,
      }, expireProjectionCursor ? 20 : 18);
    }
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
            capabilityId: null,
            availability: "capability_disabled",
            availabilityReason: "bmad_capability_disabled",
          }],
        }],
        builderPackages: [],
        coreSkills: [],
        nextCursor: null,
      },
    }, expireProjectionCursor ? (refreshed ? 21 : 20) : refreshed ? 19 : 18);
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

/// The durable transcript this fake host holds for one conversation.
///
/// `ordinal` is the position of the conversation in the order the renderer
/// first asked about it — 0 for the first task activated, 1 for the next — so a
/// test can seed a specific tab's history without knowing the ULID the renderer
/// minted for it.
type ChatHistoryFixture = (
  sessionRef: string,
  ordinal: number,
) => readonly { role: string; text: string; createdAt: number }[];

async function bmadHelpRuntime({
  chatHistoryFailures = 0,
  chatHistoryFor,
  createOutcome = "success",
  createRun,
  holdChatHistory = false,
  holdCreate = false,
  holdLatestWorkspaceId = null,
  latestRuns = {},
  modelStatus = "development_ready",
  workspaces = recoveryBootstrap.workspaces,
  runnableCapability = false,
}: {
  /// Fails this many `chat.history` reads before answering normally, so a test
  /// can drive the failure surface and then its retry.
  chatHistoryFailures?: number;
  chatHistoryFor?: ChatHistoryFixture;
  createOutcome?: "success" | "failure" | "recovery" | "renderer_expired";
  createRun?: BmadHelpRunCreatedProjection;
  holdChatHistory?: boolean;
  holdCreate?: boolean;
  holdLatestWorkspaceId?: string | null;
  latestRuns?: Readonly<Record<string, BmadHelpLatestFixture>>;
  modelStatus?: "development_ready" | "unavailable";
  workspaces?: BootstrapReply["workspaces"];
  runnableCapability?: boolean;
} = {}): Promise<{
  runtime: HostRuntime;
  invoke: ReturnType<typeof vi.fn<TauriInvoke>>;
  createStarted: Promise<void>;
  chatHistoryStarted: Promise<void>;
  releaseChatHistory: () => void;
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
      "model.selection.set",
      "chat.send",
      "chat.history",
      "bmad.help.prepare",
      "bmad.help.approve",
      "bmad.help.cancel",
      "bmad.help.submit",
      "bmad.help.latest",
      "run.create",
      "context.preview",
      "workspace.pick_files",
      "workspace.pick_folder",
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
  let resolveChatHistoryStarted!: () => void;
  let resolveChatHistory!: () => void;
  const createStarted = new Promise<void>((resolve) => { resolveCreateStarted = resolve; });
  const heldCreate = new Promise<void>((resolve) => { resolveCreate = resolve; });
  const heldLatest = new Promise<void>((resolve) => { resolveLatest = resolve; });
  const chatHistoryStarted = new Promise<void>((resolve) => {
    resolveChatHistoryStarted = resolve;
  });
  const heldChatHistory = new Promise<void>((resolve) => { resolveChatHistory = resolve; });
  /// The order in which the renderer first asked about each conversation.
  const chatHistoryOrder: string[] = [];
  let chatHistoryReads = 0;
  const createdRuns = new Map<string, BmadHelpRunCreatedProjection>();
  const createdIntents = new Map<string, string>();
  let currentModelStatus = modelStatus;
  let currentModelMode = modelStatus === "development_ready"
    ? "deterministic_development"
    : "offline";
  let currentProviderId = modelStatus === "development_ready" ? "forge-local" : "openai";
  let currentModelId = modelStatus === "development_ready" ? "bmad-help-fixture-v1" : "gpt-5-mini";
  let currentAuthEpoch = 5;
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
        apiKey?: string;
        currentIntent?: string;
        decisionId?: string;
        manifestHash?: string;
        modelId?: string;
        providerId?: string;
        contextPaths?: string[];
        relativePaths?: string[];
        workspaceGrantEpoch?: number;
        workspaceId?: string;
        agentCode?: string | null;
        text?: string;
        clientSessionRef?: string;
      };
      requestId: string;
    };
    if (envelope.command === "model.auth.status") {
      return bmadReply(envelope.requestId, {
        kind: "model_auth_status",
        value: {
          status: currentModelStatus,
          mode: currentModelMode,
          authEpoch: currentAuthEpoch,
          developmentOnly: currentModelStatus === "development_ready",
          destinationLabel: currentModelStatus === "development_ready"
            ? currentModelMode === "direct_provider_development"
              ? `OpenAI · ${currentModelId}`
              : "Deterministic local model"
            : `OpenAI · ${currentModelId}`,
          providerId: currentProviderId,
          modelId: currentModelId,
          signInAvailable: currentModelMode !== "deterministic_development",
          signOutAvailable: currentModelMode === "direct_provider_development",
        },
      });
    }

    if (envelope.command === "model.auth.sign_in") {
      currentModelStatus = "development_ready";
      currentModelMode = "direct_provider_development";
      currentProviderId = envelope.payload.providerId ?? "openai";
      currentModelId = envelope.payload.modelId ?? "gpt-5-mini";
      currentAuthEpoch += 1;
      return bmadReply(envelope.requestId, {
        kind: "model_auth_status",
        value: {
          status: currentModelStatus,
          mode: currentModelMode,
          authEpoch: currentAuthEpoch,
          developmentOnly: true,
          destinationLabel: `OpenAI · ${currentModelId}`,
          providerId: currentProviderId,
          modelId: currentModelId,
          signInAvailable: true,
          signOutAvailable: true,
        },
      });
    }

    if (envelope.command === "model.auth.sign_out") {
      currentModelStatus = "unavailable";
      currentModelMode = "offline";
      currentAuthEpoch += 1;
      return bmadReply(envelope.requestId, {
        kind: "model_auth_status",
        value: {
          status: currentModelStatus,
          mode: currentModelMode,
          authEpoch: currentAuthEpoch,
          developmentOnly: false,
          destinationLabel: `OpenAI · ${currentModelId}`,
          providerId: currentProviderId,
          modelId: currentModelId,
          signInAvailable: true,
          signOutAvailable: false,
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
    if (envelope.command === "chat.history") {
      resolveChatHistoryStarted();
      if (holdChatHistory) {
        await heldChatHistory;
      }
      const sessionRef = envelope.payload.clientSessionRef!;
      if (!chatHistoryOrder.includes(sessionRef)) {
        chatHistoryOrder.push(sessionRef);
      }
      chatHistoryReads += 1;
      if (chatHistoryReads <= chatHistoryFailures) {
        return {
          schemaVersion: "desktop-dispatch-reply.v1",
          requestId: envelope.requestId,
          sequence: 19,
          status: "error",
          error: {
            code: "temporarily_unavailable",
            safeMessage: "The stored conversation could not be read.",
            retryable: true,
            correlationId: envelope.requestId,
          },
        };
      }
      return bmadReply(envelope.requestId, {
        kind: "chat_history",
        value: {
          sessionId: "chat_01ARZ3NDEKTSV4RRFFQ69G5FAV",
          agentCode: null,
          messages: chatHistoryFor?.(sessionRef, chatHistoryOrder.indexOf(sessionRef)) ?? [],
        },
      });
    }
    if (envelope.command === "chat.send") {
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
                ? "The renderer session expired."
                : "The message could not be sent. Nothing was changed.",
            retryable: createOutcome === "failure" || createOutcome === "renderer_expired",
            correlationId: envelope.requestId,
          },
        };
      }
      const sentAt = 1_725_000_000_000;
      return bmadReply(envelope.requestId, {
        kind: "chat_turn",
        value: {
          sessionId: "chat_01ARZ3NDEKTSV4RRFFQ69G5FAV",
          agentCode: envelope.payload.agentCode ?? null,
          userMessage: { role: "user", text: envelope.payload.text ?? "", createdAt: sentAt },
          assistantMessage: {
            role: "assistant",
            text: "Deterministic local reply.",
            createdAt: sentAt + 1,
          },
        },
      });
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
      const contextItems = (envelope.payload.contextPaths ?? []).map((relativePath) => ({
        relativeLabel: relativePath,
        semanticRole: "workspace_context",
        language: "text",
        outboundByteCount: 28,
        tokenEstimate: 7,
        classification: "confidential",
        redactions: [],
        outboundContent: "authenticated source content",
      }));
      return bmadReply(envelope.requestId, {
        kind: "bmad_help_review",
        value: {
          workspaceId,
          workspaceGrantEpoch: envelope.payload.workspaceGrantEpoch ?? 1,
          runId: run.runId,
          sessionId: run.sessionId,
          destinationLabel: currentModelMode === "direct_provider_development"
            ? `OpenAI · ${currentModelId}`
            : "Deterministic local model",
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
          }, ...contextItems],
          exclusions: [],
          secretFindings: [],
          totalOutboundBytes: outboundByteCount + (contextItems.length * 28),
          totalTokenEstimate:
            Math.max(1, Math.ceil(outboundByteCount / 4)) + (contextItems.length * 7),
          redactionLimitation: "Redaction reduces risk but cannot prove every secret was detected.",
        },
      });
    }
    if (envelope.command === "workspace.pick_files") {
      return bmadReply(envelope.requestId, {
        kind: "picked_files",
        value: {
          workspaceId: envelope.payload.workspaceId,
          relativePaths: ["src/context.ts"],
          selectedCount: 1,
          rejectedOutsideRoot: 0,
          rejectedUnreadable: 0,
          truncated: false,
        },
      });
    }
    if (envelope.command === "workspace.pick_folder") {
      return bmadReply(envelope.requestId, {
        kind: "picked_files",
        value: {
          workspaceId: envelope.payload.workspaceId,
          relativePaths: ["src/folder/a.ts"],
          selectedCount: 1,
          rejectedOutsideRoot: 0,
          rejectedUnreadable: 0,
          truncated: false,
        },
      });
    }
    if (envelope.command === "context.preview") {
      const relativePaths = envelope.payload.relativePaths ?? [];
      return bmadReply(envelope.requestId, {
        kind: "context_preview",
        value: {
          workspaceId: envelope.payload.workspaceId,
          manifestHash: digestA,
          items: relativePaths.map((relativePath, index) => ({
            relativePath,
            startLine: 1,
            endLine: 1,
            reason: "Selected for this task",
            contentHash: `sha256:${String(index + 1).padStart(64, "0")}`,
            classification: "source",
            redactions: [],
            byteCount: 28,
            estimatedTokens: 7,
            content: "authenticated source content",
          })),
          totalBytes: relativePaths.length * 28,
          estimatedTokens: relativePaths.length * 7,
          modelTarget: null,
        },
      });
    }
    if (envelope.command === "bmad.help.approve") {
      return bmadReply(envelope.requestId, {
        kind: "bmad_help_approved",
        value: {
          manifestHash: envelope.payload.manifestHash ?? digestA,
          decisionId: "decision_01K0Q6H3",
          expiresAt: Date.now() + 30_000,
          sendEligible: true,
        },
      });
    }
    if (envelope.command === "bmad.help.submit") {
      const workspaceId = envelope.payload.workspaceId!;
      const run = createdRuns.get(workspaceId)
        ?? createRun
        ?? bmadHelpRun(workspaceId, "BMad Help", "01K0Q6H4");
      const completedAt = Date.now();
      return bmadReply(envelope.requestId, {
        kind: "bmad_help_run_completed",
        value: {
          schemaVersion: "bmad-help-completed.v1",
          runKind: "bmad_help",
          lifecycle: "completed",
          workspaceId,
          runId: run.runId,
          sessionId: run.sessionId,
          runnable: false,
          completionClaimed: true,
          recommendation: {
            recommendationKind: "recommended_capability",
            displayName: "Create Architecture",
            moduleCode: "bmm",
            skillName: "bmad-architecture",
            action: "create",
            evidenceClass: "user_asserted",
            guidanceRequired: true,
            rationaleSummary: "The reviewed intent asks for architecture guidance.",
            createdAt: completedAt,
          },
          receipt: {
            schemaVersion: "bmad-model-receipt-summary.v1",
            receiptId: "receipt_01K0Q6H3",
            status: "succeeded",
            retentionMode: "transient_no_store",
            region: "localdev",
            inputBytes: 512,
            outputBytes: 256,
            startedAt: completedAt - 100,
            completedAt,
          },
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
          methodAgents: runnableCapability
            ? [{
              moduleCode: "bmm",
              agentCode: "bmad-agent-analyst",
              name: "Mary",
              title: "Business Analyst",
              icon: "A",
              team: "software-development",
              description: "Drafts a bounded product brief.",
              availability: "available",
              blockerCodes: [],
              menus: [{
                code: "CB",
                description: "Create a product brief",
                targetKind: "skill_target",
                displayLabel: "Product Brief",
                capabilityId: "bmm:bmad-product-brief",
                availability: "available",
                availabilityReason: null,
              }],
            }]
            : [],
          builderPackages: [],
          coreSkills: [],
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
    chatHistoryStarted,
    releaseChatHistory: resolveChatHistory,
    releaseCreate: resolveCreate,
    releaseLatest: resolveLatest,
  };
}

/// Every `chat.history` envelope the renderer actually sent, in order.
function chatHistoryRequests(
  invoke: ReturnType<typeof vi.fn<TauriInvoke>>,
): { clientSessionRef: string; workspaceId: string; workspaceGrantEpoch: number }[] {
  return invoke.mock.calls.flatMap(([command, args]) => {
    if (command !== "host_dispatch") return [];
    const envelope = JSON.parse(String(args?.body)) as {
      command: string;
      payload: {
        clientSessionRef: string;
        workspaceId: string;
        workspaceGrantEpoch: number;
      };
    };
    return envelope.command === "chat.history" ? [envelope.payload] : [];
  });
}

async function readyMethodGuidanceComposer(): Promise<HTMLTextAreaElement> {
  const composer = await screen.findByLabelText<HTMLTextAreaElement>(
    "Describe what you want skill guidance for",
  );
  await waitFor(() => expect(composer).toHaveProperty("disabled", false));
  return composer;
}

/// Drives the *explicit* BMAD Help action. Sending an ordinary message no
/// longer invokes the advisor: chat is the composer's default lane, and asking
/// "what should I do next?" is a deliberate choice from the agent menu.
async function askBmadHelp(
  user: ReturnType<typeof userEvent.setup>,
  intent: string,
): Promise<void> {
  const composer = await readyMethodGuidanceComposer();
  await user.type(composer, intent);
  await user.click(screen.getByRole("button", { name: "Agent and model settings" }));
  const menu = screen.getByRole("region", { name: "Agent and model" });
  await user.click(
    within(menu).getByRole("button", { name: /BMAD Help — what should I do next\?/u }),
  );
}

async function enterFirstTask(): Promise<void> {
  // Await before choosing a target: an async hostRuntimeLoader must settle
  // before startNewSession() can bind a workspace, otherwise the click is a
  // no-op and Home stays mounted. Home offers "New task" both in the title
  // strip and in its Recent tasks header, so prefer the latter when present.
  const openTasks = await screen.findAllByRole("button", { name: "New task" });
  const recentTasks = screen
    .queryByRole("heading", { name: "Recent tasks" })
    ?.closest("section");
  const target = recentTasks
    ? within(recentTasks).getByRole("button", { name: "New task" })
    : openTasks[0];
  if (target) fireEvent.click(target);
}

function openWorkspaceFiles(): void {
  fireEvent.click(screen.getByRole("button", { name: "Open app menu" }));
  fireEvent.click(screen.getByRole("button", { name: "Show workspace files" }));
}

describe("Forge desktop workbench", () => {
  it("creates a real task from the v2 New task action even before model access is ready", async () => {
    const { runtime, invoke } = await bmadHelpRuntime({ modelStatus: "unavailable" });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await waitFor(() => {
      expect(dispatchedCommandNames(invoke)).toContain("model.auth.status");
    });
    const recentTasks = screen.getByRole("heading", { name: "Recent tasks" }).closest("section");
    expect(recentTasks).not.toBeNull();
    await user.click(within(recentTasks!).getByRole("button", { name: "New task" }));

    expect(screen.getByRole("heading", { name: "New task" })).toBeTruthy();
    expect(screen.getByLabelText("Describe a task")).toHaveProperty("disabled", false);
    expect(screen.getByRole("button", { name: "Send task" })).toHaveProperty("disabled", true);
    expect(dispatchedCommandNames(invoke)).not.toContain("run.create");
  });

  it("starts a titled task from the home composer and allows a fresh task after completion", async () => {
    const { runtime, createStarted } = await bmadHelpRuntime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    const intent = screen.getByLabelText<HTMLInputElement>("Describe your intent");
    await waitFor(() => expect(intent).toHaveProperty("disabled", false));
    await user.type(intent, "Plan the local prototype");
    await user.click(screen.getByRole("button", { name: "Start task" }));
    await createStarted;

    expect((await screen.findAllByText("Plan the local prototype")).length).toBeGreaterThan(0);
    // The composer opens a conversation, not a BMAD advisory run.
    await screen.findByText("Deterministic local reply.");
    await user.click(screen.getByRole("button", { name: "New task" }));
    expect(screen.getByRole("heading", { name: "New task" })).toBeTruthy();
  });

  it("keeps task creation available while model connection is still required", async () => {
    const { runtime, invoke } = await bmadHelpRuntime({ modelStatus: "unavailable" });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await waitFor(() => {
      expect(dispatchedCommandNames(invoke)).toContain("model.auth.status");
    });
    const intent = screen.getByLabelText<HTMLInputElement>("Describe your intent");
    expect(intent).toHaveProperty("disabled", false);
    await user.type(intent, "Draft a task before connecting OpenAI");
    await user.click(screen.getByRole("button", { name: "Start task" }));
    expect(screen.getByRole("heading", { name: "Draft a task before connecting OpenAI" })).toBeTruthy();
    expect(screen.getByLabelText("Describe a task")).toHaveProperty(
      "value",
      "Draft a task before connecting OpenAI",
    );
    expect(screen.getAllByText(/Connect OpenAI in Settings to send prompts/u).length).toBeGreaterThan(0);
    expect(dispatchedCommandNames(invoke)).not.toContain("run.create");
  });

  it("starts a local draft task from the browser preview", async () => {
    const user = userEvent.setup();
    render(<App />);

    await screen.findByText("Sample data · Read only");
    const intent = screen.getByLabelText<HTMLInputElement>("Describe your intent");
    expect(intent).toHaveProperty("disabled", false);

    await user.type(intent, "Inspect browser task creation");
    await user.click(screen.getByRole("button", { name: "Start task" }));

    expect(screen.getByRole("heading", { name: "Inspect browser task creation" })).toBeTruthy();
    expect(screen.getByLabelText("Describe a task")).toHaveProperty(
      "value",
      "Inspect browser task creation",
    );
    expect(screen.getByLabelText("Describe a task")).toHaveProperty("disabled", false);
    expect(screen.getByRole("button", { name: "Send task" })).toHaveProperty("disabled", true);
  });

  it("connects OpenAI from Settings and sends a prompt without extra review steps", async () => {
    const { runtime, invoke, createStarted } = await bmadHelpRuntime({
      modelStatus: "unavailable",
    });
    const runtimeCredential = ["sk", "test", "runtime", "only"].join("-");
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );

    await waitFor(() => {
      expect(dispatchedCommandNames(invoke)).toContain("model.auth.status");
    });
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("button", { name: "Providers" }));
    await user.type(
      screen.getByLabelText("OpenAI API key"),
      runtimeCredential,
    );
    await user.click(screen.getByRole("button", { name: "Connect OpenAI" }));
    await screen.findByRole("button", { name: "Disconnect OpenAI" });
    expect(document.body.textContent).not.toContain(runtimeCredential);
    await user.click(screen.getByRole("button", { name: "Close settings" }));

    const intent = screen.getByLabelText<HTMLInputElement>("Describe your intent");
    await user.type(intent, "Review the local BMAD prototype");
    await user.click(screen.getByRole("button", { name: "Start task" }));
    await createStarted;

    // Sending is one ordinary chat turn: no advisory run, no second review.
    expect(await screen.findByText("Deterministic local reply.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Skills and agents" }));
    expect(await screen.findByRole("heading", { name: "Skills & agents" })).toBeTruthy();
    expect(await screen.findByText("No agents available.")).toBeTruthy();
    expect(dispatchedCommandNames(invoke)).toContain("bmad.library.snapshot");
    expect(screen.queryByRole("heading", { name: "Request receipt" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Suggested next step" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Review request context" })).toBeNull();
    expect(dispatchedCommandNames(invoke)).toContain("model.auth.sign_in");
    expect(dispatchedCommandNames(invoke)).toContain("chat.send");
    // BMAD is not invoked by an ordinary message.
    expect(dispatchedCommandNames(invoke)).not.toContain("run.create");
    expect(dispatchedCommandNames(invoke)).not.toContain("bmad.help.submit");
    expect(screen.queryByText("Authority profile")).toBeNull();
  });

  it("treats an ordinary message as chat and keeps BMAD entirely optional", async () => {
    const { runtime, invoke } = await bmadHelpRuntime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await enterFirstTask();

    await user.type(await readyMethodGuidanceComposer(), "What does this workspace do?");
    await user.click(screen.getByRole("button", { name: "Send message" }));

    // The question and its answer render as an ordinary conversation.
    expect(await screen.findByText("Deterministic local reply.")).toBeTruthy();
    expect(screen.getByRole("log", { name: "Conversation" })).toBeTruthy();

    // No BMAD concept was invoked to hold a conversation.
    const commands = dispatchedCommandNames(invoke);
    expect(commands).toContain("chat.send");
    expect(commands).not.toContain("run.create");
    expect(commands).not.toContain("bmad.help.prepare");
    expect(commands).not.toContain("bmad.capability.prepare");

    // The send carried no agent: plain chat is the default, not an opt-out.
    const sent = invoke.mock.calls.find(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "chat.send");
    const payload = (JSON.parse(String(sent![1]?.body)) as {
      payload: { agentCode: string | null; text: string };
    }).payload;
    expect(payload.agentCode).toBeNull();
    expect(payload.text).toBe("What does this workspace do?");
  });

  /// A failed send used to clear `pending`, so the question disappeared along
  /// with the answer and the user was left with an error about a message they
  /// could no longer see. That is worst precisely when the failure is worth
  /// retrying — the host now classifies a rate limit or a provider outage as
  /// retryable, and telling someone to send it again while having discarded
  /// what they wrote is advice they cannot take.
  it("keeps the question on screen when the send fails, not just the error", async () => {
    const { runtime } = await bmadHelpRuntime({ createOutcome: "failure" });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await enterFirstTask();

    await user.type(await readyMethodGuidanceComposer(), "What does this workspace do?");
    await user.click(screen.getByRole("button", { name: "Send message" }));

    // The failure is reported...
    expect(
      await screen.findByText(/The message could not be sent\./u),
    ).toBeTruthy();

    // ...and the question survives *in the transcript*. Scoped to the log on
    // purpose: the task tab is titled after its opening message and the
    // composer restores the draft, so an unscoped text query passes whether or
    // not the transcript kept anything — it did, before this was fixed.
    const conversation = screen.getByRole("log", { name: "Conversation" });
    expect(
      within(conversation).getByText("What does this workspace do?"),
    ).toBeTruthy();

    // Nothing is in flight, so the app must not still claim to be working on
    // it. Keeping the question must not keep the spinner.
    expect(within(conversation).queryByText("Thinking…")).toBeNull();

    // And it is back in the composer, so a retryable failure can be retried
    // with one action instead of retyping.
    expect((await readyMethodGuidanceComposer()).value).toBe(
      "What does this workspace do?",
    );
    expect(
      screen.getByRole("button", { name: "Send message" }).hasAttribute("disabled"),
    ).toBe(false);
  });

  it("keeps plain chat reachable in one click so BMAD never becomes mandatory", async () => {
    const { runtime, invoke } = await bmadHelpRuntime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await enterFirstTask();
    await readyMethodGuidanceComposer();

    await user.click(screen.getByRole("button", { name: "Agent and model settings" }));
    const menu = screen.getByRole("region", { name: "Agent and model" });
    await user.click(within(menu).getByRole("button", { name: /No agent — plain chat/u }));

    await user.type(await readyMethodGuidanceComposer(), "Draft the boundaries");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    expect(await screen.findByText("Deterministic local reply.")).toBeTruthy();

    const sent = invoke.mock.calls.find(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "chat.send");
    expect((JSON.parse(String(sent![1]?.body)) as {
      payload: { agentCode: string | null };
    }).payload.agentCode).toBeNull();
  });

  it("keeps a capability run scoped to the task it was started on", async () => {
    const { runtime, invoke } = await bmadHelpRuntime({ runnableCapability: true });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await waitFor(() => {
      expect(dispatchedCommandNames(invoke)).toContain("model.auth.status");
    });
    await enterFirstTask();

    // Start a reviewed capability run on the first task.
    await user.click(screen.getByRole("button", { name: "Skills and agents" }));
    await user.click(await screen.findByRole("button", { name: "Start reviewed run" }));
    expect(
      await screen.findByRole("dialog", { name: /Product Brief capability run/ }),
    ).toBeTruthy();

    // A brand-new task must not inherit the first task's run.
    await user.click(screen.getByRole("button", { name: "New task" }));
    await user.click(screen.getByRole("button", { name: "Skills and agents" }));
    expect(await screen.findByRole("heading", { name: "Skills & agents" })).toBeTruthy();
    expect(
      screen.queryByRole("dialog", { name: /Product Brief capability run/ }),
    ).toBeNull();

    // Returning to the first task restores its run. Tabs render as
    // [first task, second task] in creation order — there is no phantom
    // "New task" tab any more — so index 0 is the task the run was started
    // on (both still carry the default "New task" label).
    const firstTaskTab = screen.getAllByRole("tab", { name: "New task" })[0]!;
    await user.click(firstTaskTab);
    if (screen.queryByRole("heading", { name: "Skills & agents" }) === null) {
      await user.click(screen.getByRole("button", { name: "Skills and agents" }));
    }
    expect(
      await screen.findByRole("dialog", { name: /Product Brief capability run/ }),
    ).toBeTruthy();
  });

  /// Attachments used to stay armed after the send that carried them, so every
  /// later message in the task silently re-sent, re-billed and re-entered the
  /// same files into the reviewed-context manifest. The user consented once and
  /// it happened every turn, with nothing on screen distinguishing "these went
  /// last time" from "these go again".
  it("disarms attachments once the message carrying them has been sent", async () => {
    const { runtime, invoke } = await bmadHelpRuntime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await waitFor(() => {
      expect(dispatchedCommandNames(invoke)).toContain("model.auth.status");
    });
    await enterFirstTask();
    await readyMethodGuidanceComposer();

    await user.click(screen.getByRole("button", { name: "Add files or folder" }));
    await user.click(screen.getByRole("menuitem", { name: "Add files…" }));
    await waitFor(() => {
      expect(dispatchedCommandNames(invoke)).toContain("context.preview");
    });
    expect(await screen.findByText("src/context.ts")).toBeTruthy();

    await user.type(
      await readyMethodGuidanceComposer(),
      "Explain the attached module",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));

    // The attachment is gone from the composer...
    await waitFor(() => {
      expect(screen.queryByText("src/context.ts")).toBeNull();
    });
    // ...and the user is told it was spent rather than it vanishing silently.
    expect(await screen.findByText(/Attach again to include it in the next one/u))
      .toBeTruthy();
  });

  it("sends explicitly attached local context without a second review step", async () => {
    const { runtime, invoke } = await bmadHelpRuntime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await waitFor(() => {
      expect(dispatchedCommandNames(invoke)).toContain("model.auth.status");
    });
    await enterFirstTask();
    await readyMethodGuidanceComposer();

    await user.click(screen.getByRole("button", { name: "Add files or folder" }));
    await user.click(screen.getByRole("menuitem", { name: "Add files…" }));
    await waitFor(() => {
      expect(dispatchedCommandNames(invoke)).toContain("context.preview");
    });
    expect(await screen.findByText("src/context.ts")).toBeTruthy();

    await askBmadHelp(user, "Explain the selected module");

    expect(await screen.findByText(/Create Architecture:/)).toBeTruthy();
    expect(screen.queryByRole("heading", {
      name: "Send attached context?",
    })).toBeNull();
    await waitFor(() => {
      expect(dispatchedCommandNames(invoke)).toContain("bmad.help.submit");
    });
    const prepareCall = invoke.mock.calls.find(([command, args]) =>
      command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "bmad.help.prepare"
    );
    expect(JSON.parse(String(prepareCall?.[1]?.body))).toMatchObject({
      payload: { contextPaths: ["src/context.ts"] },
    });
  });

  it("opens a separate task tab from the title-bar plus action", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Home starts with no tasks at all. The shell used to seed a phantom
    // "New task" tab that was not a real record — it could not be renamed,
    // closed, or deleted — which is what made every task look unnamed.
    const openTab = () =>
      user.click(
        within(screen.getByRole("banner", { name: "Forge application" }))
          .getByRole("button", { name: "New task" }),
      );
    expect(screen.queryAllByRole("tab")).toHaveLength(0);

    await openTab();
    expect(screen.getAllByRole("tab", { name: "New task" })).toHaveLength(1);

    await openTab();
    const tabs = screen.getAllByRole("tab", { name: "New task" });
    expect(tabs).toHaveLength(2);
    expect(tabs[0]?.getAttribute("aria-selected")).toBe("false");
    expect(tabs[1]?.getAttribute("aria-selected")).toBe("true");
  });

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
    await enterFirstTask();

    await screen.findAllByText("Browser preview");

    expect(screen.getByRole("banner", { name: "Forge application" })).toBeTruthy();
    expect(screen.getByRole("banner", { name: "Forge application" })).toBeTruthy();
    expect(screen.getByRole("main")).toBeTruthy();
    expect(screen.queryByRole("complementary", { name: /Files|Review|Run details|Skills and agents/ })).toBeNull();
    expect(screen.getByRole("button", { name: "New task" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "New task" })).toBeTruthy();
    expect(screen.queryByText("Add a safe workspace scan")).toBeNull();
    expect(screen.queryByText("Refactor config loader")).toBeNull();
    expect(screen.queryByText("Demo response")).toBeNull();
    expect(screen.queryByRole("button", { name: "Review changes" })).toBeNull();
    expect(screen.getByRole("button", { name: "Add files or folder" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Review" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Run details" })).toBeTruthy();
    // Governed edits stay fail-closed without a real reviewed proposal.
    await user.click(screen.getByRole("button", { name: "Review" }));
    expect(screen.getByRole("heading", { name: "Review" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Working tree" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Apply changes" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Revise" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Discard" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Agent proposals" }));
    expect(screen.getByText("No proposed changes")).toBeTruthy();
    expect(screen.getAllByText("Browser preview").length).toBeGreaterThan(0);
    expect(
      (await screen.findAllByText(/Governed edits require the Windows desktop host/i)).length,
    ).toBeGreaterThan(0);

    expect(screen.queryByText(/^Chat$/i)).toBeNull();
    expect(screen.queryByText(/^Command$/i)).toBeNull();
    expect(screen.queryByText(/Approve & apply locally/i)).toBeNull();
    expect(screen.queryByText(/^Execute$/i)).toBeNull();
    expect(screen.queryByRole("button", { name: /^Auto$/i })).toBeNull();
    expect(screen.queryByText(/^Connected$/i)).toBeNull();
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\/);
  });

  it("offers native folder selection from Workspaces but keeps browser QA inert", async () => {
    const user = userEvent.setup();
    render(<App />);
    await enterFirstTask();
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
    await enterFirstTask();
    try {
      await screen.findAllByText("Browser preview");
      expect(document.querySelector(".task-shell-layout__sidebar")).toBeNull();

      openWorkspaceFiles();
      const filesDrawer = screen.getByRole("dialog", { name: "Files" });
      expect(filesDrawer.getAttribute("aria-modal")).toBe("true");
      // The focus trap takes the drawer's *first* focusable node in document
      // order, which is the leading panel-view tab; the close control remains
      // reachable from there.
      //
      // Naming the element matters. `filesDrawer.contains(activeElement)` is
      // true for every control in the drawer, so it held even when the trap
      // took the last node instead of the first — which is precisely the
      // failure mode `focusableElements` sorts against.
      expect(document.activeElement).toBe(
        within(filesDrawer).getByRole("button", { name: "Files" }),
      );
      expect(screen.getByRole("button", { name: "Close Files" })).toBeTruthy();
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
    await enterFirstTask();
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

  it("keeps the compact agent selector focused on Agent and Model", async () => {
    const user = userEvent.setup();
    render(<App />);
    await enterFirstTask();
    await screen.findByText("Browser preview");

    const agentTrigger = screen.getByRole("button", { name: "Agent and model settings" });
    await user.click(agentTrigger);
    const agentRegion = screen.getByRole("region", { name: "Agent and model" });
    expect(within(agentRegion).getByText("No agent — plain chat")).toBeTruthy();
    expect(within(agentRegion).getByText("No model access")).toBeTruthy();
    expect(within(agentRegion).queryByText("Review before send")).toBeNull();
    expect(within(agentRegion).queryByText("Loading installed agents…")).toBeNull();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: "Agent and model" })).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(agentTrigger));
  });

  it("provides an interactive but unmistakable browser-demo Files drawer", async () => {
    const user = userEvent.setup();
    render(<App />);
    await enterFirstTask();
    await screen.findAllByText("Browser preview");

    openWorkspaceFiles();
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
    expect(screen.getByRole("heading", { name: "Context window" })).toBeTruthy();
    expect(screen.getByText("README.md")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add files or folder" })).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/[A-Z]:\\/);
  });

  it("returns focus to the task action after closing a side-by-side drawer", async () => {
    const user = userEvent.setup();
    render(<App />);
    await enterFirstTask();
    await screen.findByText("Browser preview");

    const reviewTrigger = screen.getByRole("button", { name: "Review" });
    await user.click(reviewTrigger);
    await user.click(screen.getByRole("button", { name: "Files" }));
    await user.click(screen.getByRole("button", { name: "Close Files" }));

    await waitFor(() => expect(document.activeElement).toBe(reviewTrigger));
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
    await enterFirstTask();

    openWorkspaceFiles();
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
    expect(screen.getByRole("heading", { name: "Context window" })).toBeTruthy();
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
    await enterFirstTask();
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
    await enterFirstTask();
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
    await enterFirstTask();
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
    await enterFirstTask();

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
            contextReadEpoch: 1,
            governedEditEpoch: 1,
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
    await enterFirstTask();

    expect(await screen.findByRole("status")).toHaveProperty(
      "textContent",
      expect.stringContaining("Read-only recovery"),
    );
    expect((await screen.findAllByText("refreshed-workspace")).length).toBeGreaterThan(0);

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /Manage workspace/ }));
    expect((screen.getByRole("button", { name: "Choose local workspace" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("resynchronizes an expired projection cursor from an authoritative snapshot", async () => {
    const readyBootstrap: BootstrapReply = {
      ...recoveryBootstrap,
      bootMode: "ready",
      projectionSequence: 18,
    };
    const reboundBootstrap: BootstrapReply = {
      ...readyBootstrap,
      rendererSessionId: "renderer_01K0Q9N4_rebound",
      projectionSequence: 24,
    };
    const eventCursors: number[] = [];
    let bootstrapCount = 0;
    let projectionAttempt = 0;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        bootstrapCount += 1;
        return bootstrapCount > 1 ? reboundBootstrap : readyBootstrap;
      }
      if (command === "host_projection_events") {
        const request = JSON.parse(String(args?.body)) as { afterSequence: number };
        eventCursors.push(request.afterSequence);
        projectionAttempt += 1;
        if (projectionAttempt === 1) {
          return {
            schemaVersion: "desktop-projection-reply.v1",
            rendererSessionId: readyBootstrap.rendererSessionId,
            status: "error",
            error: {
              code: "conflict",
              safeMessage: "The projection cursor expired; request a fresh snapshot.",
              retryable: false,
              correlationId: null,
            },
          };
        }
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId: bootstrapCount > 1
            ? reboundBootstrap.rendererSessionId
            : readyBootstrap.rendererSessionId,
          status: "events",
          events: [],
        };
      }
      if (command === "host_projection_snapshot") {
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId: readyBootstrap.rendererSessionId,
          status: "snapshot",
          snapshot: {
            sequence: 24,
            generatedAt: 1_725_000_000_024,
            bootMode: "ready",
            workspaceCount: readyBootstrap.workspaces.length,
            activeSessionId: null,
          },
        };
      }
      if (command === "host_dispatch") {
        const envelope = JSON.parse(String(args?.body)) as {
          command: string;
          requestId: string;
        };
        if (envelope.command === "workspace.list") {
          return successfulReply(envelope.requestId, {
            kind: "workspace_list",
            value: readyBootstrap.workspaces,
          }, 24);
        }
      }
      throw new Error(`Unexpected command ${command}`);
    });
    const client = new DesktopHostClient({ invoke });
    await client.bootstrap();
    const runtime: HostRuntime = { kind: "ready", client, bootstrap: readyBootstrap };

    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={5}
      />,
    );

    await waitFor(() => {
      expect(invoke.mock.calls.filter(([command]) => command === "host_projection_snapshot"))
        .toHaveLength(1);
    });
    await waitFor(() => {
      expect(invoke.mock.calls.filter(([command]) => command === "host_bootstrap"))
        .toHaveLength(2);
    });
    await waitFor(() => expect(eventCursors).toContain(24));
    expect(screen.queryByText("Host unavailable")).toBeNull();
  });

  it("reconciles workspace and requested Method projections after cursor expiry", async () => {
    const { runtime, invoke } = await bmadLibraryRuntime({
      expireProjectionCursor: true,
    });

    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={5}
      />,
    );

    await waitFor(() => {
      expect(invoke.mock.calls.filter(([command]) => command === "host_projection_snapshot"))
        .toHaveLength(1);
    });
    await waitFor(() => {
      expect(invoke.mock.calls.filter(([command]) => command === "host_bootstrap"))
        .toHaveLength(2);
    });
    expect(dispatchedCommandNames(invoke).filter((command) => command === "workspace.list"))
      .toHaveLength(0);
    await waitFor(() => {
      expect(dispatchedCommandNames(invoke).filter((command) => command === "bmad.library.snapshot"))
        .toHaveLength(2);
    });
    await enterFirstTask();
    expect((await screen.findAllByText("reconciled-workspace")).length).toBeGreaterThan(0);
    expect(screen.queryByText("Host unavailable")).toBeNull();
  });

  it("enters read-only recovery without rebinding when the gap snapshot requires it", async () => {
    const { runtime, invoke } = await bmadLibraryRuntime({
      expireProjectionCursor: true,
      projectionSnapshotBootMode: "read_only_recovery",
    });
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={5}
      />,
    );
    await enterFirstTask();

    expect(await screen.findByRole("status")).toHaveProperty(
      "textContent",
      expect.stringContaining("Read-only recovery"),
    );
    expect(invoke.mock.calls.filter(([command]) => command === "host_bootstrap"))
      .toHaveLength(1);
    expect(screen.queryByText("Host unavailable")).toBeNull();
  });

  it("resets governed-change projections before polling resumes after cursor expiry", async () => {
    let releaseProjectionGap!: () => void;
    const projectionGapGate = new Promise<void>((resolve) => {
      releaseProjectionGap = resolve;
    });
    let reboundPollCursor: number | null = null;
    let recoveryVisibleAtReboundPoll: boolean | null = null;
    const { runtime, invoke } = await reviewedRecoveryRuntime({
      projectionGapGate,
      onReboundProjectionPoll: (afterSequence) => {
        reboundPollCursor = afterSequence;
        recoveryVisibleAtReboundPoll = screen.queryByRole("heading", {
          name: "Review checkpoint recovery",
        }) !== null;
      },
    });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={5}
      />,
    );
    await enterFirstTask();
    await user.click(screen.getByRole("button", { name: "Review" }));
    await user.click(screen.getByRole("button", { name: "Agent proposals" }));
    await user.click(screen.getByRole("button", { name: "Refresh history" }));
    await user.click(await screen.findByRole("button", { name: "Review recovery" }));
    expect(await screen.findByRole("heading", {
      name: "Review checkpoint recovery",
    })).toBeTruthy();

    await act(async () => {
      releaseProjectionGap();
      await projectionGapGate;
    });

    await waitFor(() => expect(reboundPollCursor).not.toBeNull());
    expect(recoveryVisibleAtReboundPoll).toBe(false);
    expect(screen.queryByRole("heading", {
      name: "Review checkpoint recovery",
    })).toBeNull();
    expect(invoke.mock.calls.filter(([command]) => command === "host_bootstrap"))
      .toHaveLength(2);
    expect(screen.queryByText("Host unavailable")).toBeNull();
  });

  it("opens each task-scoped context surface through its canonical drawer trigger", async () => {
    const user = userEvent.setup();
    render(<App />);
    await enterFirstTask();

    await screen.findAllByText("Browser preview");
    openWorkspaceFiles();
    expect(screen.getByRole("heading", { name: "Files" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close Files" }));

    await user.click(screen.getByRole("button", { name: "Review" }));
    expect(screen.getByRole("heading", { name: "Review" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Agent proposals" }));
    expect(screen.getByText("No proposed changes")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close Review" }));

    await user.click(screen.getByRole("button", { name: "Run details" }));
    expect(screen.getByRole("heading", { name: "Activity" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "No activity yet" })).toBeTruthy();
  });

  it("labels local context estimates without inventing provider cost", async () => {
    const user = userEvent.setup();
    render(<App />);
    await enterFirstTask();
    await screen.findAllByText("Browser preview");

    await user.click(screen.getByRole("button", { name: "Open context details" }));

    expect(screen.getByText("Local Review Budget")).toBeTruthy();
    expect(screen.getByText("Input Estimate")).toBeTruthy();
    expect(screen.queryByText("Context Limit")).toBeNull();
    // This test is named "without inventing provider cost" and used to pin
    // `forge-bmad-preview` — an invented model name, for a browser session that
    // cannot contact a model at all. Naming one told the reader a model had been
    // involved. Absent renders as "—", which is what is true here.
    expect(screen.getByText("Model").parentElement?.textContent).toBe("Model—");
    expect(screen.getByText("Total Cost").parentElement?.textContent).toBe("Total Cost—");
    expect(screen.queryByText("$0.00")).toBeNull();
  });

  /// Browser preview pushed a fabricated two-message exchange into every new
  /// task, so a session that had never sent anything displayed a conversation,
  /// a provider, a model and a token count. A reader concluded a model had been
  /// contacted and billed.
  it("shows no invented conversation for a browser session that sent nothing", async () => {
    const user = userEvent.setup();
    render(<App />);
    await enterFirstTask();
    await screen.findAllByText("Browser preview");

    await user.click(screen.getByRole("button", { name: "Open context details" }));

    expect(
      screen.queryByText(/Review this workspace and identify the next BMAD milestone/u),
    ).toBeNull();
    expect(screen.queryByText("forge-bmad-preview")).toBeNull();
    expect(screen.queryByText("1,842")).toBeNull();
  });

  it("shows the actual host model ID in session context", async () => {
    const { runtime, invoke } = await bmadHelpRuntime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await waitFor(() => {
      expect(dispatchedCommandNames(invoke)).toContain("model.auth.status");
    });
    await enterFirstTask();
    await user.click(screen.getByRole("button", { name: "Open context details" }));

    expect(screen.getByText("Model").parentElement?.textContent)
      .toBe("Modelbmad-help-fixture-v1");
    expect(screen.queryByText("Deterministic local model")).toBeNull();
  });

  it("owns one shared recovery review across Changes and Activity and refreshes after cancel", async () => {
    const { runtime, invoke } = await reviewedRecoveryRuntime();
    const user = userEvent.setup();
    render(<App hostRuntimeLoader={async () => runtime} projectionPollIntervalMs={60_000} />);
    await enterFirstTask();
    await screen.findAllByText("opaque-workspace-name");

    await user.click(screen.getByRole("button", { name: "Review" }));
    await user.click(screen.getByRole("button", { name: "Agent proposals" }));
    await user.click(screen.getByRole("button", { name: "Refresh history" }));
    const changesTrigger = await screen.findByRole("button", { name: "Review recovery" });
    await user.click(changesTrigger);
    expect(await screen.findByRole("heading", { name: "Review checkpoint recovery" })).toBeTruthy();
    expect(document.body.textContent).not.toContain(digestA);
    await user.dblClick(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("heading", {
      name: "Review checkpoint recovery",
    })).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(changesTrigger));

    await user.click(screen.getByRole("button", { name: "Close Review" }));
    await user.click(screen.getByRole("button", { name: "Run details" }));
    const activityTrigger = await screen.findByRole("button", { name: "Review recovery" });
    await user.click(activityTrigger);
    expect(await screen.findByRole("heading", { name: "Review checkpoint recovery" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Refresh activity" }));
    await waitFor(() => expect(screen.queryByRole("heading", {
      name: "Review checkpoint recovery",
    })).toBeNull());
    expect(screen.queryByRole("button", { name: "Restore checkpoint" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();

    const envelopes = invoke.mock.calls
      .filter(([command]) => command === "host_dispatch")
      .map(([, args]) => JSON.parse(String(args?.body)) as {
        command: string;
        payload: Record<string, unknown>;
      });
    expect(envelopes.filter(({ command }) => command === "changes.recovery.prepare"))
      .toEqual([
        expect.objectContaining({
          payload: {
            workspaceId: "workspace_01K0Q6H3",
            workspaceGrantEpoch: 8,
            journalId: "journal_01K0Q6H3",
          },
        }),
        expect.objectContaining({
          payload: {
            workspaceId: "workspace_01K0Q6H3",
            workspaceGrantEpoch: 8,
            journalId: "journal_01K0Q6H3",
          },
        }),
      ]);
    expect(envelopes.filter(({ command }) => command === "changes.recovery.decide"))
      .toHaveLength(1);
    expect(envelopes.filter(({ command }) => command === "changes.history").length)
      .toBeGreaterThanOrEqual(2);
  });

  it("removes an expired recovery review without dispatching a decision", async () => {
    const { runtime, invoke } = await reviewedRecoveryRuntime({
      expiresAt: Date.now() + 2_000,
      now: () => Date.now(),
    });
    const user = userEvent.setup();
    render(<App hostRuntimeLoader={async () => runtime} projectionPollIntervalMs={60_000} />);
    await enterFirstTask();
    await screen.findAllByText("opaque-workspace-name");
    await user.click(screen.getByRole("button", { name: "Review" }));
    await user.click(screen.getByRole("button", { name: "Agent proposals" }));
    await user.click(screen.getByRole("button", { name: "Refresh history" }));
    await user.click(await screen.findByRole("button", { name: "Review recovery" }));
    expect(await screen.findByRole("heading", { name: "Review checkpoint recovery" })).toBeTruthy();

    await waitFor(() => expect(screen.queryByRole("heading", {
      name: "Review checkpoint recovery",
    })).toBeNull(), { timeout: 4_000 });
    expect(screen.queryByRole("button", { name: "Restore checkpoint" })).toBeNull();
    const decisions = invoke.mock.calls.filter(([, args]) => {
      if (!args?.body) return false;
      return (JSON.parse(String(args.body)) as { command: string }).command
        === "changes.recovery.decide";
    });
    expect(decisions).toHaveLength(0);
  });

  it("preserves an in-flight recovery decision across review expiry", async () => {
    let releaseDecision!: () => void;
    let markDecisionStarted!: () => void;
    const decisionGate = new Promise<void>((resolve) => { releaseDecision = resolve; });
    const decisionStarted = new Promise<void>((resolve) => { markDecisionStarted = resolve; });
    const { runtime, invoke } = await reviewedRecoveryRuntime({
      decisionGate,
      expiresAt: Date.now() + 1_500,
      now: () => Date.now(),
      onDecisionStarted: markDecisionStarted,
    });
    const user = userEvent.setup();
    render(<App hostRuntimeLoader={async () => runtime} projectionPollIntervalMs={60_000} />);
    await enterFirstTask();
    await screen.findAllByText("opaque-workspace-name");
    await user.click(screen.getByRole("button", { name: "Review" }));
    await user.click(screen.getByRole("button", { name: "Agent proposals" }));
    await user.click(screen.getByRole("button", { name: "Refresh history" }));
    await user.click(await screen.findByRole("button", { name: "Review recovery" }));
    await user.click(await screen.findByRole("button", { name: "Restore checkpoint" }));
    await decisionStarted;

    await new Promise((resolve) => window.setTimeout(resolve, 1_700));
    expect(screen.getByRole("heading", { name: "Review checkpoint recovery" })).toBeTruthy();
    const restore = screen.getByRole<HTMLButtonElement>("button", { name: "Restore checkpoint" });
    expect(restore.disabled).toBe(true);
    await user.click(restore);
    const dispatchCommands = () => invoke.mock.calls
      .filter(([command, args]) => command === "host_dispatch" && args?.body)
      .map(([, args]) => (JSON.parse(String(args?.body)) as { command: string }).command);
    expect(dispatchCommands().filter((command) => command === "changes.recovery.prepare"))
      .toHaveLength(1);
    expect(dispatchCommands().filter((command) => command === "changes.recovery.decide"))
      .toHaveLength(1);

    releaseDecision();
    await waitFor(() => expect(screen.queryByRole("heading", {
      name: "Review checkpoint recovery",
    })).toBeNull());
    await waitFor(() => expect(
      dispatchCommands().filter((command) => command === "changes.history"),
    ).toHaveLength(2));
    expect(screen.queryByRole("button", { name: "Review recovery" })).toBeNull();
  });

  it("guards history refresh while a recovery decision is in flight", async () => {
    let releaseDecision!: () => void;
    let markDecisionStarted!: () => void;
    const decisionGate = new Promise<void>((resolve) => { releaseDecision = resolve; });
    const decisionStarted = new Promise<void>((resolve) => { markDecisionStarted = resolve; });
    const { runtime, invoke } = await reviewedRecoveryRuntime({
      decisionGate,
      onDecisionStarted: markDecisionStarted,
    });
    const user = userEvent.setup();
    render(<App hostRuntimeLoader={async () => runtime} projectionPollIntervalMs={60_000} />);
    await enterFirstTask();
    await screen.findAllByText("opaque-workspace-name");
    await user.click(screen.getByRole("button", { name: "Review" }));
    await user.click(screen.getByRole("button", { name: "Agent proposals" }));
    await user.click(screen.getByRole("button", { name: "Refresh history" }));
    await user.click(await screen.findByRole("button", { name: "Review recovery" }));
    await user.click(await screen.findByRole("button", { name: "Restore checkpoint" }));
    await decisionStarted;

    const dispatchCommands = () => invoke.mock.calls
      .filter(([command, args]) => command === "host_dispatch" && args?.body)
      .map(([, args]) => (JSON.parse(String(args?.body)) as { command: string }).command);
    const changesRefresh = screen.getByRole<HTMLButtonElement>("button", {
      name: "Refresh history",
    });
    await waitFor(() => expect(changesRefresh.disabled).toBe(true));
    fireEvent.click(changesRefresh);

    await user.click(screen.getByRole("button", { name: "Close Review" }));
    await user.click(screen.getByRole("button", { name: "Run details" }));
    const activityRefresh = screen.getByRole<HTMLButtonElement>("button", {
      name: "Refresh activity",
    });
    expect(activityRefresh.disabled).toBe(true);
    fireEvent.click(activityRefresh);
    fireEvent.click(screen.getByRole("button", { name: "Restore checkpoint" }));

    expect(dispatchCommands().filter((command) => command === "changes.history"))
      .toHaveLength(1);
    expect(dispatchCommands().filter((command) => command === "changes.recovery.prepare"))
      .toHaveLength(1);
    expect(dispatchCommands().filter((command) => command === "changes.recovery.decide"))
      .toHaveLength(1);

    releaseDecision();
    await waitFor(() => expect(
      dispatchCommands().filter((command) => command === "changes.history"),
    ).toHaveLength(2));
    expect(screen.queryByRole("heading", { name: "Review checkpoint recovery" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Review recovery" })).toBeNull();
  });

  it("renders owned manual-review copy for a closed native reason code", async () => {
    const { runtime } = await reviewedRecoveryRuntime({
      manualReasonCode: "checkpoint_incomplete_or_inconsistent",
    });
    const user = userEvent.setup();
    render(<App hostRuntimeLoader={async () => runtime} projectionPollIntervalMs={60_000} />);
    await enterFirstTask();
    await screen.findAllByText("opaque-workspace-name");
    await user.click(screen.getByRole("button", { name: "Review" }));
    await user.click(screen.getByRole("button", { name: "Agent proposals" }));
    await user.click(screen.getByRole("button", { name: "Refresh history" }));
    await user.click(await screen.findByRole("button", { name: "Review recovery" }));
    expect(await screen.findByText(
      "Recovery cannot continue automatically because the durable checkpoint is incomplete or inconsistent. Keep this journal quarantined for manual review.",
    )).toBeTruthy();
    expect(document.body.textContent).not.toContain("checkpoint_incomplete_or_inconsistent");
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
    await enterFirstTask();

    await waitFor(() => {
      const latestCalls = invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
        && (JSON.parse(String(args?.body)) as { command: string }).command === "bmad.help.latest");
      expect(latestCalls).toHaveLength(1);
    });
    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));

    expect(screen.getByText("Created · Unbound")).toBeTruthy();
    expect(screen.queryByText("Restored Method guidance")).toBeNull();
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
    await enterFirstTask();

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
      screen.getByRole("button", { name: "Send message" }),
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
    await enterFirstTask();

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

    await askBmadHelp(user, "Create a newly retained Method session");

    expect(await screen.findByText(/Create Architecture:/)).toBeTruthy();
    expect(invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "run.create"))
      .toHaveLength(1);
  });

  it("keeps offline drafting open but creates no Help run when model access is unavailable", async () => {
    const { runtime, invoke } = await bmadHelpRuntime({ modelStatus: "unavailable" });
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await enterFirstTask();

    await waitFor(() => {
      const commands = invoke.mock.calls
        .filter(([command]) => command === "host_dispatch")
        .map(([, args]) => (JSON.parse(String(args?.body)) as { command: string }).command);
      expect(commands).toContain("model.auth.status");
    });
    expect(screen.getByRole("textbox")).toHaveProperty("disabled", false);
    expect(screen.getByRole("button", { name: "Send task" })).toHaveProperty("disabled", true);
    await user.click(screen.getByRole("button", { name: "Agent and model settings" }));
    const modelPanel = screen.getByRole("region", { name: "Agent and model" });
    expect(await within(modelPanel).findByText("Connect a provider")).toBeTruthy();
    await user.click(within(modelPanel).getByRole("button", { name: "Open model settings" }));
    expect(screen.getByRole("dialog", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Providers" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Close settings" }));
    fireEvent.submit(screen.getByRole("form", { name: "Task composer" }));

    const commands = invoke.mock.calls
      .filter(([command]) => command === "host_dispatch")
      .map(([, args]) => (JSON.parse(String(args?.body)) as { command: string }).command);
    expect(commands).not.toContain("run.create");
    expect(commands).not.toContain("bmad.help.prepare");
  });

  it("sends one Help message from the exact submitted intent", async () => {
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
    await enterFirstTask();

    await askBmadHelp(user, "  Choose the next safe architecture step.  ");

    expect(await screen.findByText(/Create Architecture:/)).toBeTruthy();
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

  it("suppresses a duplicate chat send while the first request is pending", async () => {
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
    await enterFirstTask();

    await user.type(
      await readyMethodGuidanceComposer(),
      "Recommend one safe Method step",
    );
    const submit = screen.getByRole("button", { name: "Send message" });
    fireEvent.click(submit);
    fireEvent.click(submit);
    await createStarted;

    // A double-click must not send the same message twice: the host's strict
    // user->assistant alternation would reject the second and lose it.
    const sendCalls = () => invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
      && (JSON.parse(String(args?.body)) as { command: string }).command === "chat.send");
    expect(sendCalls()).toHaveLength(1);
    expect(submit).toHaveProperty("disabled", true);

    releaseCreate();
    expect(await screen.findByText("Deterministic local reply.")).toBeTruthy();
    expect(sendCalls()).toHaveLength(1);
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
    await enterFirstTask();

    await askBmadHelp(user, "Recommend a safe next step");
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
    await enterFirstTask();

    await askBmadHelp(user, "Recommend a safe next step");
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
    await enterFirstTask();

    await askBmadHelp(user, "Recommend a safe next step");
    await oldHost.createStarted;

    view.rerender(
      <App
        hostRuntimeLoader={async () => currentHost.runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await enterFirstTask();
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
    await enterFirstTask();

    await askBmadHelp(user, "Recommend a safe next step");

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
      contextReadEpoch: 1,
      governedEditEpoch: 1,
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
    await enterFirstTask();

    await screen.findAllByText("primary-workspace");
    await user.click(screen.getByRole("button", { name: /Manage workspace/ }));
    await user.click(screen.getByRole("button", {
      name: "Switch to workspace secondary-workspace",
    }));
    await user.click(screen.getByRole("button", { name: "Close workspaces" }));
    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));
    expect(screen.getByText("Created · Unbound")).toBeTruthy();

    await act(async () => {
      releaseLatest();
      await Promise.resolve();
    });
    expect(screen.getByText("Created · Unbound")).toBeTruthy();
    expect(screen.queryByText("Stale primary guidance")).toBeNull();
  });

  it("preloads the native Method library for the composer agent control", async () => {
    const { runtime, invoke } = await bmadLibraryRuntime();
    const user = userEvent.setup();
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await enterFirstTask();

    await waitFor(() => {
      const snapshotCalls = invoke.mock.calls.filter(([command, args]) => command === "host_dispatch"
        && (JSON.parse(String(args?.body)) as { command: string }).command === "bmad.library.snapshot");
      expect(snapshotCalls).toHaveLength(1);
    });

    await user.click(screen.getByRole("button", { name: "Agent and model settings" }));
    expect(screen.queryByText("Loading installed agents…")).toBeNull();
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
    await enterFirstTask();

    const trigger = await screen.findByRole("button", { name: "Skills and agents" });
    await user.click(trigger);
    expect(await screen.findByRole("heading", { name: "Skills and agents" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Reviewed agents" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Installed skills" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Source actions" })).toBeTruthy();
    expect(screen.getByText("Create Architecture")).toBeTruthy();
    expect(screen.getByText("Winston")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Suggested next step" })).toBeNull();
    expect(screen.queryByText(/No active governed session/i)).toBeNull();

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
    await enterFirstTask();
    await screen.findAllByText("Browser preview");
    expect(screen.queryByRole("button", { name: "Skills and agents" })).toBeNull();
    expect(screen.getByLabelText("Describe a task")).toHaveProperty("disabled", false);
    browser.unmount();

    const runtime = await recoveryRuntime();
    render(<App hostRuntimeLoader={async () => runtime} />);
    await enterFirstTask();
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
    await enterFirstTask();

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
    await enterFirstTask();

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
    await enterFirstTask();

    await waitFor(() => {
      expect(invoke.mock.calls.filter(([command]) => command === "host_bootstrap")).toHaveLength(2);
    });
    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));
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
    await enterFirstTask();

    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Skills and agents" })).toBeNull();
    });
    expect(screen.getAllByText("Local host ready").length).toBeGreaterThan(0);
    expect(screen.queryByText("Read-only recovery")).toBeNull();
    expect(screen.getByRole("button", { name: "Add files or folder" })).toBeTruthy();
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
    await enterFirstTask();

    await waitFor(() => {
      expect(invoke.mock.calls.some(([command]) => command === "host_projection_events")).toBe(true);
    });
    await waitFor(() => {
      expect(invoke.mock.calls.filter(([command]) => command === "host_bootstrap")).toHaveLength(2);
    });
    await user.click(await screen.findByRole("button", { name: "Skills and agents" }));
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
    await enterFirstTask();
    await screen.findAllByText("Browser preview");
    const results = await axe.run(container, {
      rules: {
        "color-contrast": { enabled: false },
      },
    });

    expect(results.violations).toEqual([]);
  });

  it("has no automated accessibility violations in the populated Files drawer", async () => {
    const { container } = render(<App />);
    await enterFirstTask();
    await screen.findAllByText("Browser preview");
    openWorkspaceFiles();
    await screen.findByRole("button", { name: /README\.md/i });
    const results = await axe.run(container, {
      rules: {
        "color-contrast": { enabled: false },
      },
    });

    expect(results.violations).toEqual([]);
  });
});

/// The task list itself: how a task gets its name, and how the user gets rid
/// of one. Before this, a task's title was written once at creation and never
/// touched again, and there was no close or delete anywhere in the shell.
describe("Forge task list", () => {
  function renderApp(runtime: HostRuntime) {
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
  }

  function taskTabNames(): string[] {
    return screen.queryAllByRole("tab").map((tab) => tab.textContent ?? "");
  }

  async function sendMessage(
    user: ReturnType<typeof userEvent.setup>,
    text: string,
  ): Promise<void> {
    await user.type(await readyMethodGuidanceComposer(), text);
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await screen.findAllByText("Deterministic local reply.");
  }

  it("names a task after its first message rather than leaving it New task", async () => {
    // The reported bug: only the Home composer ever titled a task, so every
    // task opened from the tab strip or the palette stayed called "New task".
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();

    expect(taskTabNames()).toEqual(["New task"]);

    await sendMessage(user, "Explain how the workspace is structured");

    expect(taskTabNames()).toEqual(["Explain how the workspace is structured"]);
    expect(
      screen.getByRole("heading", { name: "Explain how the workspace is structured" }),
    ).toBeTruthy();
  });

  it("keeps the automatic name from the opening message as the conversation goes on", async () => {
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();

    await sendMessage(user, "Plan the migration");
    await sendMessage(user, "Actually review the parser instead");

    // The title describes how the conversation opened; later messages do not
    // rewrite it out from under the user.
    expect(taskTabNames()).toEqual(["Plan the migration"]);
  });

  it("renames a task by hand and never lets a later message overwrite that name", async () => {
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();

    await user.click(screen.getByRole("button", { name: "Rename task New task" }));
    const field = screen.getByLabelText<HTMLInputElement>("Task name");
    await user.clear(field);
    await user.type(field, "Release checklist");
    await user.click(screen.getByRole("button", { name: "Save name" }));

    expect(taskTabNames()).toEqual(["Release checklist"]);
    expect(screen.queryByLabelText("Task name")).toBeNull();

    await sendMessage(user, "Draft the rollout steps");
    expect(taskTabNames()).toEqual(["Release checklist"]);
  });

  it("abandons a rename on Cancel without touching the name", async () => {
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();
    await sendMessage(user, "Audit the IPC catalog");

    await user.click(
      screen.getByRole("button", { name: "Rename task Audit the IPC catalog" }),
    );
    await user.clear(screen.getByLabelText("Task name"));
    await user.type(screen.getByLabelText("Task name"), "Discarded");
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(taskTabNames()).toEqual(["Audit the IPC catalog"]);
  });

  it("closes a task without destroying it and reopens it with its transcript", async () => {
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();
    await sendMessage(user, "Trace the boot sequence");

    await user.click(screen.getByRole("button", { name: "Close task" }));

    // Closing the only open task lands on Home; there is no phantom tab to
    // fall back onto.
    expect(taskTabNames()).toEqual([]);
    expect(screen.getByRole("heading", { name: "Recent tasks" })).toBeTruthy();
    // Closing is not deleting: the task is offered back.
    const closed = screen.getByRole("list", { name: "Closed tasks" });
    expect(within(closed).getByText("Trace the boot sequence")).toBeTruthy();

    await user.click(
      screen.getByRole("button", { name: "Reopen task Trace the boot sequence" }),
    );

    expect(taskTabNames()).toEqual(["Trace the boot sequence"]);
    // The transcript survived the round trip — that is the whole difference
    // between closing and deleting.
    expect(screen.getByRole("log", { name: "Conversation" })).toBeTruthy();
    expect(screen.getAllByText("Deterministic local reply.").length).toBeGreaterThan(0);
  });

  it("never deletes a task without confirmation, and cancelling changes nothing", async () => {
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();
    await sendMessage(user, "Review the recovery path");

    await user.click(screen.getByRole("button", { name: "Delete task" }));

    const dialog = screen.getByRole("dialog", { name: "Delete this task?" });
    // The dialog names the task and says what is actually at stake.
    expect(within(dialog).getByText("Review the recovery path")).toBeTruthy();
    expect(within(dialog).getByText(/2 messages/u)).toBeTruthy();
    expect(within(dialog).getByText(/cannot be undone/iu)).toBeTruthy();
    // It offers the non-destructive alternative rather than only a scary button.
    expect(within(dialog).getByText(/Close the task instead/iu)).toBeTruthy();

    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));

    expect(screen.queryByRole("dialog", { name: "Delete this task?" })).toBeNull();
    expect(taskTabNames()).toEqual(["Review the recovery path"]);
    expect(screen.getByRole("log", { name: "Conversation" })).toBeTruthy();
  });

  it("is honest that deleting cannot scrub the copy already written to this device", async () => {
    // Forge has no per-session chat delete in the host command catalog and the
    // local store offers no removal, so the dialog must not promise an erase
    // that does not happen. It points at the one control that does erase.
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();
    // A message must actually be sent first. This used to delete a task that
    // had never reached `chat.send` and assert the warning anyway — so it
    // pinned the dialog warning about an encrypted copy that was never written.
    await sendMessage(user, "Write something durable");

    await user.click(screen.getByRole("button", { name: "Delete task" }));

    const dialog = screen.getByRole("dialog", { name: "Delete this task?" });
    expect(within(dialog).getByText(/cannot erase the encrypted copy/iu)).toBeTruthy();
    expect(within(dialog).getByText(/Local data/u)).toBeTruthy();
  });

  /// `messageCount` is `lane.messages.length`, which is `0` for a genuinely
  /// empty task *and* for one whose history has not been read. Saying "no
  /// messages yet" on the second is telling the user there is nothing to lose
  /// at the moment that is most likely to be false — and because task ids are
  /// never reused, a transcript discarded on that sentence is orphaned forever.
  it("never claims a task has no messages before its history has been read", async () => {
    const { runtime } = await bmadHelpRuntime({ holdChatHistory: true });
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();

    await user.click(screen.getByRole("button", { name: "Delete task" }));

    const dialog = screen.getByRole("dialog", { name: "Delete this task?" });
    expect(
      within(dialog).getByText(/has not finished reading this task's messages/iu),
    ).toBeTruthy();
    expect(within(dialog).queryByText(/no messages yet/iu)).toBeNull();
  });

  /// The mirror: a task proven to hold nothing never reached `chat.send`, so no
  /// durable row exists and warning about one is the same untruth inverted.
  it("does not warn about an encrypted copy a task never wrote", async () => {
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();

    await user.click(screen.getByRole("button", { name: "Delete task" }));

    const dialog = screen.getByRole("dialog", { name: "Delete this task?" });
    // The history read resolves empty, so this is the one state where "no
    // messages yet" is true — and the encrypted-copy warning is not.
    expect(await within(dialog).findByText(/no messages yet/iu)).toBeTruthy();
    expect(within(dialog).queryByText(/cannot erase the encrypted copy/iu)).toBeNull();
  });

  it("destroys the task and its transcript on confirmation and returns Home", async () => {
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();
    await sendMessage(user, "Destroy this task");

    await user.click(screen.getByRole("button", { name: "Delete task" }));
    const dialog = screen.getByRole("dialog", { name: "Delete this task?" });
    await user.click(within(dialog).getByRole("button", { name: "Delete task" }));

    // Deleting the last task leaves the list genuinely empty.
    expect(taskTabNames()).toEqual([]);
    expect(screen.queryByRole("dialog", { name: "Delete this task?" })).toBeNull();
    expect(screen.getByRole("heading", { name: "Recent tasks" })).toBeTruthy();
    expect(screen.getByText(/No tasks yet/u)).toBeTruthy();
    // Unlike closing, nothing is offered back.
    expect(screen.queryByRole("list", { name: "Closed tasks" })).toBeNull();
    expect(screen.queryByText("Deterministic local reply.")).toBeNull();
  });

  it("selects the neighbouring task when a task in the middle is deleted", async () => {
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    const titlebar = await screen.findByRole("banner", { name: "Forge application" });
    const newTask = () =>
      user.click(within(titlebar).getByRole("button", { name: "New task" }));

    await enterFirstTask();
    await sendMessage(user, "First");
    await newTask();
    await sendMessage(user, "Second");
    await newTask();
    await sendMessage(user, "Third");

    await user.click(screen.getAllByRole("tab")[1]!);
    expect(screen.getByRole("heading", { name: "Second" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Delete task" }));
    await user.click(
      within(screen.getByRole("dialog", { name: "Delete this task?" }))
        .getByRole("button", { name: "Delete task" }),
    );

    expect(taskTabNames()).toEqual(["First", "Third"]);
    // The tab to the left, which is where the user most likely came from.
    expect(screen.getByRole("heading", { name: "First" })).toBeTruthy();
  });

  it("closes a task from its own tab without selecting it first", async () => {
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    const titlebar = await screen.findByRole("banner", { name: "Forge application" });

    await enterFirstTask();
    await sendMessage(user, "Keep this one");
    await user.click(within(titlebar).getByRole("button", { name: "New task" }));
    await sendMessage(user, "Close this from its tab");

    const target = screen.getByRole("tab", { name: /Close this from its tab/u });
    fireEvent.click(target.querySelector(".title-strip__tab-close")!);

    expect(taskTabNames()).toEqual(["Keep this one"]);
    expect(screen.getByRole("heading", { name: "Keep this one" })).toBeTruthy();
  });

  it("gives every task a fresh durable session ref so none inherits another transcript", async () => {
    // The host derives the durable chat session from `clientSessionRef`, so a
    // recycled id would hand a new task an older task's persisted messages —
    // including one the user deleted. Ids used to be `new-1`, `new-2`, ...
    // from a counter that restarted with the process.
    const { runtime, invoke } = await bmadHelpRuntime();
    const user = userEvent.setup();
    renderApp(runtime);
    const titlebar = await screen.findByRole("banner", { name: "Forge application" });

    await enterFirstTask();
    await sendMessage(user, "First task message");
    await user.click(within(titlebar).getByRole("button", { name: "New task" }));
    await sendMessage(user, "Second task message");

    const refs = invoke.mock.calls
      .filter(([command, args]) => command === "host_dispatch"
        && (JSON.parse(String(args?.body)) as { command: string }).command === "chat.send")
      .map(([, args]) => (JSON.parse(String(args?.body)) as {
        payload: { clientSessionRef: string };
      }).payload.clientSessionRef);

    expect(refs).toHaveLength(2);
    expect(new Set(refs).size).toBe(2);
    for (const ref of refs) {
      expect(ref).not.toMatch(/^new-\d+$/u);
      // The host rejects a ref longer than 128 bytes.
      expect(ref.length).toBeLessThanOrEqual(128);
    }
  });

  it("has no automated accessibility violations with tasks open and a delete pending", async () => {
    const { runtime } = await bmadHelpRuntime();
    const user = userEvent.setup();
    const { container } = render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
    await enterFirstTask();
    await sendMessage(user, "Audit this task");
    await user.click(screen.getByRole("button", { name: "Delete task" }));
    await screen.findByRole("dialog", { name: "Delete this task?" });

    const results = await axe.run(container, {
      rules: { "color-contrast": { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });
});

/// The host owns the transcript; the renderer's store is a bounded cache in
/// front of it. Nothing used to re-read that copy, so evicting a lane or
/// switching workspace made a conversation that is still on disk disappear from
/// the screen.
///
/// Every assertion below is written so it can only pass if the host was asked
/// *and its answer was used*: the text asserted on is content the renderer has
/// never held and could not have cached.
describe("Forge conversation restore", () => {
  const at = 1_700_000_000_000;
  /// A transcript only the host can know, including a tool exchange — which
  /// `chat.send` never returns, so it can reach the screen by no other route.
  const durableTranscript = [
    { role: "user", text: "Where is the envelope parser?", createdAt: at },
    {
      role: "tool_call",
      text: "read_file crates/desktop-ipc/src/envelope.rs",
      createdAt: at + 1,
    },
    { role: "tool_result", text: "fn parse_chat_history(payload: Value)", createdAt: at + 2 },
    {
      role: "assistant",
      text: "Restored from the durable store, not from the renderer cache.",
      createdAt: at + 3,
    },
  ] as const;

  function renderApp(runtime: HostRuntime) {
    render(
      <App
        hostRuntimeLoader={async () => runtime}
        projectionPollIntervalMs={60_000}
      />,
    );
  }

  function conversation(): HTMLElement {
    return screen.getByRole("log", { name: "Conversation" });
  }

  it("reads the host's copy of the transcript when a task is activated", async () => {
    const { runtime, invoke } = await bmadHelpRuntime({
      chatHistoryFor: () => durableTranscript,
    });
    renderApp(runtime);

    await enterFirstTask();

    // Nothing has been sent, and the renderer has never held this text. The
    // only way it can be on screen is that `chat.history` was dispatched for
    // this task and its answer was applied.
    expect(
      await screen.findByText("Restored from the durable store, not from the renderer cache."),
    ).toBeTruthy();
    expect(within(conversation()).getByText("Where is the envelope parser?")).toBeTruthy();
    expect(dispatchedCommandNames(invoke)).not.toContain("chat.send");

    const requests = chatHistoryRequests(invoke);
    expect(requests).toHaveLength(1);
    expect(requests[0]!.workspaceId).toBe(recoveryBootstrap.workspaces[0]!.workspaceId);
    expect(requests[0]!.workspaceGrantEpoch).toBe(recoveryBootstrap.workspaces[0]!.grantEpoch);
    // The read is scoped to the durable session ref of the activated task —
    // the same value `chat.send` carries — not to the Home placeholder.
    expect(requests[0]!.clientSessionRef).not.toBe("task-none");
    expect(requests[0]!.clientSessionRef.length).toBeGreaterThan(0);
  });

  it("renders the tool exchanges a restored transcript carries", async () => {
    // `chat.send` returns only a question and its answer, so before history was
    // read there was no path by which a tool call could ever reach the
    // transcript, even though it has always known how to render one.
    const { runtime } = await bmadHelpRuntime({ chatHistoryFor: () => durableTranscript });
    renderApp(runtime);

    await enterFirstTask();
    await screen.findByText("Restored from the durable store, not from the renderer cache.");

    const transcript = conversation();
    expect(within(transcript).getByText("Tool requested by the model")).toBeTruthy();
    expect(
      within(transcript).getByText("read_file crates/desktop-ipc/src/envelope.rs"),
    ).toBeTruthy();
    // Workspace content is labelled as such rather than styled as Forge speaking.
    expect(
      within(transcript).getByText("Tool result · workspace content, not Forge"),
    ).toBeTruthy();
    expect(
      within(transcript).getAllByText("fn parse_chat_history(payload: Value)").length,
    ).toBeGreaterThan(0);
  });

  it("leaves a turn in flight untouched when the read lands mid-send", async () => {
    const { runtime, createStarted, releaseChatHistory, releaseCreate } = await bmadHelpRuntime({
      chatHistoryFor: () => durableTranscript,
      holdChatHistory: true,
      holdCreate: true,
    });
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();

    const composer = await readyMethodGuidanceComposer();
    await user.type(composer, "Explain the boundary check");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await createStarted;

    // The read comes back while the question is still unanswered.
    releaseChatHistory();
    await waitFor(() => expect(screen.getByText("Thinking…")).toBeTruthy());
    releaseCreate();

    expect(await screen.findByText("Deterministic local reply.")).toBeTruthy();
    const transcript = conversation();
    // The live turn survived intact and exactly once...
    expect(within(transcript).getAllByText("Explain the boundary check")).toHaveLength(1);
    expect(within(transcript).getAllByText("Deterministic local reply.")).toHaveLength(1);
    // ...and the host's older answer, which predates the question just asked,
    // was not written over it.
    expect(
      within(transcript).queryByText("Restored from the durable store, not from the renderer cache."),
    ).toBeNull();
  });

  it("does not replace a completed turn with a read that was still in flight", async () => {
    const { runtime, releaseChatHistory } = await bmadHelpRuntime({
      chatHistoryFor: () => durableTranscript,
      holdChatHistory: true,
    });
    const user = userEvent.setup();
    renderApp(runtime);
    await enterFirstTask();

    await user.type(await readyMethodGuidanceComposer(), "Explain the boundary check");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await screen.findByText("Deterministic local reply.");

    releaseChatHistory();
    // Give the resolved read a chance to be applied before asserting it was not.
    await act(async () => { await Promise.resolve(); });

    const transcript = conversation();
    expect(within(transcript).getAllByText("Explain the boundary check")).toHaveLength(1);
    expect(within(transcript).getAllByText("Deterministic local reply.")).toHaveLength(1);
    expect(
      within(transcript).queryByText("Restored from the durable store, not from the renderer cache."),
    ).toBeNull();
  });

  it("re-reads a transcript the bounded lane store evicted, keeping the tab open", async () => {
    // The reported defect, end to end, and the exact shape in which it is
    // reachable. `MAX_TASKS` and `MAX_LANE_SESSIONS` are both twenty, but they
    // do not evict the same thing: the task list drops the oldest *closed*
    // task first, while the lane store drops the oldest lane it holds. Close
    // one task and add another and the two disagree — the closed task's record
    // goes, and the *first* task's transcript goes, while its tab is still open
    // and selected. That tab used to show a conversation the user had
    // definitely had as if it had never happened.
    let firstTaskReads = 0;
    const { runtime, invoke } = await bmadHelpRuntime({
      chatHistoryFor: (_sessionRef, ordinal) => {
        if (ordinal !== 0) return [];
        firstTaskReads += 1;
        // Empty on activation — the conversation starts here — and the durable
        // record on the re-read after eviction. So the text asserted below can
        // come neither from the first read nor from anything cached.
        return firstTaskReads === 1 ? [] : durableTranscript;
      },
    });
    const user = userEvent.setup();
    renderApp(runtime);
    const titlebar = await screen.findByRole("banner", { name: "Forge application" });

    await enterFirstTask();
    await user.type(await readyMethodGuidanceComposer(), "Trace the boot sequence");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await screen.findByText("Deterministic local reply.");
    const [firstRequest] = chatHistoryRequests(invoke);
    expect(firstRequest, "the activated task's transcript was never requested").toBeDefined();
    const firstTaskRef = firstRequest!.clientSessionRef;

    // Nineteen more tasks fill both bounds exactly.
    for (let index = 0; index < 19; index += 1) {
      fireEvent.click(within(titlebar).getByRole("button", { name: "New task" }));
    }
    // Closing keeps the task's record and its transcript, so it is the record
    // the task list gives up next — but the lane store gives up the oldest lane.
    await user.click(screen.getByRole("button", { name: "Close task" }));
    fireEvent.click(within(titlebar).getByRole("button", { name: "New task" }));

    fireEvent.click(screen.getByRole("tab", { name: "Trace the boot sequence" }));

    expect(
      await screen.findByText("Restored from the durable store, not from the renderer cache."),
    ).toBeTruthy();
    // Asked twice for the same conversation: once on first activation, once
    // after eviction took the lane away.
    expect(
      chatHistoryRequests(invoke).filter(({ clientSessionRef }) => clientSessionRef === firstTaskRef),
    ).toHaveLength(2);
  });

  it("says a transcript could not be read rather than showing an empty task", async () => {
    const { runtime, invoke } = await bmadHelpRuntime({
      chatHistoryFailures: 1,
      chatHistoryFor: () => durableTranscript,
    });
    const user = userEvent.setup();
    renderApp(runtime);

    await enterFirstTask();

    const alert = await screen.findByRole("alert");
    // An empty transcript with no explanation reads as "you never said
    // anything here", which is the opposite of the truth.
    expect(alert.textContent).toContain("could not read this task's earlier messages");
    expect(alert.textContent).toContain("may not be the whole conversation");

    await user.click(within(alert).getByRole("button", { name: "Retry loading messages" }));

    expect(
      await screen.findByText("Restored from the durable store, not from the renderer cache."),
    ).toBeTruthy();
    expect(chatHistoryRequests(invoke)).toHaveLength(2);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
