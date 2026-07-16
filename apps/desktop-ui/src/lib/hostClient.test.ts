import { describe, expect, it, vi } from "vitest";
import {
  DesktopHostClient,
  HostCapabilityError,
  HostCommandError,
  HostProtocolError,
  initializeHostRuntime,
  parseBootstrapReply,
  type BootstrapReply,
  type TauriInvoke,
} from "./hostClient";
import type {
  BmadHelpRunCreatedProjection,
  BmadLibrarySnapshot,
} from "./bmadProjection";
import type {
  BmadHelpApprovedProjection,
  BmadHelpCancelledProjection,
  BmadHelpContextReviewProjection,
  BmadHelpRunCompletedProjection,
  ModelAuthStatusProjection,
} from "./bmadModelProjection";

const readyBootstrap: BootstrapReply = {
  schemaVersion: "desktop-bootstrap.v1",
  rendererSessionId: "renderer_01K0Q6H3",
  installationId: "install_01K0Q6H3",
  windowLabel: "main",
  bootMode: "ready",
  supportedCommands: [
    "app.get_boot_state",
    "workspace.select_folder",
    "workspace.list",
    "bmad.library.snapshot",
  ],
  workspaces: [
    {
      workspaceId: "workspace_01K0Q6H3",
      projectId: "project_01K0Q6H3",
      displayName: "sapphirus-desktop",
      grantEpoch: 3,
      permissions: "read_only",
    },
  ],
  projectionSequence: 12,
};

