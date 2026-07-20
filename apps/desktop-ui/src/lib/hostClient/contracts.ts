import {
  type BmadHelpApprovedProjection,
  type BmadHelpContextReviewProjection,
  type BmadHelpRunCompletedProjection,
  type BmadHelpTerminalProjection,
  bmadModelCommands,
} from "../bmadModelProjection";
import { type BmadHelpRunCreatedProjection } from "../bmadProjection";

export const BOOTSTRAP_SCHEMA = "desktop-bootstrap.v1" as const;

export const COMMAND_SCHEMA = "desktop-ipc-command.v1" as const;

export const DISPATCH_REPLY_SCHEMA = "desktop-dispatch-reply.v1" as const;

export const PROJECTION_REQUEST_SCHEMA =
  "desktop-projection-request.v1" as const;

export const PROJECTION_REPLY_SCHEMA = "desktop-projection-reply.v1" as const;

export const BMAD_LIBRARY_SCHEMA = "bmad-library-snapshot.v2" as const;

export const BMAD_HELP_RECOMMENDATION_SCHEMA =
  "bmad-help-recommendation.v1" as const;

export const BMAD_HELP_RUN_SCHEMA = "bmad-help-run.v1" as const;

export const BMAD_HELP_COMPLETED_SCHEMA = "bmad-help-completed.v1" as const;

export const BMAD_MODEL_RECEIPT_SCHEMA =
  "bmad-model-receipt-summary.v1" as const;

export const CHANGES_REVIEW_SCHEMA = "sapphirus.changes-review.v1" as const;

export const desktopHostCommands = [
  "app.get_boot_state",
  "workspace.select_folder",
  "workspace.list",
  "workspace.revoke",
  "workspace.list_entries",
  "workspace.read_text",
  "workspace.search",
  "bmad.scan",
  "bmad.library.snapshot",
  "bmad.persona.view",
  bmadModelCommands.authStatus,
  bmadModelCommands.authSignIn,
  bmadModelCommands.authSignOut,
  bmadModelCommands.prepare,
  bmadModelCommands.approve,
  bmadModelCommands.cancel,
  bmadModelCommands.submit,
  "bmad.help.latest",
  "bmad.capability.prepare",
  "bmad.capability.approve",
  "bmad.capability.cancel",
  "bmad.capability.submit",
  "bmad.capability.latest",
  "run.create",
  "context.preview",
  "workspace.enable_edits",
  "changes.propose",
  "approval.decide",
  "rollback.request",
  "changes.history",
  "changes.recovery.prepare",
  "changes.recovery.decide",
  "app.preferences.get",
  "app.preferences.set",
  "app.about",
  "app.offboarding.inspect",
  "app.offboarding.erase",
  "workspace.pick_files",
] as const;

export type DesktopHostCommand = (typeof desktopHostCommands)[number];

export type BootMode = "ready" | "read_only_recovery";

export type WorkspacePermission = "read_only" | "governed_edits";

export type ThemePreference = "light" | "dark" | "system";

export type DensityPreference = "comfortable" | "compact";

export interface PreferencesProjection {
  schemaVersion: "desktop-preferences.v1";
  theme: ThemePreference;
  density: DensityPreference;
  updatedAt: number | null;
}

export interface AboutProjection {
  appVersion: string;
  installationId: string;
  bootMode: BootMode;
  foundationPackageName: string;
  foundationPackageVersion: string;
  inactiveBuilderPackageCount: number;
  updateConfigured: boolean;
  updateInstallAvailable: boolean;
}

export interface RetentionCategory {
  category: string;
  count: number;
}

export interface RetentionManifestProjection {
  schemaVersion: "sapphirus.retention-manifest.v1";
  categories: RetentionCategory[];
  retainedBytes: number;
}

export interface OffboardingErasedProjection {
  schemaVersion: "sapphirus.offboarding-erased.v1";
  status: "erased";
  restartRequired: boolean;
}

export const workspaceReadLimits = {
  contextBytes: 256 * 1024,
  contextPaths: 100,
  entryPage: 500,
  readBytes: 1024 * 1024,
  searchQueryBytes: 256,
  searchResults: 200,
} as const;

