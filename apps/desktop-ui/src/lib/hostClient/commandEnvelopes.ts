import {
  type ApprovalChoice,
  COMMAND_SCHEMA,
  type CommandEnvelope,
  type DensityPreference,
  type HostBinding,
  type ProposedChange,
  type RecoveryApprovalChoice,
  type RendererDispatchCommand,
  type ThemePreference,
} from "./contracts";
import {
  asContractId,
  asSha256,
  asUnsignedInteger,
  fail,
} from "./validation";

function asPositiveRecoveryEpoch(value: unknown): number {
  const epoch = asUnsignedInteger(value);
  return epoch < 1 ? fail() : epoch;
}

export function buildWorkspaceSelectionEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
): CommandEnvelope<"workspace.select_folder", Record<string, never>> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "workspace.select_folder",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {},
  };
}

export function buildWorkspaceListEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
): CommandEnvelope<"workspace.list", Record<string, never>> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "workspace.list",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {},
  };
}

export function buildEmptyPayloadEnvelope<
  TCommand extends
    | "app.preferences.get"
    | "app.about"
    | "app.offboarding.inspect",
>(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  command: TCommand,
): CommandEnvelope<TCommand, Record<string, never>> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command,
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {},
  };
}

export function buildOffboardingEraseEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  confirm: string,
): CommandEnvelope<"app.offboarding.erase", { confirm: string }> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "app.offboarding.erase",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: { confirm },
  };
}

export function buildCapabilityPrepareEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
  workspaceGrantEpoch: number,
  capabilityId: string,
  contextPaths: string[],
): CommandEnvelope<
  "bmad.capability.prepare",
  {
    workspaceId: string;
    workspaceGrantEpoch: number;
    capabilityId: string;
    contextPaths: string[];
  }
> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "bmad.capability.prepare",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {
      workspaceId: asContractId(workspaceId),
      workspaceGrantEpoch: asUnsignedInteger(workspaceGrantEpoch),
      capabilityId,
      contextPaths,
    },
  };
}

export function buildCapabilityDecisionEnvelope<
  TCommand extends
    | "bmad.capability.approve"
    | "bmad.capability.cancel"
    | "bmad.capability.submit",
>(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  command: TCommand,
  payload: TCommand extends "bmad.capability.approve"
    ? {
      workspaceId: string;
      workspaceGrantEpoch: number;
      capabilityId: string;
      manifestHash: string;
    }
    : {
      workspaceId: string;
      workspaceGrantEpoch: number;
      capabilityId: string;
      manifestHash: string;
      decisionId: string;
    },
): CommandEnvelope<TCommand, typeof payload> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command,
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload,
  };
}

export function buildCapabilityProposeChangesEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
  workspaceGrantEpoch: number,
  capabilityId: string,
): CommandEnvelope<
  "bmad.capability.propose_changes",
  { workspaceId: string; workspaceGrantEpoch: number; capabilityId: string }
> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "bmad.capability.propose_changes",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {
      workspaceId: asContractId(workspaceId),
      workspaceGrantEpoch: asUnsignedInteger(workspaceGrantEpoch),
      capabilityId,
    },
  };
}

export function buildCapabilityLatestEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
  workspaceGrantEpoch: number,
  capabilityId: string,
): CommandEnvelope<
  "bmad.capability.latest",
  { workspaceId: string; workspaceGrantEpoch: number; capabilityId: string }
> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "bmad.capability.latest",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {
      workspaceId: asContractId(workspaceId),
      workspaceGrantEpoch: asUnsignedInteger(workspaceGrantEpoch),
      capabilityId,
    },
  };
}

export function buildPreferencesUpdateEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  theme: ThemePreference,
  density: DensityPreference,
): CommandEnvelope<
  "app.preferences.set",
  { theme: ThemePreference; density: DensityPreference }
> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "app.preferences.set",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: { theme, density },
  };
}

export function buildWorkspaceFilePickEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
): CommandEnvelope<"workspace.pick_files", { workspaceId: string }> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "workspace.pick_files",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: { workspaceId: asContractId(workspaceId) },
  };
}

export function buildWorkspaceRevocationEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
): CommandEnvelope<"workspace.revoke", { workspaceId: string }> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "workspace.revoke",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: { workspaceId: asContractId(workspaceId) },
  };
}

export function buildBmadHelpRunEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
  workspaceGrantEpoch: number,
  currentIntent: string,
): CommandEnvelope<
  "run.create",
  {
    workspaceId: string;
    workspaceGrantEpoch: number;
    runKind: "bmad_help";
    currentIntent: string;
  }
> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "run.create",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {
      workspaceId: asContractId(workspaceId),
      workspaceGrantEpoch,
      runKind: "bmad_help",
      currentIntent,
    },
  };
}

export function buildLatestBmadHelpRunEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
  workspaceGrantEpoch: number,
): CommandEnvelope<
  "bmad.help.latest",
  {
    workspaceId: string;
    workspaceGrantEpoch: number;
  }
> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "bmad.help.latest",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {
      workspaceId: asContractId(workspaceId),
      workspaceGrantEpoch,
    },
  };
}

