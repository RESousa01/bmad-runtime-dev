import {
  type ApprovalChoice,
  COMMAND_SCHEMA,
  type CommandEnvelope,
  type HostBinding,
  type ProposedChange,
  type RendererDispatchCommand,
} from "./contracts";
import { asContractId, asSha256, asUnsignedInteger } from "./validation";

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