export const localEditsLimits = {
  changeContentBytes: 64 * 1024,
  changesPerProposal: 20,
  historyEntries: 50,
  openJournals: 64,
  reviewFiles: 20,
  recoveryOperations: 20,
  undoConflicts: 20,
} as const;

export interface WorkspaceProjection {
  workspaceId: string;
  projectId: string;
  displayName: string;
  grantEpoch: number;
  permissions: WorkspacePermission;
}

export interface BootstrapReply {
  schemaVersion: typeof BOOTSTRAP_SCHEMA;
  rendererSessionId: string;
  installationId: string;
  windowLabel: string;
  bootMode: BootMode;
  supportedCommands: DesktopHostCommand[];
  workspaces: WorkspaceProjection[];
  projectionSequence: number;
}

export interface ProjectionScope {
  workspaceId?: string;
}

export interface ProjectionSnapshot {
  sequence: number;
  generatedAt: number;
  bootMode: BootMode;
  workspaceCount: number;
  activeSessionId: string | null;
}

export type ProjectionEventPayload =
  | { type: "boot_state_changed"; projection: { mode: BootMode } }
  | { type: "workspace_changed"; projection: { workspaceId: string } }
  | { type: "bmad.projection_changed"; projection: { scope: "library" } }
  | {
      type: "session_changed";
      projection: { sessionId: string; state: string };
    }
  | {
      type: "approval_required";
      projection: { approvalId: string; candidateHash: string };
    }
  | {
      type: "execution_state_changed";
      projection: { executionId: string; state: string };
    }
  | {
      type: "checkpoint_changed";
      projection: { checkpointId: string; rollbackAvailable: boolean };
    }
  | { type: "evidence_changed"; projection: { streamId: string } }
  | { type: "connectivity_changed"; projection: { state: string } }
  | { type: "update_state_changed"; projection: { state: string } };

export interface ProjectionEvent {
  sequence: number;
  occurredAt: number;
  event: ProjectionEventPayload;
}

export interface LocalHostError {
  code:
    | "invalid_request"
    | "unauthorized"
    | "conflict"
    | "not_found"
    | "resource_limit"
    | "expired"
    | "integrity_failure"
    | "recovery_required"
    | "temporarily_unavailable"
    | "bmad_projection_unavailable"
    | "bmad_projection_gap"
    | "renderer_session_expired"
    | "identity_unavailable"
    | "authentication_required"
    | "reauthentication_required"
    | "tenant_mismatch"
    | "entitlement_unavailable"
    | "feature_disabled"
    | "context_rejected"
    | "context_drift"
    | "consent_required"
    | "consent_expired"
    | "consent_binding_mismatch"
    | "consent_already_consumed"
    | "support_plane_offline"
    | "transport_failed"
    | "response_binding_mismatch"
    | "invalid_model_output"
    | "receipt_invalid"
    | "internal";
  safeMessage: string;
  retryable: boolean;
  correlationId: string | null;
}

export type WorkspaceSelection =
  | { kind: "no_selection" }
  | { kind: "workspace_selected"; value: WorkspaceProjection };

export interface PickedFilesProjection {
  workspaceId: string;
  relativePaths: string[];
  selectedCount: number;
  rejectedOutsideRoot: number;
  rejectedUnreadable: number;
  truncated: boolean;
}

export type WorkspaceFilePick =
  | { kind: "no_selection" }
  | { kind: "picked"; value: PickedFilesProjection };

export interface WorkspaceRevocationResult {
  revoked: WorkspaceProjection;
  workspaces: WorkspaceProjection[];
}

export type WorkspaceEntryKind =
  "directory" | "text_file" | "binary_file" | "blocked";

export interface WorkspaceTreeEntry {
  relativePath: string;
  kind: WorkspaceEntryKind;
  sizeBytes: number;
  childCursor: string | null;
}

export interface WorkspaceEntriesProjection {
  workspaceId: string;
  entries: WorkspaceTreeEntry[];
  nextCursor: string | null;
}