const d1Bootstrap: BootstrapReply = {
  ...readyBootstrap,
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

const helpBootstrap: BootstrapReply = {
  ...d1Bootstrap,
  supportedCommands: [
    ...d1Bootstrap.supportedCommands,
    "model.auth.status",
    "model.auth.sign_in",
    "model.auth.sign_out",
    "bmad.help.prepare",
    "bmad.help.approve",
    "bmad.help.cancel",
    "bmad.help.submit",
    "bmad.help.latest",
    "run.create",
  ],
};

const firstCursor = "cursor_01ARZ3NDEKTSV4RRFFQ69G5FAV";
const secondCursor = "cursor_01ARZ3NDEKTSV4RRFFQ69G5FAW";
const digestA = `sha256:${"a".repeat(64)}`;
const digestB = `sha256:${"b".repeat(64)}`;

const modelAuthStatus: ModelAuthStatusProjection = {
  status: "development_ready",
  mode: "deterministic_development",
  authEpoch: 5,
  developmentOnly: true,
  destinationLabel: "Deterministic local model",
  signInAvailable: false,
  signOutAvailable: true,
};

const bmadLibrarySnapshot: BmadLibrarySnapshot = {
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
    displayName: "Create Architecture",
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
};

const bmadHelpRun: BmadHelpRunCreatedProjection = {
  schemaVersion: "bmad-help-run.v1",
  runKind: "bmad_help",
  lifecycle: "created_unbound",
  workspaceId: "workspace_01K0Q6H3",
  runId: "run_01K0Q6H3",
  sessionId: "session_01K0Q6H3",
  currentIntent: "Help me choose the next Method step",
  runnable: false,
  completionClaimed: false,
  recommendation: {
    schemaVersion: "bmad-help-recommendation.v1",
    displayName: "BMad Help",
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

const bmadHelpReview: BmadHelpContextReviewProjection = {
  workspaceId: bmadHelpRun.workspaceId,
  workspaceGrantEpoch: 3,
  runId: bmadHelpRun.runId,
  sessionId: bmadHelpRun.sessionId,
  destinationLabel: "Deterministic local model",
  developmentOnly: true,
  consentDisclosure: "Send the exact reviewed context once for this Help request.",
  manifestHash: digestA,
  purpose: "bmad_help",
  region: "localdev",
  retentionMode: "transient_no_store",
  expiresAt: 1_725_000_300_000,
  items: [{
    relativeLabel: "method/current-intent.txt",
    semanticRole: "current_intent",
    language: "text",
    outboundByteCount: 35,
    tokenEstimate: 9,
    classification: "internal",
    redactions: [],
    outboundContent: "Help me choose the next Method step",
  }],
  exclusions: [{ relativeLabel: "secrets/.env", reason: "Secret-bearing input excluded" }],
  secretFindings: [{
    relativeLabel: "method/current-intent.txt",
    kind: "example_secret",
    occurrenceCount: 1,
  }],
  totalOutboundBytes: 35,
  totalTokenEstimate: 9,
  redactionLimitation: "Redaction reduces risk but cannot prove every secret was detected.",
};

const bmadHelpApproval: BmadHelpApprovedProjection = {
  manifestHash: digestA,
  decisionId: "decision_01K0Q6H3",
  expiresAt: 1_725_000_200_000,
  sendEligible: true,
};

const bmadHelpCancelled: BmadHelpCancelledProjection = {
  manifestHash: digestA,
  decisionId: bmadHelpApproval.decisionId,
};

const bmadHelpCompleted: BmadHelpRunCompletedProjection = {
  schemaVersion: "bmad-help-completed.v1",
  runKind: "bmad_help",
  lifecycle: "completed",
  workspaceId: bmadHelpRun.workspaceId,
  runId: bmadHelpRun.runId,
  sessionId: bmadHelpRun.sessionId,
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
    rationaleSummary: "The reviewed intent explicitly asks for architecture readiness.",
    createdAt: 1_725_000_001_200,
  },
  receipt: {
    schemaVersion: "bmad-model-receipt-summary.v1",
    receiptId: "receipt_01K0Q6H3",
    status: "succeeded",
    retentionMode: "transient_no_store",
    region: "localdev",
    inputBytes: 512,
    outputBytes: 256,
    startedAt: 1_725_000_001_000,
    completedAt: 1_725_000_001_100,
  },
};

function successfulReply(requestId: string, data: unknown, sequence = 12) {
  return {
    schemaVersion: "desktop-dispatch-reply.v1",
    requestId,
    sequence,
    status: "ok",
    receipt: {
      requestId,
      acceptedAt: 1_725_000_000_005,
      operationId: null as string | null,
    },
    data,
  };
}

async function createBmadReplyClient(data: unknown, bootstrap = readyBootstrap) {
  const invoke = vi.fn<TauriInvoke>(async (command, args) => {
    if (command === "host_bootstrap") {
      return bootstrap;
    }
    const envelope = JSON.parse(String(args?.body)) as { requestId: string };
    return successfulReply(envelope.requestId, data);
  });
  const client = new DesktopHostClient({
    invoke,
    requestId: () => "request_bmad_validation",
  });
  await client.bootstrap();
  return { client, invoke };
}

describe("DesktopHostClient", () => {
  it("creates an inert Help run through the exact renderer-bound payload", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return helpBootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      const reply = successfulReply(envelope.requestId, {
        kind: "bmad_help_run_created",
        value: bmadHelpRun,
      }, 13);
      reply.receipt.operationId = bmadHelpRun.runId;
      return reply;
    });
    const client = new DesktopHostClient({
      invoke,
      now: () => 1_725_000_000_000,
      requestId: () => "request_bmad_help_run",
    });
    await client.bootstrap();

    await expect(client.createBmadHelpRun(
      "workspace_01K0Q6H3",
      3,
      "Help me choose the next Method step",
    )).resolves.toEqual(bmadHelpRun);

    expect(invoke).toHaveBeenNthCalledWith(2, "host_dispatch", {
      body: JSON.stringify({
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_bmad_help_run",
        command: "run.create",
        windowLabel: "main",
        rendererSessionId: "renderer_01K0Q6H3",
        installationId: "install_01K0Q6H3",
        issuedAt: 1_725_000_000_000,
        payload: {
          workspaceId: "workspace_01K0Q6H3",
          workspaceGrantEpoch: 3,
          runKind: "bmad_help",
          currentIntent: "Help me choose the next Method step",
        },
      }),
    });
  });

  it("uses the exact closed auth, review, approval, cancel, submit, and completed contracts", async () => {
    let request = 0;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") return helpBootstrap;
      const envelope = JSON.parse(String(args?.body)) as { command: string; requestId: string };
      const responses = {
        "model.auth.status": { kind: "model_auth_status", value: modelAuthStatus },
        "model.auth.sign_in": { kind: "model_auth_status", value: modelAuthStatus },
        "model.auth.sign_out": {
          kind: "model_auth_status",
          value: { ...modelAuthStatus, authEpoch: modelAuthStatus.authEpoch + 1 },
        },
        "bmad.help.prepare": { kind: "bmad_help_review", value: bmadHelpReview },
        "bmad.help.approve": { kind: "bmad_help_approved", value: bmadHelpApproval },
        "bmad.help.cancel": { kind: "bmad_help_cancelled", value: bmadHelpCancelled },
        "bmad.help.submit": { kind: "bmad_help_run_completed", value: bmadHelpCompleted },
        "bmad.help.latest": { kind: "bmad_help_run_completed", value: bmadHelpCompleted },
      } as const;
      const data = responses[envelope.command as keyof typeof responses];
      return successfulReply(envelope.requestId, data, 13);
    });
    const client = new DesktopHostClient({
      invoke,
      now: () => 1_725_000_000_000,
      requestId: () => `request_model_${request += 1}`,
    });
    await client.bootstrap();

    await expect(client.modelAuthStatus()).resolves.toEqual(modelAuthStatus);
    await expect(client.modelAuthSignIn()).resolves.toEqual(modelAuthStatus);
    await expect(client.modelAuthSignOut()).resolves.toEqual({
      ...modelAuthStatus,
      authEpoch: modelAuthStatus.authEpoch + 1,
    });
    await expect(client.prepareBmadHelp(bmadHelpRun.workspaceId, 3)).resolves.toEqual(bmadHelpReview);
    await expect(client.approveBmadHelp(bmadHelpRun.workspaceId, 3, digestA)).resolves
      .toEqual(bmadHelpApproval);
    await expect(client.cancelBmadHelp(
      bmadHelpRun.workspaceId,
      3,
      digestA,
      bmadHelpApproval.decisionId,
    )).resolves.toEqual(bmadHelpCancelled);
    await expect(client.submitBmadHelp(
      bmadHelpRun.workspaceId,
      3,
      digestA,
      bmadHelpApproval.decisionId,
    )).resolves.toEqual(bmadHelpCompleted);
    await expect(client.latestBmadHelpRun(bmadHelpRun.workspaceId, 3)).resolves.toEqual({
      kind: "completed",
      result: bmadHelpCompleted,
    });

    const envelopes = invoke.mock.calls.slice(1).map(([, args]) =>
      JSON.parse(String(args?.body)) as { command: string; payload: unknown }
    );
    expect(envelopes.map(({ command }) => command)).toEqual([
      "model.auth.status",
      "model.auth.sign_in",
      "model.auth.sign_out",
      "bmad.help.prepare",
      "bmad.help.approve",
      "bmad.help.cancel",
      "bmad.help.submit",
      "bmad.help.latest",
    ]);
    expect(envelopes.map(({ payload }) => payload)).toEqual([
      {},
      {},
      {},
      { workspaceId: bmadHelpRun.workspaceId, workspaceGrantEpoch: 3 },
      { workspaceId: bmadHelpRun.workspaceId, workspaceGrantEpoch: 3, manifestHash: digestA },
      {
        workspaceId: bmadHelpRun.workspaceId,
        workspaceGrantEpoch: 3,
        manifestHash: digestA,
        decisionId: bmadHelpApproval.decisionId,
      },
      {
        workspaceId: bmadHelpRun.workspaceId,
        workspaceGrantEpoch: 3,
        manifestHash: digestA,
        decisionId: bmadHelpApproval.decisionId,
      },
      { workspaceId: bmadHelpRun.workspaceId, workspaceGrantEpoch: 3 },
    ]);
  });

  it("loads the latest retained Help run through an exact read-only envelope", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return helpBootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return successfulReply(envelope.requestId, {
        kind: "bmad_help_run_created",
        value: bmadHelpRun,
      }, 13);
    });
    const client = new DesktopHostClient({
      invoke,
      now: () => 1_725_000_000_000,
      requestId: () => "request_bmad_help_latest",
    });
    await client.bootstrap();

    await expect(client.latestBmadHelpRun(
      "workspace_01K0Q6H3",
      3,
    )).resolves.toEqual({ kind: "retained", run: bmadHelpRun });

    expect(invoke).toHaveBeenNthCalledWith(2, "host_dispatch", {
      body: JSON.stringify({
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_bmad_help_latest",
        command: "bmad.help.latest",
        windowLabel: "main",
        rendererSessionId: "renderer_01K0Q6H3",
        installationId: "install_01K0Q6H3",
        issuedAt: 1_725_000_000_000,
        payload: {
          workspaceId: "workspace_01K0Q6H3",
          workspaceGrantEpoch: 3,
        },
      }),
    });
  });

  it.each([
    [
      "review",
      { kind: "bmad_help_review", value: bmadHelpReview },
      { kind: "review", review: bmadHelpReview },
    ],
    [
      "approved lifecycle",
      {
        kind: "bmad_help_approved_lifecycle",
        value: { review: bmadHelpReview, approval: bmadHelpApproval },
      },
      { kind: "approved", review: bmadHelpReview, approval: bmadHelpApproval },
    ],
    [
      "terminal lifecycle",
      {
        kind: "bmad_help_terminal",
        value: {
          workspaceId: bmadHelpRun.workspaceId,
          reason: "consent_consumed",
          resumable: false,
          sendEligible: false,
        },
      },
      {
        kind: "terminal",
        terminal: {
          workspaceId: bmadHelpRun.workspaceId,
          reason: "consent_consumed",
          resumable: false,
          sendEligible: false,
        },
      },
    ],
  ])("loads the exact latest %s projection", async (_name, data, expected) => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") return helpBootstrap;
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return successfulReply(envelope.requestId, data, 13);
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => "request_bmad_help_latest_lifecycle",
    });
    await client.bootstrap();

    await expect(client.latestBmadHelpRun(bmadHelpRun.workspaceId, 3)).resolves.toEqual(expected);
  });

  it("returns null only for the exact no-retained-Help-run reply", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return helpBootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return successfulReply(envelope.requestId, { kind: "no_bmad_help_run" }, 13);
    });
    const client = new DesktopHostClient({ invoke });
    await client.bootstrap();

    await expect(client.latestBmadHelpRun(bmadHelpRun.workspaceId, 3)).resolves.toEqual({
      kind: "no_run",
    });
  });

  it("surfaces an authenticated legacy run whose projection cannot be restored", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return helpBootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return successfulReply(envelope.requestId, {
        kind: "bmad_help_projection_unavailable",
      }, 13);
    });
    const client = new DesktopHostClient({ invoke });
    await client.bootstrap();

    await expect(client.latestBmadHelpRun(bmadHelpRun.workspaceId, 3)).resolves.toEqual({
      kind: "projection_unavailable",
    });
  });

  it.each([
    { kind: "no_bmad_help_run", value: null },
    { kind: "no_bmad_help_run", runId: bmadHelpRun.runId },
    { kind: "bmad_help_projection_unavailable", value: null },
    { kind: "bmad_help_run_created" },
    { kind: "bmad_help_run_created", value: { ...bmadHelpRun, runnable: true } },
  ])("rejects an inexact retained Help run reply %#", async (data) => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return helpBootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return successfulReply(envelope.requestId, data, 13);
    });
    const client = new DesktopHostClient({ invoke });
    await client.bootstrap();

    await expect(client.latestBmadHelpRun(
      bmadHelpRun.workspaceId,
      3,
    )).rejects.toBeInstanceOf(HostProtocolError);
  });

  it.each([
    ["run schema", { ...bmadHelpRun, schemaVersion: "bmad-help-run.v2" }],
    ["run kind", { ...bmadHelpRun, runKind: "bmad_architecture" }],
    ["lifecycle", { ...bmadHelpRun, lifecycle: "running" }],
    ["runnable state", { ...bmadHelpRun, runnable: true }],
    ["completion state", { ...bmadHelpRun, completionClaimed: true }],
    ["missing current intent", (({ currentIntent: _, ...run }) => run)(bmadHelpRun)],
    ["unsafe current intent", { ...bmadHelpRun, currentIntent: "unsafe\u0000intent" }],
    ["recommendation completion", {
      ...bmadHelpRun,
      recommendation: { ...bmadHelpRun.recommendation, completionClaimed: true },
    }],
    ["source kind", {
      ...bmadHelpRun,
      recommendation: {
        ...bmadHelpRun.recommendation,
        source: { ...bmadHelpRun.recommendation.source, sourceKind: "renderer" },
      },
    }],
    ["authority field", { ...bmadHelpRun, authorityRef: "must-not-cross-ipc" }],
    ["nested authority field", {
      ...bmadHelpRun,
      recommendation: { ...bmadHelpRun.recommendation, capabilityCatalogHash: digestA },
    }],
  ])("rejects an unsafe Help run %s", async (_field, value) => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return helpBootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      const reply = successfulReply(envelope.requestId, {
        kind: "bmad_help_run_created",
        value,
      });
      reply.receipt.operationId = bmadHelpRun.runId;
      return reply;
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => "request_bmad_help_invalid",
    });
    await client.bootstrap();

    await expect(client.createBmadHelpRun(
      bmadHelpRun.workspaceId,
      3,
      "BMad Help",
    )).rejects.toBeInstanceOf(HostProtocolError);
  });

  it("rejects a Help run whose receipt is not bound to the projected run", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return helpBootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      const reply = successfulReply(envelope.requestId, {
        kind: "bmad_help_run_created",
        value: bmadHelpRun,
      });
      reply.receipt.operationId = "run_different";
      return reply;
    });
    const client = new DesktopHostClient({ invoke });
    await client.bootstrap();

    await expect(client.createBmadHelpRun(
      bmadHelpRun.workspaceId,
      3,
      "BMad Help",
    )).rejects.toBeInstanceOf(HostProtocolError);
  });

  it("requests and validates the installed Method library through the exact renderer envelope", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return successfulReply(envelope.requestId, {
        kind: "bmad_library_snapshot",
        value: bmadLibrarySnapshot,
      });
    });
    const client = new DesktopHostClient({
      invoke,
      now: () => 1_725_000_000_000,
      requestId: () => "request_bmad_library",
    });
    await client.bootstrap();

    await expect(client.bmadLibrarySnapshot()).resolves.toEqual(bmadLibrarySnapshot);

    expect(invoke).toHaveBeenNthCalledWith(2, "host_dispatch", {
      body: JSON.stringify({
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_bmad_library",
        command: "bmad.library.snapshot",
        windowLabel: "main",
        rendererSessionId: "renderer_01K0Q6H3",
        installationId: "install_01K0Q6H3",
        issuedAt: 1_725_000_000_000,
        payload: {
          scope: "installed_method",
          cursor: null,
        },
      }),
    });
  });

  it("forwards only a native-valid opaque BMAD continuation cursor", async () => {
    const cursor = "library-cursor.v1!";
    const { client, invoke } = await createBmadReplyClient({
      kind: "bmad_library_snapshot",
      value: bmadLibrarySnapshot,
    });

    await client.bmadLibrarySnapshot(cursor);

    const envelope = JSON.parse(String(invoke.mock.calls[1]?.[1]?.body)) as {
      payload: Record<string, unknown>;
    };
    expect(envelope.payload).toEqual({ scope: "installed_method", cursor });
    expect(Object.keys(envelope.payload)).toEqual(["scope", "cursor"]);
  });

  it.each([
    "",
    "contains space",
    "contains\u007fdelete",
    "é",
    "x".repeat(257),
  ])("rejects an invalid BMAD continuation cursor %j before dispatch", async (cursor) => {
    const { client, invoke } = await createBmadReplyClient({
      kind: "bmad_library_snapshot",
      value: bmadLibrarySnapshot,
    });

    await expect(client.bmadLibrarySnapshot(cursor)).rejects.toBeInstanceOf(HostProtocolError);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("capability-checks the current ready bootstrap before dispatch", async () => {
    const bootstrapWithoutLibrary: BootstrapReply = {
      ...readyBootstrap,
      supportedCommands: readyBootstrap.supportedCommands.filter(
        (command) => command !== "bmad.library.snapshot",
      ),
    };
    const { client, invoke } = await createBmadReplyClient(
      { kind: "bmad_library_snapshot", value: bmadLibrarySnapshot },
      bootstrapWithoutLibrary,
    );

    await expect(client.bmadLibrarySnapshot()).rejects.toBeInstanceOf(HostCapabilityError);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("does not return an in-flight Method library after the current bootstrap enters recovery", async () => {
    let resolveLibrary!: (value: unknown) => void;
    const libraryReply = new Promise<unknown>((resolve) => {
      resolveLibrary = resolve;
    });
    const invoke = vi.fn<TauriInvoke>(async (command) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      if (command === "host_projection_events") {
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId: readyBootstrap.rendererSessionId,
          status: "events",
          events: [{
            sequence: 13,
            occurredAt: 1_725_000_000_010,
            event: {
              type: "boot_state_changed",
              projection: { mode: "read_only_recovery" },
            },
          }],
        };
      }
      return libraryReply;
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => "request_bmad_recovery",
    });
    await client.bootstrap();

    const pending = client.bmadLibrarySnapshot();
    await client.projectionEvents(12);
    resolveLibrary(successfulReply("request_bmad_recovery", {
      kind: "bmad_library_snapshot",
      value: bmadLibrarySnapshot,
    }, 13));

    await expect(pending).rejects.toBeInstanceOf(HostCapabilityError);
  });

  it.each([
    ["dispatch data", {
      kind: "bmad_library_snapshot",
      value: bmadLibrarySnapshot,
      owner: "must-not-cross-ipc",
    }],
    ["snapshot", {
      kind: "bmad_library_snapshot",
      value: { ...bmadLibrarySnapshot, workspaceId: "must-not-cross-ipc" },
    }],
    ["source", {
      kind: "bmad_library_snapshot",
      value: {
        ...bmadLibrarySnapshot,
        source: { ...bmadLibrarySnapshot.source, owner: "must-not-cross-ipc" },
      },
    }],
    ["installed skill", {
      kind: "bmad_library_snapshot",
      value: {
        ...bmadLibrarySnapshot,
        installedSkills: [{
          ...bmadLibrarySnapshot.installedSkills[0]!,
          sessionId: "must-not-cross-ipc",
        }],
      },
    }],
    ["help action", {
      kind: "bmad_library_snapshot",
      value: {
        ...bmadLibrarySnapshot,
        helpActions: [{
          ...bmadLibrarySnapshot.helpActions[0]!,
          confidence: "must-not-cross-ipc",
        }],
      },
    }],
    ["method agent", {
      kind: "bmad_library_snapshot",
      value: {
        ...bmadLibrarySnapshot,
        methodAgents: [{
          ...bmadLibrarySnapshot.methodAgents[0]!,
          availabilityReason: "must-not-cross-ipc",
        }],
      },
    }],
    ["agent menu", {
      kind: "bmad_library_snapshot",
      value: {
        ...bmadLibrarySnapshot,
        methodAgents: [{
          ...bmadLibrarySnapshot.methodAgents[0]!,
          menus: [{
            ...bmadLibrarySnapshot.methodAgents[0]!.menus[0]!,
            prompt: "must-not-cross-ipc",
          }],
        }],
      },
    }],
  ])("rejects unknown keys on the %s projection record", async (_record, data) => {
    const { client } = await createBmadReplyClient(data);
    await expect(client.bmadLibrarySnapshot()).rejects.toBeInstanceOf(HostProtocolError);
  });

  it.each([
    ["installed skills", {
      ...bmadLibrarySnapshot,
      installedSkills: Array.from({ length: 65 }, (_, index) => ({
        ...bmadLibrarySnapshot.installedSkills[0]!,
        skillName: `skill_${index}`,
      })),
    }],
    ["help actions", {
      ...bmadLibrarySnapshot,
      helpActions: Array.from({ length: 65 }, (_, index) => ({
        ...bmadLibrarySnapshot.helpActions[0]!,
        action: `action_${index}`,
      })),
    }],
    ["method agents", {
      ...bmadLibrarySnapshot,
      methodAgents: Array.from({ length: 17 }, (_, index) => ({
        ...bmadLibrarySnapshot.methodAgents[0]!,
        agentCode: `agent_${index}`,
      })),
    }],
    ["agent menus", {
      ...bmadLibrarySnapshot,
      methodAgents: [{
        ...bmadLibrarySnapshot.methodAgents[0]!,
        menus: Array.from({ length: 33 }, (_, index) => ({
          ...bmadLibrarySnapshot.methodAgents[0]!.menus[0]!,
          code: `M_${index}`,
        })),
      }],
    }],
    ["skill actions", {
      ...bmadLibrarySnapshot,
      installedSkills: [{
        ...bmadLibrarySnapshot.installedSkills[0]!,
        actions: Array.from({ length: 17 }, (_, index) => `action_${index}`),
      }],
    }],
    ["expected artifacts", {
      ...bmadLibrarySnapshot,
      helpActions: [{
        ...bmadLibrarySnapshot.helpActions[0]!,
        expectedArtifacts: Array.from({ length: 17 }, (_, index) => `artifact ${index}`),
      }],
    }],
  ])("rejects a Method library exceeding the native %s bound", async (_bound, value) => {
    const { client } = await createBmadReplyClient({
      kind: "bmad_library_snapshot",
      value,
    });
    await expect(client.bmadLibrarySnapshot()).rejects.toBeInstanceOf(HostProtocolError);
  });

  it.each([
    ["skill identity", {
      ...bmadLibrarySnapshot,
      installedSkills: [
        bmadLibrarySnapshot.installedSkills[0]!,
        { ...bmadLibrarySnapshot.installedSkills[0]! },
      ],
    }],
    ["help-action identity", {
      ...bmadLibrarySnapshot,
      helpActions: [
        bmadLibrarySnapshot.helpActions[0]!,
        { ...bmadLibrarySnapshot.helpActions[0]! },
      ],
    }],
    ["help-action menu alias within a module", {
      ...bmadLibrarySnapshot,
      helpActions: [
        bmadLibrarySnapshot.helpActions[0]!,
        {
          ...bmadLibrarySnapshot.helpActions[0]!,
          action: "review",
          skillName: "bmad-architecture-review",
        },
      ],
    }],
    ["agent identity", {
      ...bmadLibrarySnapshot,
      methodAgents: [
        bmadLibrarySnapshot.methodAgents[0]!,
        { ...bmadLibrarySnapshot.methodAgents[0]! },
      ],
    }],
    ["menu identity within one agent", {
      ...bmadLibrarySnapshot,
      methodAgents: [{
        ...bmadLibrarySnapshot.methodAgents[0]!,
        menus: [
          bmadLibrarySnapshot.methodAgents[0]!.menus[0]!,
          { ...bmadLibrarySnapshot.methodAgents[0]!.menus[0]! },
        ],
      }],
    }],
  ])("rejects a duplicate composite %s", async (_identity, value) => {
    const { client } = await createBmadReplyClient({
      kind: "bmad_library_snapshot",
      value,
    });
    await expect(client.bmadLibrarySnapshot()).rejects.toBeInstanceOf(HostProtocolError);
  });

  it("allows the same menu code on different agents", async () => {
    const secondAgent = {
      ...bmadLibrarySnapshot.methodAgents[0]!,
      agentCode: "bmad-agent-second",
      name: "Second agent",
    };
    const value = {
      ...bmadLibrarySnapshot,
      methodAgents: [bmadLibrarySnapshot.methodAgents[0]!, secondAgent],
    };
    const { client } = await createBmadReplyClient({
      kind: "bmad_library_snapshot",
      value,
    });

    await expect(client.bmadLibrarySnapshot()).resolves.toEqual(value);
  });

  it.each([
    ["snapshot schema", { ...bmadLibrarySnapshot, schemaVersion: "bmad-library-snapshot.v2" }],
    ["projection scope", { ...bmadLibrarySnapshot, scope: "workspace" }],
    ["source kind", {
      ...bmadLibrarySnapshot,
      source: { ...bmadLibrarySnapshot.source, sourceKind: "downloaded_package" },
    }],
    ["entrypoint kind", {
      ...bmadLibrarySnapshot,
      installedSkills: [{
        ...bmadLibrarySnapshot.installedSkills[0]!,
        entrypointKind: "host_script",
      }],
    }],
    ["availability", {
      ...bmadLibrarySnapshot,
      installedSkills: [{
        ...bmadLibrarySnapshot.installedSkills[0]!,
        availability: "future_state",
      }],
    }],
    ["blocker code", {
      ...bmadLibrarySnapshot,
      helpActions: [{
        ...bmadLibrarySnapshot.helpActions[0]!,
        blockerCodes: ["internal_path_unavailable"],
      }],
    }],
    ["duplicate blocker code", {
      ...bmadLibrarySnapshot,
      helpActions: [{
        ...bmadLibrarySnapshot.helpActions[0]!,
        blockerCodes: ["bmad_capability_disabled", "bmad_capability_disabled"],
      }],
    }],
    ["menu target kind", {
      ...bmadLibrarySnapshot,
      methodAgents: [{
        ...bmadLibrarySnapshot.methodAgents[0]!,
        menus: [{
          ...bmadLibrarySnapshot.methodAgents[0]!.menus[0]!,
          targetKind: "prompt_body",
        }],
      }],
    }],
    ["availability reason", {
      ...bmadLibrarySnapshot,
      methodAgents: [{
        ...bmadLibrarySnapshot.methodAgents[0]!,
        menus: [{
          ...bmadLibrarySnapshot.methodAgents[0]!.menus[0]!,
          availabilityReason: "internal_path_unavailable",
        }],
      }],
    }],
    ["response cursor", { ...bmadLibrarySnapshot, nextCursor: "contains space" }],
    ["identifier text", {
      ...bmadLibrarySnapshot,
      installedSkills: [{
        ...bmadLibrarySnapshot.installedSkills[0]!,
        moduleCode: "not allowed",
      }],
    }],
    ["multi-byte source text", {
      ...bmadLibrarySnapshot,
      source: { ...bmadLibrarySnapshot.source, packageName: "é".repeat(129) },
    }],
    ["description text", {
      ...bmadLibrarySnapshot,
      helpActions: [{
        ...bmadLibrarySnapshot.helpActions[0]!,
        description: "unsafe\u202Etext",
      }],
    }],
    ["unpaired surrogate text", {
      ...bmadLibrarySnapshot,
      installedSkills: [{
        ...bmadLibrarySnapshot.installedSkills[0]!,
        displayName: "unsafe\uD800text",
      }],
    }],
    ["description byte bound", {
      ...bmadLibrarySnapshot,
      methodAgents: [{
        ...bmadLibrarySnapshot.methodAgents[0]!,
        description: "x".repeat(2_049),
      }],
    }],
    ["icon byte bound", {
      ...bmadLibrarySnapshot,
      methodAgents: [{
        ...bmadLibrarySnapshot.methodAgents[0]!,
        icon: "x".repeat(65),
      }],
    }],
  ])("rejects an invalid or unsafe BMAD %s", async (_field, value) => {
    const { client } = await createBmadReplyClient({
      kind: "bmad_library_snapshot",
      value,
    });
    await expect(client.bmadLibrarySnapshot()).rejects.toBeInstanceOf(HostProtocolError);
  });

  it.each([
    ["bmad_projection_unavailable", "request_bmad_error"],
    ["bmad_projection_gap", "request_bmad_error"],
    ["renderer_session_expired", null],
  ] as const)("surfaces the native local error code %s", async (code, replyRequestId) => {
    const invoke = vi.fn<TauriInvoke>(async (command) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      return {
        schemaVersion: "desktop-dispatch-reply.v1",
        requestId: replyRequestId,
        sequence: 12,
        status: "error",
        error: {
          code,
          safeMessage: "The Method library request is unavailable. Retry from the desktop.",
          retryable: true,
          correlationId: replyRequestId,
        },
      };
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => "request_bmad_error",
    });
    await client.bootstrap();

    await expect(client.bmadLibrarySnapshot()).rejects.toMatchObject({
      name: HostCommandError.name,
      details: { code },
    });
  });

  it("accepts the exact BMAD library invalidation event", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId: readyBootstrap.rendererSessionId,
        status: "events",
        events: [{
          sequence: 13,
          occurredAt: 1_725_000_000_010,
          event: {
            type: "bmad.projection_changed",
            projection: { scope: "library" },
          },
        }],
      };
    });
    const client = new DesktopHostClient({ invoke });
    await client.bootstrap();

    await expect(client.projectionEvents(12)).resolves.toEqual([{
      sequence: 13,
      occurredAt: 1_725_000_000_010,
      event: {
        type: "bmad.projection_changed",
        projection: { scope: "library" },
      },
    }]);
  });

  it.each([
    { scope: "workspace" },
    { scope: "library", owner: "must-not-cross-ipc" },
  ])("rejects an invalid BMAD library invalidation payload %j", async (projection) => {
    const invoke = vi.fn<TauriInvoke>(async (command) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId: readyBootstrap.rendererSessionId,
        status: "events",
        events: [{
          sequence: 13,
          occurredAt: 1_725_000_000_010,
          event: { type: "bmad.projection_changed", projection },
        }],
      };
    });
    const client = new DesktopHostClient({ invoke });
    await client.bootstrap();

    await expect(client.projectionEvents(12)).rejects.toBeInstanceOf(HostProtocolError);
  });

  it("constructs the exact renderer-bound command envelope", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      const body = JSON.parse(String(args?.body)) as { requestId: string };
      return {
        schemaVersion: "desktop-dispatch-reply.v1",
        requestId: body.requestId,
        sequence: 13,
        status: "ok",
        receipt: {
          requestId: body.requestId,
          acceptedAt: 1_725_000_000_005,
          operationId: null,
        },
        data: { kind: "no_selection" },
      };
    });
    const client = new DesktopHostClient({
      invoke,
      now: () => 1_725_000_000_000,
      requestId: () => "request_01K0Q6H3",
    });

    await client.bootstrap();
    await client.selectWorkspace();

    expect(invoke).toHaveBeenNthCalledWith(1, "host_bootstrap");
    expect(invoke).toHaveBeenNthCalledWith(2, "host_dispatch", {
      body: JSON.stringify({
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_01K0Q6H3",
        command: "workspace.select_folder",
        windowLabel: "main",
        rendererSessionId: "renderer_01K0Q6H3",
        installationId: "install_01K0Q6H3",
        issuedAt: 1_725_000_000_000,
        payload: {},
      }),
    });
  });

  it("revokes only the exact current grant and drops its traversal state after validation", async () => {
    let request = 0;
    const workspace = d1Bootstrap.workspaces[0]!;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as {
        command: string;
        requestId: string;
      };
      if (envelope.command === "workspace.list_entries") {
        return successfulReply(envelope.requestId, {
          kind: "workspace_entries",
          value: {
            workspaceId: workspace.workspaceId,
            entries: [{
              relativePath: "README.md",
              kind: "text_file",
              sizeBytes: 10,
              childCursor: null,
            }],
            nextCursor: firstCursor,
          },
        });
      }
      return successfulReply(envelope.requestId, {
        kind: "workspace_revoked",
        value: { ...workspace, grantEpoch: workspace.grantEpoch + 1 },
      }, 13);
    });
    const client = new DesktopHostClient({
      invoke,
      now: () => 1_725_000_000_000,
      requestId: () => `request_revoke_${request += 1}`,
    });
    await client.bootstrap();
    const root = await client.listWorkspaceEntries(workspace.workspaceId);

    const result = await client.revokeWorkspace(workspace);

    expect(result).toEqual({
      revoked: { ...workspace, grantEpoch: workspace.grantEpoch + 1 },
      workspaces: [],
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "host_dispatch", {
      body: JSON.stringify({
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_revoke_2",
        command: "workspace.revoke",
        windowLabel: d1Bootstrap.windowLabel,
        rendererSessionId: d1Bootstrap.rendererSessionId,
        installationId: d1Bootstrap.installationId,
        issuedAt: 1_725_000_000_000,
        payload: { workspaceId: workspace.workspaceId },
      }),
    });
    await expect(client.listWorkspaceEntries(workspace.workspaceId, root.nextCursor))
      .rejects.toThrow(/capability is unavailable/i);
    await expect(client.revokeWorkspace(workspace))
      .rejects.toThrow(/workspace access is no longer available/i);
    expect(invoke).toHaveBeenCalledTimes(3);
  });

  it("does not let an in-flight Explorer page restore traversal after revocation", async () => {
    const workspace = d1Bootstrap.workspaces[0]!;
    let request = 0;
    let resolvePage!: (value: unknown) => void;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as {
        command: string;
        payload: { cursor?: string | null };
        requestId: string;
      };
      if (envelope.command === "workspace.revoke") {
        return successfulReply(envelope.requestId, {
          kind: "workspace_revoked",
          value: { ...workspace, grantEpoch: workspace.grantEpoch + 1 },
        }, 13);
      }
      if (envelope.payload.cursor === null) {
        return successfulReply(envelope.requestId, {
          kind: "workspace_entries",
          value: {
            workspaceId: workspace.workspaceId,
            entries: [{
              relativePath: "src",
              kind: "directory",
              sizeBytes: 0,
              childCursor: firstCursor,
            }],
            nextCursor: null,
          },
        });
      }
      return new Promise((resolve) => {
        resolvePage = resolve;
      });
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => `request_revoke_race_${request += 1}`,
    });
    await client.bootstrap();
    const root = await client.listWorkspaceEntries(workspace.workspaceId);
    const pendingPage = client.listWorkspaceEntries(
      workspace.workspaceId,
      root.entries[0]!.childCursor,
    );
    await Promise.resolve();

    await client.revokeWorkspace(workspace);
    resolvePage(successfulReply("request_revoke_race_2", {
      kind: "workspace_entries",
      value: {
        workspaceId: workspace.workspaceId,
        entries: [{
          relativePath: "src/index.ts",
          kind: "text_file",
          sizeBytes: 10,
          childCursor: null,
        }],
        nextCursor: secondCursor,
      },
    }, 13));

    await expect(pendingPage).rejects.toThrow(/capability is unavailable/i);
  });

  it("does not return an in-flight text projection after the host enters recovery", async () => {
    const workspace = d1Bootstrap.workspaces[0]!;
    let readRequestId = "";
    let resolveRead!: (value: unknown) => void;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      if (command === "host_projection_events") {
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId: d1Bootstrap.rendererSessionId,
          status: "events",
          events: [{
            sequence: 13,
            occurredAt: 1_725_000_000_010,
            event: {
              type: "boot_state_changed",
              projection: { mode: "read_only_recovery" },
            },
          }],
        };
      }
      readRequestId = (JSON.parse(String(args?.body)) as { requestId: string }).requestId;
      return new Promise((resolve) => {
        resolveRead = resolve;
      });
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => "request_read_recovery",
    });
    await client.bootstrap();
    const pendingRead = client.readWorkspaceText(workspace.workspaceId, "README.md");
    await Promise.resolve();

    await client.projectionEvents(12);
    resolveRead(successfulReply(readRequestId, {
      kind: "workspace_text",
      value: {
        relativePath: "README.md",
        content: "safe\n",
        contentHash: digestA,
        byteCount: 5,
        truncated: false,
      },
    }, 13));

    await expect(pendingRead).rejects.toThrow(/capability is unavailable/i);
  });

  it("does not accept an in-flight projection across a same-binding re-bootstrap", async () => {
    const workspace = d1Bootstrap.workspaces[0]!;
    let readRequestId = "";
    let resolveRead!: (value: unknown) => void;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      readRequestId = (JSON.parse(String(args?.body)) as { requestId: string }).requestId;
      return new Promise((resolve) => {
        resolveRead = resolve;
      });
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => "request_read_rebootstrap",
    });
    await client.bootstrap();
    const pendingRead = client.readWorkspaceText(workspace.workspaceId, "README.md");
    await Promise.resolve();

    await client.bootstrap();
    resolveRead(successfulReply(readRequestId, {
      kind: "workspace_text",
      value: {
        relativePath: "README.md",
        content: "safe\n",
        contentHash: digestA,
        byteCount: 5,
        truncated: false,
      },
    }, 13));

    await expect(pendingRead).rejects.toBeInstanceOf(HostProtocolError);
  });

  it("blocks new dispatches while a re-bootstrap is pending", async () => {
    let bootstrapCall = 0;
    let resolveRebootstrap!: (value: unknown) => void;
    const invoke = vi.fn<TauriInvoke>(async (command) => {
      if (command !== "host_bootstrap") {
        throw new Error("A pending bootstrap must make dispatch unreachable.");
      }
      bootstrapCall += 1;
      if (bootstrapCall === 1) {
        return d1Bootstrap;
      }
      return new Promise((resolve) => {
        resolveRebootstrap = resolve;
      });
    });
    const client = new DesktopHostClient({ invoke });
    await client.bootstrap();

    const pendingBootstrap = client.bootstrap();
    await Promise.resolve();

    await expect(client.listWorkspaces()).rejects.toThrow(/has not completed bootstrap/i);
    expect(invoke).toHaveBeenCalledTimes(2);

    resolveRebootstrap(d1Bootstrap);
    await expect(pendingBootstrap).resolves.toEqual(d1Bootstrap);
  });

  it("keeps the client unavailable after a failed re-bootstrap and rejects an older in-flight reply", async () => {
    const workspace = d1Bootstrap.workspaces[0]!;
    let bootstrapCall = 0;
    let readRequestId = "";
    let resolveRead!: (value: unknown) => void;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        bootstrapCall += 1;
        if (bootstrapCall === 1) {
          return d1Bootstrap;
        }
        throw new Error("The replacement bootstrap failed.");
      }
      readRequestId = (JSON.parse(String(args?.body)) as { requestId: string }).requestId;
      return new Promise((resolve) => {
        resolveRead = resolve;
      });
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => "request_failed_rebootstrap",
    });
    await client.bootstrap();
    const pendingRead = client.readWorkspaceText(workspace.workspaceId, "README.md");
    await Promise.resolve();

    await expect(client.bootstrap()).rejects.toThrow("The replacement bootstrap failed.");
    resolveRead(successfulReply(readRequestId, {
      kind: "workspace_text",
      value: {
        relativePath: "README.md",
        content: "safe\n",
        contentHash: digestA,
        byteCount: 5,
        truncated: false,
      },
    }, 13));

    await expect(pendingRead).rejects.toBeInstanceOf(HostProtocolError);
    await expect(client.listWorkspaces()).rejects.toThrow(/has not completed bootstrap/i);
    expect(invoke.mock.calls.filter(([command]) => command === "host_dispatch")).toHaveLength(1);
  });

  it("keeps the newest concurrent bootstrap when an older attempt completes later", async () => {
    const olderBootstrap: BootstrapReply = {
      ...d1Bootstrap,
      rendererSessionId: "renderer_01K0Q6H4",
      installationId: "install_01K0Q6H4",
      projectionSequence: 19,
      workspaces: [{
        ...d1Bootstrap.workspaces[0]!,
        displayName: "older-workspace",
      }],
    };
    const newestBootstrap: BootstrapReply = {
      ...d1Bootstrap,
      rendererSessionId: "renderer_01K0Q6H5",
      installationId: "install_01K0Q6H5",
      projectionSequence: 20,
      workspaces: [{
        ...d1Bootstrap.workspaces[0]!,
        displayName: "newest-workspace",
      }],
    };
    let bootstrapCall = 0;
    let resolveOlder!: (value: unknown) => void;
    let resolveNewest!: (value: unknown) => void;
    let dispatchedBinding: { installationId: string; rendererSessionId: string } | null = null;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        bootstrapCall += 1;
        return new Promise((resolve) => {
          if (bootstrapCall === 1) {
            resolveOlder = resolve;
          } else {
            resolveNewest = resolve;
          }
        });
      }
      const envelope = JSON.parse(String(args?.body)) as {
        installationId: string;
        rendererSessionId: string;
        requestId: string;
      };
      dispatchedBinding = envelope;
      return successfulReply(envelope.requestId, {
        kind: "workspace_list",
        value: newestBootstrap.workspaces,
      }, newestBootstrap.projectionSequence);
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => "request_newest_bootstrap",
    });

    const olderAttempt = client.bootstrap();
    const newestAttempt = client.bootstrap();
    resolveNewest(newestBootstrap);
    await expect(newestAttempt).resolves.toEqual(newestBootstrap);
    resolveOlder(olderBootstrap);
    await expect(olderAttempt).rejects.toBeInstanceOf(HostProtocolError);

    await expect(client.listWorkspaces()).resolves.toEqual(newestBootstrap.workspaces);
    expect(dispatchedBinding).toMatchObject({
      rendererSessionId: newestBootstrap.rendererSessionId,
      installationId: newestBootstrap.installationId,
    });
  });

  it.each([
    ["replayed grant epoch", { grantEpoch: d1Bootstrap.workspaces[0]!.grantEpoch }],
    ["skipped grant epoch", { grantEpoch: d1Bootstrap.workspaces[0]!.grantEpoch + 2 }],
    ["workspace identity drift", { workspaceId: "workspace_01K0Q6H4" }],
    ["project identity drift", { projectId: "project_01K0Q6H4" }],
    ["display identity drift", { displayName: "another-workspace" }],
  ])("rejects %s in a revoke reply without consuming the local grant", async (_case, drift) => {
    const workspace = d1Bootstrap.workspaces[0]!;
    let dispatch = 0;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      dispatch += 1;
      return successfulReply(envelope.requestId, {
        kind: "workspace_revoked",
        value: dispatch === 1
          ? { ...workspace, grantEpoch: workspace.grantEpoch + 1, ...drift }
          : { ...workspace, grantEpoch: workspace.grantEpoch + 1 },
      }, 13);
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => `request_revoke_drift_${dispatch + 1}`,
    });
    await client.bootstrap();

    await expect(client.revokeWorkspace(workspace)).rejects.toBeInstanceOf(HostProtocolError);
    await expect(client.revokeWorkspace(workspace)).resolves.toMatchObject({ workspaces: [] });
    expect(dispatch).toBe(2);
  });

  it("rejects a non-advancing revoke reply without consuming the local grant", async () => {
    const workspace = d1Bootstrap.workspaces[0]!;
    let dispatch = 0;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      dispatch += 1;
      return successfulReply(envelope.requestId, {
        kind: "workspace_revoked",
        value: { ...workspace, grantEpoch: workspace.grantEpoch + 1 },
      }, dispatch === 1 ? d1Bootstrap.projectionSequence : d1Bootstrap.projectionSequence + 1);
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => `request_revoke_sequence_${dispatch + 1}`,
    });
    await client.bootstrap();

    await expect(client.revokeWorkspace(workspace)).rejects.toBeInstanceOf(HostProtocolError);
    await expect(client.revokeWorkspace(workspace)).resolves.toMatchObject({ workspaces: [] });
    expect(dispatch).toBe(2);
  });

  it("accepts a valid revoke reply when its event sequence was observed concurrently", async () => {
    const workspace = d1Bootstrap.workspaces[0]!;
    let resolveRevoke!: (value: unknown) => void;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      if (command === "host_projection_events") {
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId: d1Bootstrap.rendererSessionId,
          status: "events",
          events: [{
            sequence: 13,
            occurredAt: 1_725_000_000_010,
            event: {
              type: "workspace_changed",
              projection: { workspaceId: workspace.workspaceId },
            },
          }],
        };
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return new Promise((resolve) => {
        resolveRevoke = resolve;
      }).then(() => successfulReply(envelope.requestId, {
        kind: "workspace_revoked",
        value: { ...workspace, grantEpoch: workspace.grantEpoch + 1 },
      }, 13));
    });
    const client = new DesktopHostClient({
      invoke,
      requestId: () => "request_revoke_concurrent_event",
    });
    await client.bootstrap();
    const revocation = client.revokeWorkspace(workspace);
    await Promise.resolve();

    await client.projectionEvents(12);
    resolveRevoke(undefined);

    await expect(revocation).resolves.toMatchObject({ workspaces: [] });
  });

  it("validates every D1 read projection and emits only the narrow command payloads", async () => {
    let requestIndex = 0;
    const contextContent = "export const safe = true;\n";
    const contextBytes = new TextEncoder().encode(contextContent).byteLength;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as {
        requestId: string;
        command: string;
        payload: Record<string, unknown>;
      };
      switch (envelope.command) {
        case "workspace.list_entries":
          return successfulReply(envelope.requestId, {
            kind: "workspace_entries",
            value: envelope.payload.cursor === null ? {
              workspaceId: d1Bootstrap.workspaces[0]!.workspaceId,
              entries: [{
                relativePath: "src",
                kind: "directory",
                sizeBytes: 0,
                childCursor: firstCursor,
              }],
              nextCursor: secondCursor,
            } : {
              workspaceId: d1Bootstrap.workspaces[0]!.workspaceId,
              entries: [{
                relativePath: "src/index.ts",
                kind: "text_file",
                sizeBytes: contextBytes,
                childCursor: null,
              }],
              nextCursor: null,
            },
          });
        case "workspace.read_text":
          return successfulReply(envelope.requestId, {
            kind: "workspace_text",
            value: {
              relativePath: "src/index.ts",
              content: contextContent,
              contentHash: digestA,
              byteCount: contextBytes,
              truncated: false,
            },
          });
        case "workspace.search":
          return successfulReply(envelope.requestId, {
            kind: "search_results",
            value: [{ relativePath: "src/index.ts", line: 1, preview: "export const safe = true;" }],
          });
        case "bmad.scan":
          return successfulReply(envelope.requestId, {
            kind: "bmad_scan",
            value: {
              status: "method_and_builder_drafts_detected",
              assets: [
                {
                  relativePath: "_bmad/agents/reviewer.md",
                  assetKind: "agent",
                  activation: "read_only",
                },
                {
                  relativePath: "_bmad-output/build/draft.md",
                  assetKind: "builder_build_draft",
                  activation: "inactive_draft",
                },
              ],
              truncated: false,
            },
          });
        case "context.preview":
          return successfulReply(envelope.requestId, {
            kind: "context_preview",
            value: {
              workspaceId: d1Bootstrap.workspaces[0]!.workspaceId,
              manifestHash: digestB,
              items: [{
                relativePath: "src/index.ts",
                startLine: 1,
                endLine: 1,
                reason: "Selected for this task",
                contentHash: digestA,
                classification: "source",
                redactions: [],
                byteCount: contextBytes,
                estimatedTokens: Math.floor((contextBytes + 3) / 4),
                content: contextContent,
              }],
              totalBytes: contextBytes,
              estimatedTokens: Math.floor((contextBytes + 3) / 4),
              modelTarget: null,
            },
          });
        default:
          throw new Error(`Unexpected command: ${envelope.command}`);
      }
    });
    const client = new DesktopHostClient({
      invoke,
      now: () => 1_725_000_000_000,
      requestId: () => `request_d1_${requestIndex += 1}`,
    });
    const workspaceId = d1Bootstrap.workspaces[0]!.workspaceId;
    await client.bootstrap();

    const root = await client.listWorkspaceEntries(workspaceId, null, 80);
    const child = await client.listWorkspaceEntries(workspaceId, root.entries[0]!.childCursor, 40);
    await expect(client.listWorkspaceEntries(workspaceId, root.entries[0]!.childCursor))
      .rejects.toThrow(/cursor is unavailable/i);
    const text = await client.readWorkspaceText(workspaceId, "src/index.ts", 128 * 1024);
    const matches = await client.searchWorkspace(workspaceId, "safe", 25);
    const bmad = await client.scanBmad(workspaceId);
    const context = await client.previewContext(workspaceId, ["src/index.ts"]);

    expect(child.entries[0]?.relativePath).toBe("src/index.ts");
    expect(text.contentHash).toBe(digestA);
    expect(matches[0]?.line).toBe(1);
    expect(bmad.assets[1]?.activation).toBe("inactive_draft");
    expect(context.modelTarget).toBeNull();
    const envelopes = invoke.mock.calls.slice(1).map(([, args]) =>
      JSON.parse(String(args?.body)) as { command: string; payload: unknown }
    );
    expect(envelopes).toEqual([
      {
        command: "workspace.list_entries",
        payload: { workspaceId, cursor: null, limit: 80 },
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_d1_1",
        windowLabel: "main",
        rendererSessionId: d1Bootstrap.rendererSessionId,
        installationId: d1Bootstrap.installationId,
        issuedAt: 1_725_000_000_000,
      },
      {
        command: "workspace.list_entries",
        payload: { workspaceId, cursor: firstCursor, limit: 40 },
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_d1_2",
        windowLabel: "main",
        rendererSessionId: d1Bootstrap.rendererSessionId,
        installationId: d1Bootstrap.installationId,
        issuedAt: 1_725_000_000_000,
      },
      {
        command: "workspace.read_text",
        payload: { workspaceId, relativePath: "src/index.ts", maxBytes: 131072 },
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_d1_3",
        windowLabel: "main",
        rendererSessionId: d1Bootstrap.rendererSessionId,
        installationId: d1Bootstrap.installationId,
        issuedAt: 1_725_000_000_000,
      },
      {
        command: "workspace.search",
        payload: { workspaceId, query: "safe", maxResults: 25 },
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_d1_4",
        windowLabel: "main",
        rendererSessionId: d1Bootstrap.rendererSessionId,
        installationId: d1Bootstrap.installationId,
        issuedAt: 1_725_000_000_000,
      },
      {
        command: "bmad.scan",
        payload: { workspaceId },
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_d1_5",
        windowLabel: "main",
        rendererSessionId: d1Bootstrap.rendererSessionId,
        installationId: d1Bootstrap.installationId,
        issuedAt: 1_725_000_000_000,
      },
      {
        command: "context.preview",
        payload: { workspaceId, relativePaths: ["src/index.ts"] },
        schemaVersion: "desktop-ipc-command.v1",
        requestId: "request_d1_6",
        windowLabel: "main",
        rendererSessionId: d1Bootstrap.rendererSessionId,
        installationId: d1Bootstrap.installationId,
        issuedAt: 1_725_000_000_000,
      },
    ]);
  });

  it.each([
    "/root/source.ts",
    "C:/private/source.ts",
    "src\\source.ts",
    "../source.ts",
    "src/../source.ts",
    "src//source.ts",
    "src/source.ts ",
    "CON.txt",
    "src/CON .txt",
    "src/CLOCK$.log",
    "src/conin$.txt",
    "src/CONOUT$",
    "src/COM¹.txt",
    "src/LPT³.log",
    "src/source?.ts",
    "src/source*.ts",
    "src/<source>.ts",
    "src/source\"draft.ts",
    "src/source|draft.ts",
    "src/\u202Eevil.ts",
  ])("rejects adversarial projected relative path %s", async (relativePath) => {
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return successfulReply(envelope.requestId, {
        kind: "workspace_entries",
        value: {
          workspaceId: d1Bootstrap.workspaces[0]!.workspaceId,
          entries: [{
            relativePath,
            kind: "text_file",
            sizeBytes: 10,
            childCursor: null,
          }],
          nextCursor: null,
        },
      });
    });
    const client = new DesktopHostClient({ invoke, requestId: () => "request_path_guard" });
    await client.bootstrap();

    await expect(client.listWorkspaceEntries(d1Bootstrap.workspaces[0]!.workspaceId))
      .rejects.toBeInstanceOf(HostProtocolError);
  });

  it("rejects a case-colliding path repeated across Explorer pages", async () => {
    const workspaceId = d1Bootstrap.workspaces[0]!.workspaceId;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as {
        requestId: string;
        payload: { cursor: string | null };
      };
      return successfulReply(envelope.requestId, {
        kind: "workspace_entries",
        value: {
          workspaceId,
          entries: [{
            relativePath: envelope.payload.cursor === null ? "README.md" : "readme.md",
            kind: "text_file",
            sizeBytes: 10,
            childCursor: null,
          }],
          nextCursor: envelope.payload.cursor === null ? firstCursor : null,
        },
      });
    });
    const client = new DesktopHostClient({ invoke, requestId: () => "request_page_guard" });
    await client.bootstrap();

    const firstPage = await client.listWorkspaceEntries(workspaceId);
    await expect(client.listWorkspaceEntries(workspaceId, firstPage.nextCursor))
      .rejects.toBeInstanceOf(HostProtocolError);
  });

  it("rejects Unicode format controls in search line previews", async () => {
    const workspaceId = d1Bootstrap.workspaces[0]!.workspaceId;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return successfulReply(envelope.requestId, {
        kind: "search_results",
        value: [{
          relativePath: "README.md",
          line: 1,
          preview: "safe prefix \u202Etxt.exe",
        }],
      });
    });
    const client = new DesktopHostClient({ invoke, requestId: () => "request_search_guard" });
    await client.bootstrap();

    await expect(client.searchWorkspace(workspaceId, "safe"))
      .rejects.toBeInstanceOf(HostProtocolError);
  });

  it("rejects cursor reuse, unknown D2 events, and inconsistent D1 discriminators", async () => {
    const workspaceId = d1Bootstrap.workspaces[0]!.workspaceId;
    const client = new DesktopHostClient({
      invoke: async (command, args) => {
        if (command === "host_bootstrap") {
          return d1Bootstrap;
        }
        if (command === "host_projection_events") {
          return {
            schemaVersion: "desktop-projection-reply.v1",
            rendererSessionId: d1Bootstrap.rendererSessionId,
            status: "events",
            events: [{
              sequence: 13,
              occurredAt: 1_725_000_000_010,
              event: {
                type: "model_response_received",
                projection: { responseId: "response_01K0Q6H3" },
              },
            }],
          };
        }
        const envelope = JSON.parse(String(args?.body)) as { requestId: string };
        return successfulReply(envelope.requestId, {
          kind: "bmad_scan",
          value: {
            status: "method_detected",
            assets: [{
              relativePath: "_bmad/build/draft.md",
              assetKind: "builder_build_draft",
              activation: "read_only",
            }],
            truncated: false,
          },
        });
      },
      requestId: () => "request_discriminator_guard",
    });
    await client.bootstrap();

    await expect(client.listWorkspaceEntries(workspaceId, firstCursor))
      .rejects.toThrow(/cursor is unavailable/i);
    await expect(client.projectionEvents(12)).rejects.toBeInstanceOf(HostProtocolError);
    await expect(client.scanBmad(workspaceId)).rejects.toBeInstanceOf(HostProtocolError);
  });

  it("rejects context projection drift and any D2 model target", async () => {
    const workspaceId = d1Bootstrap.workspaces[0]!.workspaceId;
    const content = "safe\n";
    const bytes = new TextEncoder().encode(content).byteLength;
    const invoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return d1Bootstrap;
      }
      const envelope = JSON.parse(String(args?.body)) as { requestId: string };
      return successfulReply(envelope.requestId, {
        kind: "context_preview",
        value: {
          workspaceId,
          manifestHash: digestA,
          items: [{
            relativePath: "README.md",
            startLine: 1,
            endLine: 1,
            reason: "Selected for this task",
            contentHash: digestB,
            classification: "source",
            redactions: [],
            byteCount: bytes,
            estimatedTokens: Math.floor((bytes + 3) / 4),
            content,
          }],
          totalBytes: bytes,
          estimatedTokens: Math.floor((bytes + 3) / 4),
          modelTarget: {
            model: "not-available-in-d1",
            deployment: "not-available-in-d1",
            region: "not-available-in-d1",
            retention: "not-available-in-d1",
            schemaHash: digestA,
            profileHash: digestB,
          },
        },
      });
    });
    const client = new DesktopHostClient({ invoke, requestId: () => "request_context_guard" });
    await client.bootstrap();

    await expect(client.previewContext(workspaceId, ["README.md"]))
      .rejects.toBeInstanceOf(HostProtocolError);
  });

  it("binds projection snapshot and event requests to the bootstrapped renderer", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      if (command === "host_projection_snapshot") {
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId: readyBootstrap.rendererSessionId,
          status: "snapshot",
          snapshot: {
            sequence: 12,
            generatedAt: 1_725_000_000_000,
            bootMode: "ready",
            workspaceCount: 1,
            activeSessionId: null,
          },
        };
      }
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId: readyBootstrap.rendererSessionId,
        status: "events",
        events: [{
          sequence: 13,
          occurredAt: 1_725_000_000_010,
          event: {
            type: "workspace_changed",
            projection: { workspaceId: "workspace_01K0Q6H3" },
          },
        }],
      };
    });
    const client = new DesktopHostClient({ invoke });
    await client.bootstrap();

    await client.projectionSnapshot({ workspaceId: "workspace_01K0Q6H3" });
    await client.projectionEvents(12, { workspaceId: "workspace_01K0Q6H3" });

    expect(invoke).toHaveBeenNthCalledWith(2, "host_projection_snapshot", {
      body: JSON.stringify({
        schemaVersion: "desktop-projection-request.v1",
        rendererSessionId: readyBootstrap.rendererSessionId,
        installationId: readyBootstrap.installationId,
        workspaceId: "workspace_01K0Q6H3",
        sessionId: null,
        afterSequence: null,
      }),
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "host_projection_events", {
      body: JSON.stringify({
        schemaVersion: "desktop-projection-request.v1",
        rendererSessionId: readyBootstrap.rendererSessionId,
        installationId: readyBootstrap.installationId,
        workspaceId: "workspace_01K0Q6H3",
        sessionId: null,
        afterSequence: 12,
      }),
    });
  });

  it("rejects unknown bootstrap fields and path-shaped workspace labels", () => {
    expect(() => parseBootstrapReply({ ...readyBootstrap, accessToken: "must-not-cross-ipc" }))
      .toThrow(HostProtocolError);
    expect(() => parseBootstrapReply({
      ...readyBootstrap,
      workspaces: [{ ...readyBootstrap.workspaces[0], displayName: "C:\\private\\source" }],
    })).toThrow(HostProtocolError);
    expect(() => parseBootstrapReply({
      ...readyBootstrap,
      workspaces: [{ ...readyBootstrap.workspaces[0], permissions: "unrestricted" }],
    })).toThrow(HostProtocolError);
    expect(() => parseBootstrapReply({
      ...readyBootstrap,
      bootMode: "read_only_recovery",
    })).toThrow(HostProtocolError);
  });

  it("rejects path-bearing errors and non-advancing projection events", async () => {
    const rootedMessages = [
      "Failed at C:\\private\\source",
      "Failed at C:/private/source",
      "Failed at \\Users\\Alice\\source",
      "Failed at \\Device\\HarddiskVolume1\\source",
      "Failed at \\\\server\\share\\source",
      "Failed at /root/source",
      "Failed at /opt/sapphirus/source",
      "Failed at //server/share/source",
      "Failed at /",
    ];
    for (const safeMessage of rootedMessages) {
      const pathInvoke = vi.fn<TauriInvoke>(async (command, args) => {
        if (command === "host_bootstrap") {
          return readyBootstrap;
        }
        const body = JSON.parse(String(args?.body)) as { requestId: string };
        return {
          schemaVersion: "desktop-dispatch-reply.v1",
          requestId: body.requestId,
          sequence: 12,
          status: "error",
          error: {
            code: "internal",
            safeMessage,
            retryable: false,
            correlationId: body.requestId,
          },
        };
      });
      const pathClient = new DesktopHostClient({
        invoke: pathInvoke,
        requestId: () => "request_01K0Q6H3",
      });
      await pathClient.bootstrap();
      await expect(pathClient.selectWorkspace()).rejects.toBeInstanceOf(HostProtocolError);
    }

    const projectionInvoke = vi.fn<TauriInvoke>(async (command) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId: readyBootstrap.rendererSessionId,
        status: "events",
        events: [{
          sequence: 12,
          occurredAt: 1_725_000_000_010,
          event: {
            type: "workspace_changed",
            projection: { workspaceId: "workspace_01K0Q6H3" },
          },
        }],
      };
    });
    const client = new DesktopHostClient({
      invoke: projectionInvoke,
      requestId: () => "request_01K0Q6H3",
    });
    await client.bootstrap();

    await expect(client.projectionEvents(12)).rejects.toBeInstanceOf(HostProtocolError);
  });

  it("rejects Unicode format controls in errors while preserving ordinary Unicode prose", async () => {
    for (const unsafeMessage of [
      "Request failed \u202Etxt.exe",
      "Request failed \u2066hidden\u2069",
      "Request failed \u200Bsilently",
    ]) {
      const invoke = vi.fn<TauriInvoke>(async (command, args) => {
        if (command === "host_bootstrap") {
          return readyBootstrap;
        }
        const body = JSON.parse(String(args?.body)) as { requestId: string };
        return {
          schemaVersion: "desktop-dispatch-reply.v1",
          requestId: body.requestId,
          sequence: 12,
          status: "error",
          error: {
            code: "internal",
            safeMessage: unsafeMessage,
            retryable: false,
            correlationId: body.requestId,
          },
        };
      });
      const client = new DesktopHostClient({
        invoke,
        requestId: () => "request_unicode_guard",
      });
      await client.bootstrap();
      await expect(client.selectWorkspace()).rejects.toBeInstanceOf(HostProtocolError);
    }

    const safeMessage = "Não foi possível concluir — tente novamente.";
    const safeInvoke = vi.fn<TauriInvoke>(async (command, args) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      const body = JSON.parse(String(args?.body)) as { requestId: string };
      return {
        schemaVersion: "desktop-dispatch-reply.v1",
        requestId: body.requestId,
        sequence: 12,
        status: "error",
        error: {
          code: "temporarily_unavailable",
          safeMessage,
          retryable: true,
          correlationId: body.requestId,
        },
      };
    });
    const safeClient = new DesktopHostClient({
      invoke: safeInvoke,
      requestId: () => "request_unicode_prose",
    });
    await safeClient.bootstrap();

    await expect(safeClient.selectWorkspace()).rejects.toMatchObject({
      name: HostCommandError.name,
      message: safeMessage,
    });
  });

  it("adopts a projected recovery transition before any later dispatch", async () => {
    const invoke = vi.fn<TauriInvoke>(async (command) => {
      if (command === "host_bootstrap") {
        return readyBootstrap;
      }
      if (command === "host_projection_events") {
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId: readyBootstrap.rendererSessionId,
          status: "events",
          events: [{
            sequence: 13,
            occurredAt: 1_725_000_000_010,
            event: {
              type: "boot_state_changed",
              projection: { mode: "read_only_recovery" },
            },
          }],
        };
      }
      throw new Error("Dispatch must remain unreachable after recovery.");
    });
    const client = new DesktopHostClient({ invoke });
    await client.bootstrap();

    await client.projectionEvents(12);
    await expect(client.selectWorkspace()).rejects.toThrow(/unavailable in the current host mode/i);
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("surfaces a validated read-only recovery bootstrap mode", async () => {
    const recoveryBootstrap = {
      ...readyBootstrap,
      bootMode: "read_only_recovery" as const,
      supportedCommands: ["app.get_boot_state", "workspace.list"] as const,
    };
    const runtime = await initializeHostRuntime({
      isTauri: () => true,
      loadInvoke: async () => async () => recoveryBootstrap,
    });

    expect(runtime.kind).toBe("read_only_recovery");
    expect(runtime.bootstrap?.bootMode).toBe("read_only_recovery");
    expect(runtime.bootstrap?.workspaces[0]?.displayName).toBe("sapphirus-desktop");
  });

  it("uses the browser demo without importing or invoking Tauri", async () => {
    const loadInvoke = vi.fn<() => Promise<TauriInvoke>>();

    const runtime = await initializeHostRuntime({
      isTauri: () => false,
      loadInvoke,
    });

    expect(runtime).toEqual({ kind: "browser_demo", client: null, bootstrap: null });
    expect(loadInvoke).not.toHaveBeenCalled();
  });
});