export function buildBmadModelEnvelope<
  TCommand extends
    | "model.auth.status"
    | "model.auth.sign_in"
    | "model.auth.sign_out"
    | "bmad.help.prepare"
    | "bmad.help.approve"
    | "bmad.help.cancel"
    | "bmad.help.submit",
  TPayload extends object,
>(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  command: TCommand,
  payload: TPayload,
): CommandEnvelope<TCommand, TPayload> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command,
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload,
  };
}

export function buildWorkspaceEpochEnvelope<
  TCommand extends "workspace.enable_edits" | "changes.history",
>(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  command: TCommand,
  workspaceId: string,
  workspaceGrantEpoch: number,
): CommandEnvelope<
  TCommand,
  { workspaceId: string; workspaceGrantEpoch: number }
> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command,
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {
      workspaceId: asContractId(workspaceId),
      workspaceGrantEpoch,
    },
  };
}

export function buildProposeChangesEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
  workspaceGrantEpoch: number,
  changes: readonly ProposedChange[],
): CommandEnvelope<
  "changes.propose",
  {
    workspaceId: string;
    workspaceGrantEpoch: number;
    changes: readonly ProposedChange[];
  }
> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "changes.propose",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {
      workspaceId: asContractId(workspaceId),
      workspaceGrantEpoch,
      changes,
    },
  };
}

export function buildApprovalDecisionEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  approvalId: string,
  candidateHash: string,
  displayedDiffHash: string,
  choice: ApprovalChoice,
): CommandEnvelope<
  "approval.decide",
  {
    approvalId: string;
    candidateHash: string;
    displayedDiffHash: string;
    choice: ApprovalChoice;
  }
> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "approval.decide",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {
      approvalId: asContractId(approvalId),
      candidateHash: asSha256(candidateHash),
      displayedDiffHash: asSha256(displayedDiffHash),
      choice,
    },
  };
}

export function buildRollbackRequestEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  executionId: string,
): CommandEnvelope<"rollback.request", { executionId: string }> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "rollback.request",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: { executionId: asContractId(executionId) },
  };
}

export function buildChangesRecoveryPrepareEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
  workspaceGrantEpoch: number,
  journalId: string,
): CommandEnvelope<
  "changes.recovery.prepare",
  { workspaceId: string; workspaceGrantEpoch: number; journalId: string }
> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "changes.recovery.prepare",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {
      workspaceId: asContractId(workspaceId),
      workspaceGrantEpoch: asPositiveRecoveryEpoch(workspaceGrantEpoch),
      journalId: asContractId(journalId),
    },
  };
}

export function buildChangesRecoveryDecisionEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  recoveryApprovalId: string,
  displayedRecoveryHash: string,
  choice: RecoveryApprovalChoice,
): CommandEnvelope<
  "changes.recovery.decide",
  {
    recoveryApprovalId: string;
    displayedRecoveryHash: string;
    choice: RecoveryApprovalChoice;
  }
> {
  if (choice !== "restore" && choice !== "cancel") {
    return fail();
  }
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command: "changes.recovery.decide",
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload: {
      recoveryApprovalId: asContractId(recoveryApprovalId),
      displayedRecoveryHash: asSha256(displayedRecoveryHash),
      choice,
    },
  };
}

export function buildReadOnlyEnvelope<
  TCommand extends Exclude<
    RendererDispatchCommand,
    | "workspace.select_folder"
    | "workspace.list"
    | "workspace.revoke"
    | "model.auth.status"
    | "model.auth.sign_in"
    | "model.auth.sign_out"
    | "bmad.help.prepare"
    | "bmad.help.approve"
    | "bmad.help.cancel"
    | "bmad.help.submit"
    | "bmad.help.latest"
    | "run.create"
    | "workspace.enable_edits"
    | "changes.propose"
    | "approval.decide"
    | "rollback.request"
    | "changes.history"
    | "changes.recovery.prepare"
    | "changes.recovery.decide"
  >,
>(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  command: TCommand,
  payload: TCommand extends "workspace.list_entries"
    ? { workspaceId: string; cursor: string | null; limit: number }
    : TCommand extends "workspace.read_text"
      ? { workspaceId: string; relativePath: string; maxBytes: number }
      : TCommand extends "workspace.search"
        ? { workspaceId: string; query: string; maxResults: number }
        : TCommand extends "bmad.scan"
          ? { workspaceId: string }
          : TCommand extends "bmad.library.snapshot"
            ? { scope: "installed_method"; cursor: string | null }
            : TCommand extends "bmad.persona.view"
              ? { agentCode: string }
              : { workspaceId: string; relativePaths: string[] },
): CommandEnvelope<TCommand, typeof payload> {
  return {
    schemaVersion: COMMAND_SCHEMA,
    requestId: asContractId(requestId),
    command,
    windowLabel: asContractId(binding.windowLabel),
    rendererSessionId: asContractId(binding.rendererSessionId),
    installationId: asContractId(binding.installationId),
    issuedAt: asUnsignedInteger(issuedAt),
    payload,
  };
}