export interface WorkspaceTextProjection {
  relativePath: string;
  content: string;
  contentHash: string;
  byteCount: number;
  truncated: boolean;
}

export interface WorkspaceSearchMatch {
  relativePath: string;
  line: number;
  preview: string;
}

export type BmadStatus =
  | "not_detected"
  | "method_detected"
  | "builder_drafts_detected"
  | "method_and_builder_drafts_detected";

export type BmadAssetKind =
  | "method_configuration"
  | "agent"
  | "workflow"
  | "builder_build_draft"
  | "builder_edit_draft"
  | "builder_analyze_draft";

export type BmadActivation = "read_only" | "inactive_draft";

export interface BmadAssetProjection {
  relativePath: string;
  assetKind: BmadAssetKind;
  activation: BmadActivation;
}

export interface BmadScanProjection {
  status: BmadStatus;
  assets: BmadAssetProjection[];
  truncated: boolean;
}

export interface ContextItemProjection {
  relativePath: string;
  startLine: number;
  endLine: number;
  reason: "Selected for this task";
  contentHash: string;
  classification: "source";
  redactions: [];
  byteCount: number;
  estimatedTokens: number;
  content: string;
}

export interface ContextPreviewProjection {
  workspaceId: string;
  manifestHash: string;
  items: ContextItemProjection[];
  totalBytes: number;
  estimatedTokens: number;
  modelTarget: null;
}

export type ProposedChange =
  | { change: "set_content"; relativePath: string; content: string }
  | { change: "delete"; relativePath: string };

export type ChangesReviewOperation = "create" | "modify" | "delete";

export type ChangesProposalKind = "edit" | "undo";

export type ApprovalChoice = "apply" | "revise" | "discard";

export type ChangesDisposition = "applied" | "discarded" | "revise_requested";

export interface ChangesReviewFileProjection {
  relativePath: string;
  operation: ChangesReviewOperation;
  beforeContent: string | null;
  afterContent: string | null;
  beforeHash: string | null;
  afterHash: string | null;
  beforeBytes: number;
  afterBytes: number;
}

export interface ChangesReviewProjection {
  schemaVersion: typeof CHANGES_REVIEW_SCHEMA;
  proposalId: string;
  candidateId: string;
  candidateHash: string;
  workspaceId: string;
  workspaceGrantEpoch: number;
  proposalKind: ChangesProposalKind;
  sourceExecutionId: string | null;
  files: ChangesReviewFileProjection[];
  totalChangedBytes: number;
  createdAt: number;
  expiresAt: number;
}

export interface ChangesReviewEnvelopeProjection {
  approvalId: string;
  displayedDiffHash: string;
  review: ChangesReviewProjection;
}

export interface ChangesExecutionFileProjection {
  relativePath: string;
  operation: string;
  exists: boolean;
  contentHash: string | null;
}

export interface ChangesExecutionProjection {
  executionId: string;
  checkpointId: string;
  completedAt: number;
  undoable: boolean;
  files: ChangesExecutionFileProjection[];
}

export interface ChangesDecisionProjection {
  approvalId: string;
  disposition: ChangesDisposition;
  execution: ChangesExecutionProjection | null;
}

export interface ChangesUndoConflictProjection {
  relativePath: string;
  expectedExists: boolean;
  currentExists: boolean;
}

export interface ChangesUndoUnavailableProjection {
  executionId: string;
  reason: string;
  conflicts: ChangesUndoConflictProjection[];
}

export type RollbackRequestResult =
  | { readonly kind: "review"; readonly value: ChangesReviewEnvelopeProjection }
  | {
      readonly kind: "unavailable";
      readonly value: ChangesUndoUnavailableProjection;
    };

export interface ChangesHistoryEntryProjection {
  executionId: string;
  journalState: string;
  fileCount: number;
  completedAt: string;
  undoable: boolean;
}

export interface ChangesOpenJournalProjection {
  journalId: string;
  executionId: string;
  state: string;
  updatedAt: string;
  recoveryAvailability: "review_available" | "quarantined" | "manual_review";
}

export type RecoveryApprovalChoice = "restore" | "cancel";
export type RecoveryManualReviewReasonCode = "checkpoint_incomplete_or_inconsistent";

export interface RecoveryOperationSummaryProjection {
  relativePath: string;
  operation: "create" | "replace" | "delete";
  explanation: string;
}

export type ChangesRecoveryPrepared =
  | {
      status: "review_required";
      recoveryApprovalId: string;
      displayedRecoveryHash: string;
      journalId: string;
      executionId: string;
      operations: RecoveryOperationSummaryProjection[];
      expiresAt: number;
    }
  | {
      status: "already_recovered";
      journalId: string;
      executionId: string;
    }
  | {
      status: "manual_review";
      journalId: string;
      executionId: string;
      reasonCode: RecoveryManualReviewReasonCode;
    };

export interface ChangesRecoveryDecision {
  recoveryApprovalId: string;
  disposition: "recovered" | "cancelled";
  journalId: string;
  executionId: string;
  restoredFiles: number;
}

export interface ChangesHistoryProjection {
  workspaceId: string;
  entries: ChangesHistoryEntryProjection[];
  openJournals: ChangesOpenJournalProjection[];
}

export type LatestBmadHelpRunResult =
  | { readonly kind: "no_run" }
  | { readonly kind: "projection_unavailable" }
  | {
      readonly kind: "retained";
      readonly run: BmadHelpRunCreatedProjection;
    }
  | { readonly kind: "interrupted"; readonly run: BmadHelpRunCreatedProjection }
  | {
      readonly kind: "review";
      readonly review: BmadHelpContextReviewProjection;
    }
  | {
      readonly kind: "approved";
      readonly review: BmadHelpContextReviewProjection;
      readonly approval: BmadHelpApprovedProjection;
    }
  | {
      readonly kind: "completed";
      readonly result: BmadHelpRunCompletedProjection;
    }
  | {
      readonly kind: "terminal";
      readonly terminal: BmadHelpTerminalProjection;
    };

export interface HostBinding {
  rendererSessionId: string;
  installationId: string;
  windowLabel: string;
}

export type RendererDispatchCommand =
  | "workspace.select_folder"
  | "workspace.list"
  | "workspace.revoke"
  | "workspace.list_entries"
  | "workspace.read_text"
  | "workspace.search"
  | "bmad.scan"
  | "bmad.library.snapshot"
  | "bmad.persona.view"
  | "model.auth.status"
  | "model.auth.sign_in"
  | "model.auth.sign_out"
  | "bmad.help.prepare"
  | "bmad.help.approve"
  | "bmad.help.cancel"
  | "bmad.help.submit"
  | "bmad.help.latest"
  | "bmad.capability.prepare"
  | "bmad.capability.approve"
  | "bmad.capability.cancel"
  | "bmad.capability.submit"
  | "bmad.capability.latest"
  | "context.preview"
  | "run.create"
  | "workspace.enable_edits"
  | "changes.propose"
  | "approval.decide"
  | "rollback.request"
  | "changes.history"
  | "changes.recovery.prepare"
  | "changes.recovery.decide"
  | "app.preferences.get"
  | "app.preferences.set"
  | "app.about"
  | "app.offboarding.inspect"
  | "app.offboarding.erase"
  | "workspace.pick_files";

export interface CommandEnvelope<
  TCommand extends RendererDispatchCommand,
  TPayload extends object,
> extends HostBinding {
  schemaVersion: typeof COMMAND_SCHEMA;
  requestId: string;
  command: TCommand;
  issuedAt: number;
  payload: TPayload;
}

export type TauriInvoke = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

export class HostProtocolError extends Error {
  constructor(message = "The Windows host returned an invalid response.") {
    super(message);
    this.name = "HostProtocolError";
  }
}

export class HostCapabilityError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "HostCapabilityError";
  }
}

export class HostCommandError extends Error {
  readonly details: LocalHostError;

  constructor(details: LocalHostError) {
    super(details.safeMessage);
    this.name = "HostCommandError";
    this.details = details;
  }
}
