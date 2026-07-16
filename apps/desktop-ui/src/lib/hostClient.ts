import type {
  BmadAgentMenuProjection,
  BmadAvailability,
  BmadBlockerCode,
  BmadEntrypointKind,
  BmadHelpActionProjection,
  BmadHelpConfidence,
  BmadHelpRecommendationProjection,
  BmadHelpRunCreatedProjection,
  BmadInstalledSkillProjection,
  BmadLibrarySnapshot,
  BmadMenuTargetKind,
  BmadMethodAgentProjection,
  BmadProjectionSource,
} from "./bmadProjection";
import {
  bmadModelCommands,
  type BmadHelpApprovedProjection,
  type BmadHelpCancelledProjection,
  type BmadHelpCompletedRecommendationProjection,
  type BmadHelpContextReviewProjection,
  type BmadHelpEvidenceClass,
  type BmadHelpNoRecommendationReason,
  type BmadHelpReceiptSummaryProjection,
  type BmadHelpRunCompletedProjection,
  type BmadHelpTerminalProjection,
  type ModelAuthStatusProjection,
} from "./bmadModelProjection";

const BOOTSTRAP_SCHEMA = "desktop-bootstrap.v1" as const;
const COMMAND_SCHEMA = "desktop-ipc-command.v1" as const;
const DISPATCH_REPLY_SCHEMA = "desktop-dispatch-reply.v1" as const;
const PROJECTION_REQUEST_SCHEMA = "desktop-projection-request.v1" as const;
const PROJECTION_REPLY_SCHEMA = "desktop-projection-reply.v1" as const;
const BMAD_LIBRARY_SCHEMA = "bmad-library-snapshot.v1" as const;
const BMAD_HELP_RECOMMENDATION_SCHEMA = "bmad-help-recommendation.v1" as const;
const BMAD_HELP_RUN_SCHEMA = "bmad-help-run.v1" as const;
const BMAD_HELP_COMPLETED_SCHEMA = "bmad-help-completed.v1" as const;
const BMAD_MODEL_RECEIPT_SCHEMA = "bmad-model-receipt-summary.v1" as const;

const bmadProjectionLimits = {
  responseBytes: 256 * 1024,
  installedSkills: 64,
  helpActions: 64,
  methodAgents: 16,
  menusPerAgent: 32,
  actionsPerSkill: 16,
  expectedArtifacts: 16,
  identifierBytes: 256,
  descriptionBytes: 2_048,
  iconBytes: 64,
  cursorBytes: 256,
  helpIntentBytes: 4_096,
  helpReasonBytes: 4_096,
  helpRunResponseBytes: (64 * 1_024) + 1_024,
  modelResponseBytes: 5 * 1_024 * 1_024,
  reviewItems: 16,
  reviewExclusions: 32,
  reviewSecretFindings: 64,
  reviewTextBytes: 64 * 1024,
  reviewProjectionBytes: 96 * 1024,
  reviewLabelBytes: 1_024,
  receiptInputBytes: 4 * 1024 * 1024,
  receiptOutputBytes: 1024 * 1024,
} as const;

const bmadAvailabilities = new Set<BmadAvailability>([
  "available",
  "capability_disabled",
  "dependency_unavailable",
  "orphan_skill",
  "network_unavailable",
  "source_prompt_unavailable",
]);
const bmadBlockerCodes = new Set<BmadBlockerCode>([
  "bmad_capability_disabled",
  "bmad_dependency_unavailable",
  "bmad_help_catalog_orphan",
  "bmad_network_reference_unavailable",
  "bmad_source_prompt_unavailable",
]);
const bmadEntrypointKinds = new Set<BmadEntrypointKind>([
  "direct",
  "inline",
  "step_jit",
  "script_rendered",
  "compatibility_shim",
]);
const bmadMenuTargetKinds = new Set<BmadMenuTargetKind>([
  "skill_target",
  "prompt_reference",
]);
const bmadHelpConfidences = new Set<BmadHelpConfidence>([
  "authoritative",
  "user_asserted",
  "heuristic",
  "contextual",
  "unknown",
]);

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
  bmadModelCommands.authStatus,
  bmadModelCommands.authSignIn,
  bmadModelCommands.authSignOut,
  bmadModelCommands.prepare,
  bmadModelCommands.approve,
  bmadModelCommands.cancel,
  bmadModelCommands.submit,
  "bmad.help.latest",
  "run.create",
  "context.preview",
] as const;

export type DesktopHostCommand = (typeof desktopHostCommands)[number];
export type BootMode = "ready" | "read_only_recovery";
export type WorkspacePermission = "read_only";

export const workspaceReadLimits = {
  contextBytes: 256 * 1024,
  contextPaths: 100,
  entryPage: 500,
  readBytes: 1024 * 1024,
  searchQueryBytes: 256,
  searchResults: 200,
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

type ProjectionEventPayload =
  | { type: "boot_state_changed"; projection: { mode: BootMode } }
  | { type: "workspace_changed"; projection: { workspaceId: string } }
  | { type: "bmad.projection_changed"; projection: { scope: "library" } };

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

export interface WorkspaceRevocationResult {
  revoked: WorkspaceProjection;
  workspaces: WorkspaceProjection[];
}

export type WorkspaceEntryKind = "directory" | "text_file" | "binary_file" | "blocked";

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

export type LatestBmadHelpRunResult =
  | { readonly kind: "no_run" }
  | { readonly kind: "projection_unavailable" }
  | {
    readonly kind: "retained";
    readonly run: BmadHelpRunCreatedProjection;
  }
  | { readonly kind: "interrupted"; readonly run: BmadHelpRunCreatedProjection }
  | { readonly kind: "review"; readonly review: BmadHelpContextReviewProjection }
  | {
    readonly kind: "approved";
    readonly review: BmadHelpContextReviewProjection;
    readonly approval: BmadHelpApprovedProjection;
  }
  | { readonly kind: "completed"; readonly result: BmadHelpRunCompletedProjection }
  | { readonly kind: "terminal"; readonly terminal: BmadHelpTerminalProjection };

export interface HostBinding {
  rendererSessionId: string;
  installationId: string;
  windowLabel: string;
}

type RendererDispatchCommand =
  | "workspace.select_folder"
  | "workspace.list"
  | "workspace.revoke"
  | "workspace.list_entries"
  | "workspace.read_text"
  | "workspace.search"
  | "bmad.scan"
  | "bmad.library.snapshot"
  | "model.auth.status"
  | "model.auth.sign_in"
  | "model.auth.sign_out"
  | "bmad.help.prepare"
  | "bmad.help.approve"
  | "bmad.help.cancel"
  | "bmad.help.submit"
  | "bmad.help.latest"
  | "context.preview"
  | "run.create";

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

function fail(): never {
  throw new HostProtocolError();
}

function asRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return fail();
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    return fail();
  }
  return value as Record<string, unknown>;
}

function assertExactKeys(
  value: Record<string, unknown>,
  required: readonly string[],
): void {
  const actual = Object.keys(value).sort();
  const expected = [...required].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    fail();
  }
}

function asBoundedString(value: unknown, maximumLength = 512): string {
  if (
    typeof value !== "string"
    || value.length === 0
    || value.length > maximumLength
    || /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    return fail();
  }
  return value;
}

function asContractId(value: unknown): string {
  const identifier = asBoundedString(value, 128);
  if (!/^[A-Za-z0-9._-]{3,128}$/u.test(identifier)) {
    return fail();
  }
  return identifier;
}

function asUnsignedInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    return fail();
  }
  return value as number;
}

function asBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    return fail();
  }
  return value;
}

function utf8Length(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function hasUnpairedSurrogate(value: string): boolean {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (next < 0xdc00 || next > 0xdfff) {
        return true;
      }
      index += 1;
    } else if (code >= 0xdc00 && code <= 0xdfff) {
      return true;
    }
  }
  return false;
}

function asBmadCursor(value: unknown): string | null {
  if (value === null) {
    return null;
  }
  if (
    typeof value !== "string"
    || value.length === 0
    || utf8Length(value) > bmadProjectionLimits.cursorBytes
    || !/^[\x21-\x7e]+$/u.test(value)
  ) {
    return fail();
  }
  return value;
}

function asBmadIdentifier(value: unknown): string {
  if (
    typeof value !== "string"
    || value.length === 0
    || utf8Length(value) > bmadProjectionLimits.identifierBytes
    || !/^[A-Za-z0-9._-]+$/u.test(value)
  ) {
    return fail();
  }
  return value;
}

function asModelRegion(value: unknown): string {
  if (
    typeof value !== "string"
    || value.length < 3
    || value.length > 64
    || !/^[a-z][a-z0-9-]+$/u.test(value)
  ) {
    return fail();
  }
  return value;
}

function asNullableBmadIdentifier(value: unknown): string | null {
  return value === null ? null : asBmadIdentifier(value);
}

function asBmadSafeText(value: unknown, maximumBytes: number): string {
  if (
    typeof value !== "string"
    || utf8Length(value) > maximumBytes
    || hasUnpairedSurrogate(value)
    || /[\p{Cc}\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/u.test(value)
  ) {
    return fail();
  }
  return value;
}

function asBmadNonemptySafeText(value: unknown, maximumBytes: number): string {
  const text = asBmadSafeText(value, maximumBytes);
  if (text.trim().length === 0) {
    return fail();
  }
  return text;
}

function asBmadHelpIntent(value: unknown): string {
  if (
    typeof value !== "string"
    || value.trim().length === 0
    || utf8Length(value) > bmadProjectionLimits.helpIntentBytes
    || hasUnpairedSurrogate(value)
    || /[\p{Cc}\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/u.test(value)
  ) {
    return fail();
  }
  return value;
}

function asBmadAvailability(value: unknown): BmadAvailability {
  const availability = asBmadIdentifier(value) as BmadAvailability;
  if (!bmadAvailabilities.has(availability)) {
    return fail();
  }
  return availability;
}

function asBmadEntrypointKind(value: unknown): BmadEntrypointKind {
  const entrypointKind = asBmadIdentifier(value) as BmadEntrypointKind;
  if (!bmadEntrypointKinds.has(entrypointKind)) {
    return fail();
  }
  return entrypointKind;
}

function asBmadMenuTargetKind(value: unknown): BmadMenuTargetKind {
  const targetKind = asBmadIdentifier(value) as BmadMenuTargetKind;
  if (!bmadMenuTargetKinds.has(targetKind)) {
    return fail();
  }
  return targetKind;
}

function asBmadHelpConfidence(value: unknown): BmadHelpConfidence {
  const confidence = asBmadIdentifier(value) as BmadHelpConfidence;
  if (!bmadHelpConfidences.has(confidence)) {
    return fail();
  }
  return confidence;
}

function asBmadBlockerCode(value: unknown): BmadBlockerCode {
  const blockerCode = asBmadIdentifier(value) as BmadBlockerCode;
  if (!bmadBlockerCodes.has(blockerCode)) {
    return fail();
  }
  return blockerCode;
}

function parseBmadBlockerCodes(value: unknown): BmadBlockerCode[] {
  if (!Array.isArray(value) || value.length > bmadBlockerCodes.size) {
    return fail();
  }
  const blockerCodes = value.map(asBmadBlockerCode);
  if (new Set(blockerCodes).size !== blockerCodes.length) {
    return fail();
  }
  return blockerCodes;
}

function asNullableBmadBlockerCode(value: unknown): BmadBlockerCode | null {
  return value === null ? null : asBmadBlockerCode(value);
}

function assertUniqueIdentities(identities: readonly string[]): void {
  if (new Set(identities).size !== identities.length) {
    fail();
  }
}

function asNullableOpaqueCursor(value: unknown): string | null {
  if (value === null) {
    return null;
  }
  const cursor = asBoundedString(value, 64);
  if (!/^cursor_[0-9A-HJKMNP-TV-Z]{26}$/u.test(cursor)) {
    return fail();
  }
  return cursor;
}

function asSha256(value: unknown): string {
  const digest = asBoundedString(value, 71);
  if (!/^sha256:[0-9a-f]{64}$/u.test(digest)) {
    return fail();
  }
  return digest;
}

function isWindowsReservedSegment(segment: string): boolean {
  const deviceName = segment
    .split(".", 1)[0]!
    .replace(/[. ]+$/u, "")
    .toLocaleUpperCase("en-US");
  return /^(?:CON|PRN|AUX|NUL|CLOCK\$|CONIN\$|CONOUT\$|(?:COM|LPT)[1-9¹²³])$/u.test(
    deviceName,
  );
}

function asRelativePath(value: unknown): string {
  if (typeof value !== "string" || value.length === 0 || utf8Length(value) > 1024) {
    return fail();
  }
  if (
    value.startsWith("/")
    || value.includes("\\")
    || value.includes(":")
    || /[<>"|?*]/u.test(value)
    || /\p{C}/u.test(value)
    || hasUnpairedSurrogate(value)
  ) {
    return fail();
  }
  const segments = value.split("/");
  if (
    segments.some((segment) =>
      segment.length === 0
      || segment.length > 255
      || segment === "."
      || segment === ".."
      || segment.endsWith(".")
      || segment.endsWith(" ")
      || isWindowsReservedSegment(segment)
    )
  ) {
    return fail();
  }
  return value;
}

function asTextContent(value: unknown, maximumBytes: number): string {
  if (
    typeof value !== "string"
    || value.includes("\0")
    || hasUnpairedSurrogate(value)
    || utf8Length(value) > maximumBytes
  ) {
    return fail();
  }
  return value;
}

function asSingleLineText(value: unknown, maximumLength: number): string {
  const text = asBoundedString(value, maximumLength);
  if (/\p{C}/u.test(text) || hasUnpairedSurrogate(text)) {
    return fail();
  }
  return text;
}

function assertUniqueRelativePaths(paths: readonly string[]): void {
  const folded = new Set<string>();
  for (const path of paths) {
    const key = path.toLocaleLowerCase("en-US");
    if (folded.has(key)) {
      fail();
    }
    folded.add(key);
  }
}

function isImmediateChild(relativePath: string, relativeDirectory: string): boolean {
  if (relativeDirectory === ".") {
    return !relativePath.includes("/");
  }
  const prefix = `${relativeDirectory}/`;
  if (!relativePath.startsWith(prefix)) {
    return false;
  }
  return !relativePath.slice(prefix.length).includes("/");
}

function asNullableContractId(value: unknown): string | null {
  return value === null ? null : asContractId(value);
}

function asBootMode(value: unknown): BootMode {
  if (value !== "ready" && value !== "read_only_recovery") {
    return fail();
  }
  return value;
}

function asWorkspacePermission(value: unknown): WorkspacePermission {
  if (value !== "read_only") {
    return fail();
  }
  return value;
}

function asSafeDisplayName(value: unknown): string {
  const displayName = asBoundedString(value, 255);
  if (
    displayName !== displayName.trim()
    || displayName.includes("/")
    || displayName.includes("\\")
    || displayName.includes(":")
    || /\p{C}/u.test(displayName)
    || displayName === "."
    || displayName === ".."
  ) {
    return fail();
  }
  return displayName;
}

function asRendererSafeMessage(value: unknown): string {
  const message = asBoundedString(value, 512);
  if (
    /\p{C}/u.test(message)
    || /(?:\\|[A-Za-z]:\/|file:\/\/|(?:^|[^A-Za-z0-9])\/(?:[^\s]|$))/iu.test(message)
  ) {
    return fail();
  }
  return message;
}

function parseWorkspace(value: unknown): WorkspaceProjection {
  const workspace = asRecord(value);
  assertExactKeys(workspace, [
    "workspaceId",
    "projectId",
    "displayName",
    "grantEpoch",
    "permissions",
  ]);
  return {
    workspaceId: asContractId(workspace.workspaceId),
    projectId: asContractId(workspace.projectId),
    displayName: asSafeDisplayName(workspace.displayName),
    grantEpoch: asUnsignedInteger(workspace.grantEpoch),
    permissions: asWorkspacePermission(workspace.permissions),
  };
}

function parseSupportedCommands(value: unknown): DesktopHostCommand[] {
  if (!Array.isArray(value) || value.length > desktopHostCommands.length) {
    return fail();
  }
  const allowed = new Set<string>(desktopHostCommands);
  const result = value.map((command) => {
    const parsed = asBoundedString(command, 64);
    if (!allowed.has(parsed)) {
      return fail();
    }
    return parsed as DesktopHostCommand;
  });
  if (new Set(result).size !== result.length) {
    return fail();
  }
  return result;
}

export function parseBootstrapReply(value: unknown): BootstrapReply {
  const reply = asRecord(value);
  assertExactKeys(reply, [
    "schemaVersion",
    "rendererSessionId",
    "installationId",
    "windowLabel",
    "bootMode",
    "supportedCommands",
    "workspaces",
    "projectionSequence",
  ]);
  if (
    reply.schemaVersion !== BOOTSTRAP_SCHEMA
    || !Array.isArray(reply.workspaces)
    || reply.workspaces.length > 256
  ) {
    return fail();
  }
  const workspaces = reply.workspaces.map(parseWorkspace);
  if (new Set(workspaces.map(({ workspaceId }) => workspaceId)).size !== workspaces.length) {
    return fail();
  }
  const bootMode = asBootMode(reply.bootMode);
  const supportedCommands = parseSupportedCommands(reply.supportedCommands);
  if (bootMode === "read_only_recovery") {
    const recoveryCommands = new Set<DesktopHostCommand>([
      "app.get_boot_state",
      "workspace.list",
    ]);
    if (
      supportedCommands.length !== recoveryCommands.size
      || supportedCommands.some((command) => !recoveryCommands.has(command))
    ) {
      return fail();
    }
  }
  return {
    schemaVersion: BOOTSTRAP_SCHEMA,
    rendererSessionId: asContractId(reply.rendererSessionId),
    installationId: asContractId(reply.installationId),
    windowLabel: asContractId(reply.windowLabel),
    bootMode,
    supportedCommands,
    workspaces,
    projectionSequence: asUnsignedInteger(reply.projectionSequence),
  };
}

function parseLocalHostError(value: unknown): LocalHostError {
  const error = asRecord(value);
  assertExactKeys(error, ["code", "safeMessage", "retryable", "correlationId"]);
  const codes = new Set<LocalHostError["code"]>([
    "invalid_request",
    "unauthorized",
    "conflict",
    "not_found",
    "resource_limit",
    "expired",
    "integrity_failure",
    "recovery_required",
    "temporarily_unavailable",
    "bmad_projection_unavailable",
    "bmad_projection_gap",
    "renderer_session_expired",
    "identity_unavailable",
    "authentication_required",
    "reauthentication_required",
    "tenant_mismatch",
    "entitlement_unavailable",
    "feature_disabled",
    "context_rejected",
    "context_drift",
    "consent_required",
    "consent_expired",
    "consent_binding_mismatch",
    "consent_already_consumed",
    "support_plane_offline",
    "transport_failed",
    "response_binding_mismatch",
    "invalid_model_output",
    "receipt_invalid",
    "internal",
  ]);
  const code = asBoundedString(error.code, 64) as LocalHostError["code"];
  if (!codes.has(code)) {
    return fail();
  }
  return {
    code,
    safeMessage: asRendererSafeMessage(error.safeMessage),
    retryable: asBoolean(error.retryable),
    correlationId: asNullableContractId(error.correlationId),
  };
}

function parseDispatchReply(
  value: unknown,
  requestId: string,
): {
  data: Record<string, unknown>;
  sequence: number;
  receipt: { acceptedAt: number; operationId: string | null };
} {
  const reply = asRecord(value);
  if (reply.schemaVersion !== DISPATCH_REPLY_SCHEMA) {
    return fail();
  }
  if (reply.status === "error") {
    assertExactKeys(reply, ["schemaVersion", "requestId", "sequence", "status", "error"]);
    const error = parseLocalHostError(reply.error);
    const isUnboundExpiredRenderer = reply.requestId === null
      && error.code === "renderer_session_expired"
      && error.correlationId === null;
    if (reply.requestId !== requestId && !isUnboundExpiredRenderer) {
      return fail();
    }
    asUnsignedInteger(reply.sequence);
    throw new HostCommandError(error);
  }
  if (reply.status !== "ok") {
    return fail();
  }
  assertExactKeys(reply, ["schemaVersion", "requestId", "sequence", "status", "receipt", "data"]);
  if (reply.requestId !== requestId) {
    return fail();
  }
  asUnsignedInteger(reply.sequence);
  const receipt = asRecord(reply.receipt);
  assertExactKeys(receipt, ["requestId", "acceptedAt", "operationId"]);
  if (receipt.requestId !== requestId) {
    return fail();
  }
  const acceptedAt = asUnsignedInteger(receipt.acceptedAt);
  const operationId = asNullableContractId(receipt.operationId);

  return {
    data: asRecord(reply.data),
    sequence: asUnsignedInteger(reply.sequence),
    receipt: { acceptedAt, operationId },
  };
}

function parseWorkspaceSelectionReply(
  value: unknown,
  requestId: string,
): { selection: WorkspaceSelection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const { data } = parsed;
  if (data.kind === "no_selection") {
    assertExactKeys(data, ["kind"]);
    return {
      selection: { kind: "no_selection" },
      sequence: parsed.sequence,
    };
  }
  if (data.kind !== "workspace_selected") {
    return fail();
  }
  assertExactKeys(data, ["kind", "value"]);
  return {
    selection: { kind: "workspace_selected", value: parseWorkspace(data.value) },
    sequence: parsed.sequence,
  };
}

function parseWorkspaceListReply(
  value: unknown,
  requestId: string,
): { workspaces: WorkspaceProjection[]; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const { data } = parsed;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "workspace_list" || !Array.isArray(data.value) || data.value.length > 256) {
    return fail();
  }
  const workspaces = data.value.map(parseWorkspace);
  if (new Set(workspaces.map(({ workspaceId }) => workspaceId)).size !== workspaces.length) {
    return fail();
  }
  return { workspaces, sequence: parsed.sequence };
}

function sameWorkspaceIdentity(
  left: WorkspaceProjection,
  right: WorkspaceProjection,
): boolean {
  return left.workspaceId === right.workspaceId
    && left.projectId === right.projectId
    && left.displayName === right.displayName
    && left.permissions === right.permissions;
}

function parseWorkspaceRevocationReply(
  value: unknown,
  requestId: string,
  expected: WorkspaceProjection,
): { revoked: WorkspaceProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const { data } = parsed;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "workspace_revoked" || expected.grantEpoch >= Number.MAX_SAFE_INTEGER) {
    return fail();
  }
  const revoked = parseWorkspace(data.value);
  if (
    !sameWorkspaceIdentity(revoked, expected)
    || revoked.grantEpoch !== expected.grantEpoch + 1
  ) {
    return fail();
  }
  return { revoked, sequence: parsed.sequence };
}

function parseWorkspaceEntry(value: unknown, relativeDirectory: string): WorkspaceTreeEntry {
  const entry = asRecord(value);
  assertExactKeys(entry, ["relativePath", "kind", "sizeBytes", "childCursor"]);
  const relativePath = asRelativePath(entry.relativePath);
  if (!isImmediateChild(relativePath, relativeDirectory)) {
    return fail();
  }
  const allowedKinds = new Set<WorkspaceEntryKind>([
    "directory",
    "text_file",
    "binary_file",
    "blocked",
  ]);
  const kind = asBoundedString(entry.kind, 32) as WorkspaceEntryKind;
  if (!allowedKinds.has(kind)) {
    return fail();
  }
  const childCursor = asNullableOpaqueCursor(entry.childCursor);
  if ((kind === "directory") !== (childCursor !== null)) {
    return fail();
  }
  return {
    relativePath,
    kind,
    sizeBytes: asUnsignedInteger(entry.sizeBytes),
    childCursor,
  };
}

function parseWorkspaceEntriesReply(
  value: unknown,
  requestId: string,
  expected: { workspaceId: string; relativeDirectory: string; limit: number },
): { projection: WorkspaceEntriesProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "workspace_entries") {
    return fail();
  }
  const projection = asRecord(data.value);
  assertExactKeys(projection, ["workspaceId", "entries", "nextCursor"]);
  if (
    projection.workspaceId !== expected.workspaceId
    || !Array.isArray(projection.entries)
    || projection.entries.length > expected.limit
  ) {
    return fail();
  }
  const entries = projection.entries.map((entry) =>
    parseWorkspaceEntry(entry, expected.relativeDirectory)
  );
  assertUniqueRelativePaths(entries.map(({ relativePath }) => relativePath));
  return {
    projection: {
      workspaceId: asContractId(projection.workspaceId),
      entries,
      nextCursor: asNullableOpaqueCursor(projection.nextCursor),
    },
    sequence: parsed.sequence,
  };
}

function parseWorkspaceTextReply(
  value: unknown,
  requestId: string,
  expectedPath: string,
  maximumBytes: number,
): { projection: WorkspaceTextProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "workspace_text") {
    return fail();
  }
  const projection = asRecord(data.value);
  assertExactKeys(projection, [
    "relativePath",
    "content",
    "contentHash",
    "byteCount",
    "truncated",
  ]);
  const relativePath = asRelativePath(projection.relativePath);
  if (relativePath !== expectedPath) {
    return fail();
  }
  const content = asTextContent(projection.content, maximumBytes);
  const contentBytes = utf8Length(content);
  const byteCount = asUnsignedInteger(projection.byteCount);
  const truncated = asBoolean(projection.truncated);
  if (
    (!truncated && byteCount !== contentBytes)
    || (truncated && byteCount <= contentBytes)
  ) {
    return fail();
  }
  return {
    projection: {
      relativePath,
      content,
      contentHash: asSha256(projection.contentHash),
      byteCount,
      truncated,
    },
    sequence: parsed.sequence,
  };
}

function parseSearchResultsReply(
  value: unknown,
  requestId: string,
  maximumResults: number,
): { matches: WorkspaceSearchMatch[]; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (
    data.kind !== "search_results"
    || !Array.isArray(data.value)
    || data.value.length > maximumResults
  ) {
    return fail();
  }
  const matches = data.value.map((value): WorkspaceSearchMatch => {
    const match = asRecord(value);
    assertExactKeys(match, ["relativePath", "line", "preview"]);
    const line = asUnsignedInteger(match.line);
    if (line === 0) {
      return fail();
    }
    return {
      relativePath: asRelativePath(match.relativePath),
      line,
      preview: asSingleLineText(match.preview, 512),
    };
  });
  const identities = matches.map(({ relativePath, line }) => `${relativePath.toLocaleLowerCase("en-US")}:${line}`);
  if (new Set(identities).size !== identities.length) {
    return fail();
  }
  return { matches, sequence: parsed.sequence };
}

function parseBmadScanReply(
  value: unknown,
  requestId: string,
): { projection: BmadScanProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "bmad_scan") {
    return fail();
  }
  const projection = asRecord(data.value);
  assertExactKeys(projection, ["status", "assets", "truncated"]);
  const statuses = new Set<BmadStatus>([
    "not_detected",
    "method_detected",
    "builder_drafts_detected",
    "method_and_builder_drafts_detected",
  ]);
  const status = asBoundedString(projection.status, 64) as BmadStatus;
  if (!statuses.has(status) || !Array.isArray(projection.assets) || projection.assets.length > 256) {
    return fail();
  }
  const methodKinds = new Set<BmadAssetKind>(["method_configuration", "agent", "workflow"]);
  const draftKinds = new Set<BmadAssetKind>([
    "builder_build_draft",
    "builder_edit_draft",
    "builder_analyze_draft",
  ]);
  const assets = projection.assets.map((value): BmadAssetProjection => {
    const asset = asRecord(value);
    assertExactKeys(asset, ["relativePath", "assetKind", "activation"]);
    const assetKind = asBoundedString(asset.assetKind, 64) as BmadAssetKind;
    if (!methodKinds.has(assetKind) && !draftKinds.has(assetKind)) {
      return fail();
    }
    const activation = asBoundedString(asset.activation, 32) as BmadActivation;
    if (
      (methodKinds.has(assetKind) && activation !== "read_only")
      || (draftKinds.has(assetKind) && activation !== "inactive_draft")
    ) {
      return fail();
    }
    return {
      relativePath: asRelativePath(asset.relativePath),
      assetKind,
      activation,
    };
  });
  assertUniqueRelativePaths(assets.map(({ relativePath }) => relativePath));
  const hasMethod = assets.some(({ activation }) => activation === "read_only");
  const hasDraft = assets.some(({ activation }) => activation === "inactive_draft");
  const expectedStatus: BmadStatus = hasMethod
    ? hasDraft ? "method_and_builder_drafts_detected" : "method_detected"
    : hasDraft ? "builder_drafts_detected" : "not_detected";
  if (status !== expectedStatus) {
    return fail();
  }
  return {
    projection: { status, assets, truncated: asBoolean(projection.truncated) },
    sequence: parsed.sequence,
  };
}

function parseBmadProjectionSource(value: unknown): BmadProjectionSource {
  const source = asRecord(value);
  assertExactKeys(source, ["sourceKind", "packageName", "packageVersion"]);
  if (source.sourceKind !== "sealed_foundation") {
    return fail();
  }
  return {
    sourceKind: source.sourceKind,
    packageName: asBmadNonemptySafeText(source.packageName, bmadProjectionLimits.identifierBytes),
    packageVersion: asBmadNonemptySafeText(
      source.packageVersion,
      bmadProjectionLimits.identifierBytes,
    ),
  };
}

function assertBmadAvailabilityBlockers(
  availability: BmadAvailability,
  blockerCodes: readonly BmadBlockerCode[],
): void {
  const expected = availability === "available"
    ? []
    : [({
      capability_disabled: "bmad_capability_disabled",
      dependency_unavailable: "bmad_dependency_unavailable",
      orphan_skill: "bmad_help_catalog_orphan",
      network_unavailable: "bmad_network_reference_unavailable",
      source_prompt_unavailable: "bmad_source_prompt_unavailable",
    } as const)[availability]];
  if (
    blockerCodes.length !== expected.length
    || blockerCodes.some((code, index) => code !== expected[index])
  ) {
    fail();
  }
}

function parseBmadHelpRecommendation(value: unknown): BmadHelpRecommendationProjection {
  const recommendation = asRecord(value);
  assertExactKeys(recommendation, [
    "schemaVersion",
    "displayName",
    "moduleCode",
    "skillName",
    "action",
    "confidence",
    "source",
    "reason",
    "requiredGuidance",
    "expectedArtifacts",
    "availability",
    "blockerCodes",
    "completionClaimed",
  ]);
  if (
    recommendation.schemaVersion !== BMAD_HELP_RECOMMENDATION_SCHEMA
    || recommendation.completionClaimed !== false
    || !Array.isArray(recommendation.expectedArtifacts)
    || recommendation.expectedArtifacts.length > bmadProjectionLimits.expectedArtifacts
  ) {
    return fail();
  }
  const availability = asBmadAvailability(recommendation.availability);
  const blockerCodes = parseBmadBlockerCodes(recommendation.blockerCodes);
  assertBmadAvailabilityBlockers(availability, blockerCodes);
  return {
    schemaVersion: BMAD_HELP_RECOMMENDATION_SCHEMA,
    displayName: asBmadNonemptySafeText(
      recommendation.displayName,
      bmadProjectionLimits.identifierBytes,
    ),
    moduleCode: asBmadIdentifier(recommendation.moduleCode),
    skillName: asBmadIdentifier(recommendation.skillName),
    action: asNullableBmadIdentifier(recommendation.action),
    confidence: asBmadHelpConfidence(recommendation.confidence),
    source: parseBmadProjectionSource(recommendation.source),
    reason: asBmadNonemptySafeText(
      recommendation.reason,
      bmadProjectionLimits.helpReasonBytes,
    ),
    requiredGuidance: asBoolean(recommendation.requiredGuidance),
    expectedArtifacts: recommendation.expectedArtifacts.map((artifact) =>
      asBmadNonemptySafeText(artifact, bmadProjectionLimits.identifierBytes)
    ),
    availability,
    blockerCodes,
    completionClaimed: false,
  };
}

function parseBmadHelpRunCreated(
  value: unknown,
  expectedWorkspaceId: string,
): BmadHelpRunCreatedProjection {
  const run = asRecord(value);
  assertExactKeys(run, [
    "schemaVersion",
    "runKind",
    "lifecycle",
    "workspaceId",
    "runId",
    "sessionId",
    "currentIntent",
    "runnable",
    "completionClaimed",
    "recommendation",
  ]);
  if (
    run.schemaVersion !== BMAD_HELP_RUN_SCHEMA
    || run.runKind !== "bmad_help"
    || run.lifecycle !== "created_unbound"
    || run.runnable !== false
    || run.completionClaimed !== false
  ) {
    return fail();
  }
  const workspaceId = asContractId(run.workspaceId);
  if (workspaceId !== expectedWorkspaceId) {
    return fail();
  }
  const projection: BmadHelpRunCreatedProjection = {
    schemaVersion: BMAD_HELP_RUN_SCHEMA,
    runKind: "bmad_help",
    lifecycle: "created_unbound",
    workspaceId,
    runId: asContractId(run.runId),
    sessionId: asContractId(run.sessionId),
    currentIntent: asBmadHelpIntent(run.currentIntent),
    runnable: false,
    completionClaimed: false,
    recommendation: parseBmadHelpRecommendation(run.recommendation),
  };
  if (utf8Length(JSON.stringify(projection)) > bmadProjectionLimits.helpRunResponseBytes) {
    return fail();
  }
  return projection;
}

function asPositiveSafeInteger(value: unknown): number {
  const parsed = asUnsignedInteger(value);
  if (parsed === 0) return fail();
  return parsed;
}

function asBmadRendererText(value: unknown, maximumBytes: number): string {
  const text = asBmadNonemptySafeText(value, maximumBytes);
  if (/(?:\\\\|[A-Za-z]:[\\/]|file:\/\/|(?:^|\s)\/(?:[^\s]|$))/iu.test(text)) {
    return fail();
  }
  return text;
}

function parseModelAuthStatus(value: unknown): ModelAuthStatusProjection {
  const status = asRecord(value);
  const developmentOnly = asBoolean(status.developmentOnly);
  assertExactKeys(status, [
    "status",
    "mode",
    "authEpoch",
    "developmentOnly",
    "destinationLabel",
    "signInAvailable",
    "signOutAvailable",
  ]);
  if (
    (status.status !== "unavailable" && status.status !== "development_ready")
    || (status.mode !== "offline" && status.mode !== "deterministic_development")
    || status.signInAvailable !== false
    || status.signOutAvailable !== true
    || (
      status.status === "development_ready"
        ? status.mode !== "deterministic_development" || developmentOnly !== true
        : status.mode !== "offline" || developmentOnly !== false
    )
  ) {
    return fail();
  }
  return {
    status: status.status,
    mode: status.mode,
    authEpoch: asUnsignedInteger(status.authEpoch),
    developmentOnly,
    destinationLabel: asBmadRendererText(status.destinationLabel, 256),
    signInAvailable: false,
    signOutAvailable: true,
  };
}

function parseBmadHelpReview(value: unknown, expectedWorkspaceId: string): BmadHelpContextReviewProjection {
  const review = asRecord(value);
  assertExactKeys(review, [
    "workspaceId",
    "workspaceGrantEpoch",
    "runId",
    "sessionId",
    "destinationLabel",
    "developmentOnly",
    "consentDisclosure",
    "manifestHash",
    "purpose",
    "region",
    "retentionMode",
    "expiresAt",
    "items",
    "exclusions",
    "secretFindings",
    "totalOutboundBytes",
    "totalTokenEstimate",
    "redactionLimitation",
  ]);
  if (
    review.workspaceId !== expectedWorkspaceId
    || typeof review.developmentOnly !== "boolean"
    || review.retentionMode !== "transient_no_store"
    || !Array.isArray(review.items)
    || review.items.length === 0
    || review.items.length > bmadProjectionLimits.reviewItems
    || !Array.isArray(review.exclusions)
    || review.exclusions.length > bmadProjectionLimits.reviewExclusions
    || !Array.isArray(review.secretFindings)
    || review.secretFindings.length > bmadProjectionLimits.reviewSecretFindings
  ) {
    return fail();
  }
  const items = review.items.map((value) => {
    const item = asRecord(value);
    assertExactKeys(item, [
      "relativeLabel",
      "semanticRole",
      "language",
      "outboundByteCount",
      "tokenEstimate",
      "classification",
      "redactions",
      "outboundContent",
    ]);
    if (
      !["public", "internal", "confidential"].includes(String(item.classification))
      || !Array.isArray(item.redactions)
      || item.redactions.length > 32
    ) {
      return fail();
    }
    const outboundContent = asTextContent(item.outboundContent, bmadProjectionLimits.reviewTextBytes);
    const outboundByteCount = asPositiveSafeInteger(item.outboundByteCount);
    if (outboundByteCount !== utf8Length(outboundContent)) return fail();
    return {
      relativeLabel: asRelativePath(item.relativeLabel),
      semanticRole: asBmadIdentifier(item.semanticRole),
      language: item.language === null ? null : asBmadIdentifier(item.language),
      outboundByteCount,
      tokenEstimate: asPositiveSafeInteger(item.tokenEstimate),
      classification: item.classification as "public" | "internal" | "confidential",
      redactions: item.redactions.map((value) => {
        const redaction = asRecord(value);
        assertExactKeys(redaction, ["kind", "occurrenceCount"]);
        return {
          kind: asBmadIdentifier(redaction.kind),
          occurrenceCount: asPositiveSafeInteger(redaction.occurrenceCount),
        };
      }),
      outboundContent,
    };
  });
  assertUniqueRelativePaths(items.map(({ relativeLabel }) => relativeLabel));
  const exclusions = review.exclusions.map((value) => {
    const exclusion = asRecord(value);
    assertExactKeys(exclusion, ["relativeLabel", "reason"]);
    return {
      relativeLabel: asRelativePath(exclusion.relativeLabel),
      reason: asBmadRendererText(exclusion.reason, 1_024),
    };
  });
  const secretFindings = review.secretFindings.map((value) => {
    const finding = asRecord(value);
    assertExactKeys(finding, ["relativeLabel", "kind", "occurrenceCount"]);
    return {
      relativeLabel: asRelativePath(finding.relativeLabel),
      kind: asBmadIdentifier(finding.kind),
      occurrenceCount: asPositiveSafeInteger(finding.occurrenceCount),
    };
  });
  const totalOutboundBytes = asPositiveSafeInteger(review.totalOutboundBytes);
  const totalTokenEstimate = asPositiveSafeInteger(review.totalTokenEstimate);
  if (
    items.reduce((total, item) => total + item.outboundByteCount, 0) !== totalOutboundBytes
    || items.reduce((total, item) => total + item.tokenEstimate, 0) !== totalTokenEstimate
    || totalOutboundBytes > bmadProjectionLimits.reviewTextBytes
  ) {
    return fail();
  }
  const projection: BmadHelpContextReviewProjection = {
    workspaceId: asContractId(review.workspaceId),
    workspaceGrantEpoch: asPositiveSafeInteger(review.workspaceGrantEpoch),
    runId: asContractId(review.runId),
    sessionId: asContractId(review.sessionId),
    destinationLabel: asBmadRendererText(review.destinationLabel, 256),
    developmentOnly: review.developmentOnly,
    consentDisclosure: asBmadRendererText(review.consentDisclosure, 4_096),
    manifestHash: asSha256(review.manifestHash),
    purpose: asBmadIdentifier(review.purpose),
    region: asModelRegion(review.region),
    retentionMode: "transient_no_store",
    expiresAt: asPositiveSafeInteger(review.expiresAt),
    items,
    exclusions,
    secretFindings,
    totalOutboundBytes,
    totalTokenEstimate,
    redactionLimitation: asBmadRendererText(review.redactionLimitation, 1_024),
  };
  if (utf8Length(JSON.stringify(projection)) > bmadProjectionLimits.reviewProjectionBytes) return fail();
  return projection;
}

function parseBmadHelpApproval(value: unknown): BmadHelpApprovedProjection {
  const approval = asRecord(value);
  assertExactKeys(approval, ["manifestHash", "decisionId", "expiresAt", "sendEligible"]);
  if (approval.sendEligible !== true) return fail();
  return {
    manifestHash: asSha256(approval.manifestHash),
    decisionId: asContractId(approval.decisionId),
    expiresAt: asPositiveSafeInteger(approval.expiresAt),
    sendEligible: true,
  };
}

function parseBmadHelpCancellation(value: unknown): BmadHelpCancelledProjection {
  const cancellation = asRecord(value);
  assertExactKeys(cancellation, ["manifestHash", "decisionId"]);
  return {
    manifestHash: asSha256(cancellation.manifestHash),
    decisionId: asContractId(cancellation.decisionId),
  };
}

const bmadTerminalReasons = new Set<BmadHelpTerminalProjection["reason"]>([
  "cancelled", "consent_expired", "consent_consumed", "failed",
]);

function parseBmadHelpTerminal(
  value: unknown,
  expectedWorkspaceId: string,
): BmadHelpTerminalProjection {
  const terminal = asRecord(value);
  assertExactKeys(terminal, ["workspaceId", "reason", "resumable", "sendEligible"]);
  const workspaceId = asContractId(terminal.workspaceId);
  const reason = asBmadIdentifier(terminal.reason) as BmadHelpTerminalProjection["reason"];
  if (
    workspaceId !== expectedWorkspaceId
    || !bmadTerminalReasons.has(reason)
    || terminal.resumable !== false
    || terminal.sendEligible !== false
  ) return fail();
  return { workspaceId, reason, resumable: false, sendEligible: false };
}

const evidenceClasses = new Set<BmadHelpEvidenceClass>([
  "authoritative", "user_asserted", "heuristic", "contextual",
]);
const noRecommendationReasons = new Set<BmadHelpNoRecommendationReason>([
  "catalog_evidence_absent", "completion_evidence_ambiguous", "dependency_unavailable",
]);

function parseBmadCompletedRecommendation(value: unknown): BmadHelpCompletedRecommendationProjection {
  const recommendation = asRecord(value);
  if (recommendation.recommendationKind === "recommended_capability") {
    assertExactKeys(recommendation, [
      "recommendationKind", "displayName", "moduleCode", "skillName", "action",
      "evidenceClass", "guidanceRequired", "rationaleSummary", "createdAt",
    ]);
    const evidenceClass = asBmadIdentifier(recommendation.evidenceClass) as BmadHelpEvidenceClass;
    if (!evidenceClasses.has(evidenceClass)) return fail();
    return {
      recommendationKind: "recommended_capability",
      displayName: asBmadRendererText(recommendation.displayName, 256),
      moduleCode: asBmadIdentifier(recommendation.moduleCode),
      skillName: asBmadIdentifier(recommendation.skillName),
      action: asNullableBmadIdentifier(recommendation.action),
      evidenceClass,
      guidanceRequired: asBoolean(recommendation.guidanceRequired),
      rationaleSummary: asBmadRendererText(recommendation.rationaleSummary, 4_096),
      createdAt: asPositiveSafeInteger(recommendation.createdAt),
    };
  }
  if (recommendation.recommendationKind !== "no_recommendation") return fail();
  assertExactKeys(recommendation, ["recommendationKind", "reasonCode", "createdAt"]);
  const reasonCode = asBmadIdentifier(recommendation.reasonCode) as BmadHelpNoRecommendationReason;
  if (!noRecommendationReasons.has(reasonCode)) return fail();
  return {
    recommendationKind: "no_recommendation",
    reasonCode,
    createdAt: asPositiveSafeInteger(recommendation.createdAt),
  };
}

function parseBmadReceipt(value: unknown): BmadHelpReceiptSummaryProjection {
  const receipt = asRecord(value);
  assertExactKeys(receipt, [
    "schemaVersion", "receiptId", "status", "retentionMode", "region", "inputBytes",
    "outputBytes", "startedAt", "completedAt",
  ]);
  if (
    receipt.schemaVersion !== BMAD_MODEL_RECEIPT_SCHEMA
    || receipt.status !== "succeeded"
    || receipt.retentionMode !== "transient_no_store"
  ) return fail();
  const inputBytes = asPositiveSafeInteger(receipt.inputBytes);
  const outputBytes = asPositiveSafeInteger(receipt.outputBytes);
  const startedAt = asPositiveSafeInteger(receipt.startedAt);
  const completedAt = asPositiveSafeInteger(receipt.completedAt);
  if (
    inputBytes > bmadProjectionLimits.receiptInputBytes
    || outputBytes > bmadProjectionLimits.receiptOutputBytes
    || startedAt > completedAt
  ) return fail();
  return {
    schemaVersion: BMAD_MODEL_RECEIPT_SCHEMA,
    receiptId: asContractId(receipt.receiptId),
    status: "succeeded",
    retentionMode: "transient_no_store",
    region: asModelRegion(receipt.region),
    inputBytes,
    outputBytes,
    startedAt,
    completedAt,
  };
}

function parseBmadHelpCompleted(
  value: unknown,
  expectedWorkspaceId: string,
): BmadHelpRunCompletedProjection {
  const result = asRecord(value);
  assertExactKeys(result, [
    "schemaVersion", "runKind", "lifecycle", "workspaceId", "runId", "sessionId",
    "runnable", "completionClaimed", "recommendation", "receipt",
  ]);
  if (
    result.schemaVersion !== BMAD_HELP_COMPLETED_SCHEMA
    || result.runKind !== "bmad_help"
    || result.lifecycle !== "completed"
    || result.workspaceId !== expectedWorkspaceId
    || result.runnable !== false
    || result.completionClaimed !== true
  ) return fail();
  const recommendation = parseBmadCompletedRecommendation(result.recommendation);
  const receipt = parseBmadReceipt(result.receipt);
  if (recommendation.createdAt < receipt.completedAt) return fail();
  const projection: BmadHelpRunCompletedProjection = {
    schemaVersion: BMAD_HELP_COMPLETED_SCHEMA,
    runKind: "bmad_help",
    lifecycle: "completed",
    workspaceId: asContractId(result.workspaceId),
    runId: asContractId(result.runId),
    sessionId: asContractId(result.sessionId),
    runnable: false,
    completionClaimed: true,
    recommendation,
    receipt,
  };
  if (utf8Length(JSON.stringify(projection)) > bmadProjectionLimits.modelResponseBytes) return fail();
  return projection;
}

function parseBmadInstalledSkill(value: unknown): BmadInstalledSkillProjection {
  const skill = asRecord(value);
  assertExactKeys(skill, [
    "moduleCode",
    "skillName",
    "displayName",
    "description",
    "actions",
    "entrypointKind",
    "distributionProfile",
    "installProfile",
    "validationProfile",
    "availability",
    "blockerCodes",
    "hiddenFromHelp",
  ]);
  if (!Array.isArray(skill.actions) || skill.actions.length > bmadProjectionLimits.actionsPerSkill) {
    return fail();
  }
  return {
    moduleCode: asBmadIdentifier(skill.moduleCode),
    skillName: asBmadIdentifier(skill.skillName),
    displayName: asBmadSafeText(skill.displayName, bmadProjectionLimits.identifierBytes),
    description: asBmadSafeText(skill.description, bmadProjectionLimits.descriptionBytes),
    actions: skill.actions.map(asBmadIdentifier),
    entrypointKind: asBmadEntrypointKind(skill.entrypointKind),
    distributionProfile: asBmadIdentifier(skill.distributionProfile),
    installProfile: asBmadIdentifier(skill.installProfile),
    validationProfile: asBmadIdentifier(skill.validationProfile),
    availability: asBmadAvailability(skill.availability),
    blockerCodes: parseBmadBlockerCodes(skill.blockerCodes),
    hiddenFromHelp: asBoolean(skill.hiddenFromHelp),
  };
}

function parseBmadHelpAction(value: unknown): BmadHelpActionProjection {
  const action = asRecord(value);
  assertExactKeys(action, [
    "moduleCode",
    "skillName",
    "action",
    "displayName",
    "menuCode",
    "description",
    "requiredGuidance",
    "expectedArtifacts",
    "availability",
    "blockerCodes",
  ]);
  if (
    !Array.isArray(action.expectedArtifacts)
    || action.expectedArtifacts.length > bmadProjectionLimits.expectedArtifacts
  ) {
    return fail();
  }
  return {
    moduleCode: asBmadIdentifier(action.moduleCode),
    skillName: asBmadIdentifier(action.skillName),
    action: asNullableBmadIdentifier(action.action),
    displayName: asBmadSafeText(action.displayName, bmadProjectionLimits.identifierBytes),
    menuCode: asNullableBmadIdentifier(action.menuCode),
    description: asBmadSafeText(action.description, bmadProjectionLimits.descriptionBytes),
    requiredGuidance: asBoolean(action.requiredGuidance),
    expectedArtifacts: action.expectedArtifacts.map((artifact) =>
      asBmadSafeText(artifact, bmadProjectionLimits.identifierBytes)
    ),
    availability: asBmadAvailability(action.availability),
    blockerCodes: parseBmadBlockerCodes(action.blockerCodes),
  };
}

function parseBmadAgentMenu(value: unknown): BmadAgentMenuProjection {
  const menu = asRecord(value);
  assertExactKeys(menu, [
    "code",
    "description",
    "targetKind",
    "displayLabel",
    "availability",
    "availabilityReason",
  ]);
  return {
    code: asBmadIdentifier(menu.code),
    description: asBmadSafeText(menu.description, bmadProjectionLimits.descriptionBytes),
    targetKind: asBmadMenuTargetKind(menu.targetKind),
    displayLabel: asBmadSafeText(menu.displayLabel, bmadProjectionLimits.identifierBytes),
    availability: asBmadAvailability(menu.availability),
    availabilityReason: asNullableBmadBlockerCode(menu.availabilityReason),
  };
}

function parseBmadMethodAgent(value: unknown): BmadMethodAgentProjection {
  const agent = asRecord(value);
  assertExactKeys(agent, [
    "moduleCode",
    "agentCode",
    "name",
    "title",
    "icon",
    "team",
    "description",
    "availability",
    "blockerCodes",
    "menus",
  ]);
  if (!Array.isArray(agent.menus) || agent.menus.length > bmadProjectionLimits.menusPerAgent) {
    return fail();
  }
  const menus = agent.menus.map(parseBmadAgentMenu);
  assertUniqueIdentities(menus.map(({ code }) => code));
  return {
    moduleCode: asBmadIdentifier(agent.moduleCode),
    agentCode: asBmadIdentifier(agent.agentCode),
    name: asBmadSafeText(agent.name, bmadProjectionLimits.identifierBytes),
    title: asBmadSafeText(agent.title, bmadProjectionLimits.identifierBytes),
    icon: asBmadSafeText(agent.icon, bmadProjectionLimits.iconBytes),
    team: asBmadIdentifier(agent.team),
    description: asBmadSafeText(agent.description, bmadProjectionLimits.descriptionBytes),
    availability: asBmadAvailability(agent.availability),
    blockerCodes: parseBmadBlockerCodes(agent.blockerCodes),
    menus,
  };
}

function parseBmadLibrarySnapshot(value: unknown): BmadLibrarySnapshot {
  const snapshot = asRecord(value);
  assertExactKeys(snapshot, [
    "schemaVersion",
    "scope",
    "source",
    "installedSkills",
    "helpActions",
    "methodAgents",
    "nextCursor",
  ]);
  if (
    snapshot.schemaVersion !== BMAD_LIBRARY_SCHEMA
    || snapshot.scope !== "installed_method"
    || !Array.isArray(snapshot.installedSkills)
    || snapshot.installedSkills.length > bmadProjectionLimits.installedSkills
    || !Array.isArray(snapshot.helpActions)
    || snapshot.helpActions.length > bmadProjectionLimits.helpActions
    || !Array.isArray(snapshot.methodAgents)
    || snapshot.methodAgents.length > bmadProjectionLimits.methodAgents
  ) {
    return fail();
  }
  const installedSkills = snapshot.installedSkills.map(parseBmadInstalledSkill);
  const helpActions = snapshot.helpActions.map(parseBmadHelpAction);
  const methodAgents = snapshot.methodAgents.map(parseBmadMethodAgent);
  assertUniqueIdentities(installedSkills.map(({ moduleCode, skillName }) =>
    `${moduleCode}\u001f${skillName}`
  ));
  assertUniqueIdentities(helpActions.map(({ moduleCode, skillName, action }) =>
    `${moduleCode}\u001f${skillName}\u001f${action ?? "\u0000"}`
  ));
  assertUniqueIdentities(helpActions.flatMap(({ menuCode, moduleCode }) =>
    menuCode === null ? [] : [`${moduleCode}\u001f${menuCode}`]
  ));
  assertUniqueIdentities(methodAgents.map(({ moduleCode, agentCode }) =>
    `${moduleCode}\u001f${agentCode}`
  ));
  const projection: BmadLibrarySnapshot = {
    schemaVersion: BMAD_LIBRARY_SCHEMA,
    scope: "installed_method",
    source: parseBmadProjectionSource(snapshot.source),
    installedSkills,
    helpActions,
    methodAgents,
    nextCursor: asBmadCursor(snapshot.nextCursor),
  };
  if (utf8Length(JSON.stringify(projection)) > bmadProjectionLimits.responseBytes) {
    return fail();
  }
  return projection;
}

function parseBmadLibrarySnapshotReply(
  value: unknown,
  requestId: string,
): { projection: BmadLibrarySnapshot; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "bmad_library_snapshot") {
    return fail();
  }
  return {
    projection: parseBmadLibrarySnapshot(data.value),
    sequence: parsed.sequence,
  };
}

function parseBmadHelpRunCreatedReply(
  value: unknown,
  requestId: string,
  workspaceId: string,
): { projection: BmadHelpRunCreatedProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "bmad_help_run_created") {
    return fail();
  }
  const projection = parseBmadHelpRunCreated(data.value, workspaceId);
  if (parsed.receipt.operationId !== projection.runId) {
    return fail();
  }
  return { projection, sequence: parsed.sequence };
}

function parseModelAuthStatusReply(
  value: unknown,
  requestId: string,
): { projection: ModelAuthStatusProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "model_auth_status") return fail();
  return { projection: parseModelAuthStatus(parsed.data.value), sequence: parsed.sequence };
}

function parseBmadHelpReviewReply(
  value: unknown,
  requestId: string,
  workspaceId: string,
): { projection: BmadHelpContextReviewProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "bmad_help_review") return fail();
  return {
    projection: parseBmadHelpReview(parsed.data.value, workspaceId),
    sequence: parsed.sequence,
  };
}

function parseBmadHelpApprovedReply(
  value: unknown,
  requestId: string,
): { projection: BmadHelpApprovedProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "bmad_help_approved") return fail();
  return { projection: parseBmadHelpApproval(parsed.data.value), sequence: parsed.sequence };
}

function parseBmadHelpCancelledReply(
  value: unknown,
  requestId: string,
): { projection: BmadHelpCancelledProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "bmad_help_cancelled") return fail();
  return { projection: parseBmadHelpCancellation(parsed.data.value), sequence: parsed.sequence };
}

function parseBmadHelpCompletedReply(
  value: unknown,
  requestId: string,
  workspaceId: string,
): { projection: BmadHelpRunCompletedProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "bmad_help_run_completed") return fail();
  return {
    projection: parseBmadHelpCompleted(parsed.data.value, workspaceId),
    sequence: parsed.sequence,
  };
}

function parseLatestBmadHelpRunReply(
  value: unknown,
  requestId: string,
  workspaceId: string,
  workspaceGrantEpoch: number,
): { result: LatestBmadHelpRunResult; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) {
    return fail();
  }
  const data = parsed.data;
  if (data.kind === "no_bmad_help_run") {
    assertExactKeys(data, ["kind"]);
    return { result: { kind: "no_run" }, sequence: parsed.sequence };
  }
  if (data.kind === "bmad_help_projection_unavailable") {
    assertExactKeys(data, ["kind"]);
    return {
      result: { kind: "projection_unavailable" },
      sequence: parsed.sequence,
    };
  }
  if (data.kind === "bmad_help_review") {
    assertExactKeys(data, ["kind", "value"]);
    const review = parseBmadHelpReview(data.value, workspaceId);
    if (review.workspaceGrantEpoch !== workspaceGrantEpoch) return fail();
    return { result: { kind: "review", review }, sequence: parsed.sequence };
  }
  if (data.kind === "bmad_help_approved_lifecycle") {
    assertExactKeys(data, ["kind", "value"]);
    const lifecycle = asRecord(data.value);
    assertExactKeys(lifecycle, ["review", "approval"]);
    const review = parseBmadHelpReview(lifecycle.review, workspaceId);
    const approval = parseBmadHelpApproval(lifecycle.approval);
    if (
      review.workspaceGrantEpoch !== workspaceGrantEpoch
      || approval.manifestHash !== review.manifestHash
      || approval.expiresAt > review.expiresAt
    ) return fail();
    return {
      result: { kind: "approved", review, approval },
      sequence: parsed.sequence,
    };
  }
  if (data.kind === "bmad_help_terminal") {
    assertExactKeys(data, ["kind", "value"]);
    return {
      result: { kind: "terminal", terminal: parseBmadHelpTerminal(data.value, workspaceId) },
      sequence: parsed.sequence,
    };
  }
  if (data.kind === "bmad_help_run_interrupted") {
    assertExactKeys(data, ["kind", "value"]);
    return {
      result: { kind: "interrupted", run: parseBmadHelpRunCreated(data.value, workspaceId) },
      sequence: parsed.sequence,
    };
  }
  if (data.kind === "bmad_help_run_completed") {
    assertExactKeys(data, ["kind", "value"]);
    return {
      result: { kind: "completed", result: parseBmadHelpCompleted(data.value, workspaceId) },
      sequence: parsed.sequence,
    };
  }
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "bmad_help_run_created") {
    return fail();
  }
  return {
    result: {
      kind: "retained",
      run: parseBmadHelpRunCreated(data.value, workspaceId),
    },
    sequence: parsed.sequence,
  };
}

function rustLineCount(content: string): number {
  if (content.length === 0) {
    return 1;
  }
  const lineFeeds = content.match(/\n/gu)?.length ?? 0;
  return Math.max(1, lineFeeds + (content.endsWith("\n") ? 0 : 1));
}

function parseContextPreviewReply(
  value: unknown,
  requestId: string,
  workspaceId: string,
  selectedPaths: readonly string[],
): { projection: ContextPreviewProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "context_preview") {
    return fail();
  }
  const projection = asRecord(data.value);
  assertExactKeys(projection, [
    "workspaceId",
    "manifestHash",
    "items",
    "totalBytes",
    "estimatedTokens",
    "modelTarget",
  ]);
  if (
    projection.workspaceId !== workspaceId
    || !Array.isArray(projection.items)
    || projection.items.length !== selectedPaths.length
    || projection.modelTarget !== null
  ) {
    return fail();
  }
  const items = projection.items.map((value, index): ContextItemProjection => {
    const item = asRecord(value);
    assertExactKeys(item, [
      "relativePath",
      "startLine",
      "endLine",
      "reason",
      "contentHash",
      "classification",
      "redactions",
      "byteCount",
      "estimatedTokens",
      "content",
    ]);
    const relativePath = asRelativePath(item.relativePath);
    if (relativePath !== selectedPaths[index]) {
      return fail();
    }
    const content = asTextContent(item.content, workspaceReadLimits.contextBytes);
    const byteCount = asUnsignedInteger(item.byteCount);
    const estimatedTokens = asUnsignedInteger(item.estimatedTokens);
    const startLine = asUnsignedInteger(item.startLine);
    const endLine = asUnsignedInteger(item.endLine);
    if (
      item.reason !== "Selected for this task"
      || item.classification !== "source"
      || !Array.isArray(item.redactions)
      || item.redactions.length !== 0
      || byteCount !== utf8Length(content)
      || estimatedTokens !== Math.floor((byteCount + 3) / 4)
      || startLine !== 1
      || endLine !== rustLineCount(content)
    ) {
      return fail();
    }
    return {
      relativePath,
      startLine: 1,
      endLine,
      reason: "Selected for this task",
      contentHash: asSha256(item.contentHash),
      classification: "source",
      redactions: [],
      byteCount,
      estimatedTokens,
      content,
    };
  });
  const totalBytes = asUnsignedInteger(projection.totalBytes);
  const estimatedTokens = asUnsignedInteger(projection.estimatedTokens);
  const summedBytes = items.reduce((sum, item) => sum + item.byteCount, 0);
  const summedTokens = items.reduce((sum, item) => sum + item.estimatedTokens, 0);
  if (
    totalBytes !== summedBytes
    || totalBytes > workspaceReadLimits.contextBytes
    || estimatedTokens !== summedTokens
  ) {
    return fail();
  }
  return {
    projection: {
      workspaceId: asContractId(projection.workspaceId),
      manifestHash: asSha256(projection.manifestHash),
      items,
      totalBytes,
      estimatedTokens,
      modelTarget: null,
    },
    sequence: parsed.sequence,
  };
}

function parseProjectionSnapshot(value: unknown): ProjectionSnapshot {
  const snapshot = asRecord(value);
  assertExactKeys(snapshot, [
    "sequence",
    "generatedAt",
    "bootMode",
    "workspaceCount",
    "activeSessionId",
  ]);
  return {
    sequence: asUnsignedInteger(snapshot.sequence),
    generatedAt: asUnsignedInteger(snapshot.generatedAt),
    bootMode: asBootMode(snapshot.bootMode),
    workspaceCount: asUnsignedInteger(snapshot.workspaceCount),
    activeSessionId: asNullableContractId(snapshot.activeSessionId),
  };
}

function parseProjectionEventPayload(value: unknown): ProjectionEventPayload {
  const event = asRecord(value);
  assertExactKeys(event, ["type", "projection"]);
  const projection = asRecord(event.projection);
  switch (event.type) {
    case "boot_state_changed":
      assertExactKeys(projection, ["mode"]);
      return { type: event.type, projection: { mode: asBootMode(projection.mode) } };
    case "workspace_changed":
      assertExactKeys(projection, ["workspaceId"]);
      return { type: event.type, projection: { workspaceId: asContractId(projection.workspaceId) } };
    case "bmad.projection_changed":
      assertExactKeys(projection, ["scope"]);
      if (projection.scope !== "library") {
        return fail();
      }
      return { type: event.type, projection: { scope: projection.scope } };
    default:
      return fail();
  }
}

function parseProjectionEvent(value: unknown): ProjectionEvent {
  const event = asRecord(value);
  assertExactKeys(event, ["sequence", "occurredAt", "event"]);
  return {
    sequence: asUnsignedInteger(event.sequence),
    occurredAt: asUnsignedInteger(event.occurredAt),
    event: parseProjectionEventPayload(event.event),
  };
}

function parseProjectionReply(
  value: unknown,
  rendererSessionId: string,
  expectedStatus: "snapshot" | "events",
): ProjectionSnapshot | ProjectionEvent[] {
  const reply = asRecord(value);
  if (reply.schemaVersion !== PROJECTION_REPLY_SCHEMA) {
    return fail();
  }
  if (reply.status === "error") {
    assertExactKeys(reply, ["schemaVersion", "rendererSessionId", "status", "error"]);
    if (reply.rendererSessionId !== null && reply.rendererSessionId !== rendererSessionId) {
      return fail();
    }
    throw new HostCommandError(parseLocalHostError(reply.error));
  }
  if (reply.status !== expectedStatus || reply.rendererSessionId !== rendererSessionId) {
    return fail();
  }
  if (expectedStatus === "snapshot") {
    assertExactKeys(reply, ["schemaVersion", "rendererSessionId", "status", "snapshot"]);
    return parseProjectionSnapshot(reply.snapshot);
  }
  assertExactKeys(reply, ["schemaVersion", "rendererSessionId", "status", "events"]);
  if (!Array.isArray(reply.events) || reply.events.length > 512) {
    return fail();
  }
  const events = reply.events.map(parseProjectionEvent);
  for (let index = 1; index < events.length; index += 1) {
    if (events[index]!.sequence <= events[index - 1]!.sequence) {
      return fail();
    }
  }
  return events;
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

function buildWorkspaceListEnvelope(
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

function buildWorkspaceRevocationEnvelope(
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

function buildBmadHelpRunEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
  workspaceGrantEpoch: number,
  currentIntent: string,
): CommandEnvelope<"run.create", {
  workspaceId: string;
  workspaceGrantEpoch: number;
  runKind: "bmad_help";
  currentIntent: string;
}> {
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

function buildLatestBmadHelpRunEnvelope(
  binding: HostBinding,
  requestId: string,
  issuedAt: number,
  workspaceId: string,
  workspaceGrantEpoch: number,
): CommandEnvelope<"bmad.help.latest", {
  workspaceId: string;
  workspaceGrantEpoch: number;
}> {
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

function buildBmadModelEnvelope<
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

function buildReadOnlyEnvelope<TCommand extends Exclude<
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
>>(
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

export interface DesktopHostClientOptions {
  invoke: TauriInvoke;
  now?: () => number;
  requestId?: () => string;
}

export class DesktopHostClient {
  readonly #invoke: TauriInvoke;
  readonly #now: () => number;
  readonly #requestId: () => string;
  readonly #directoryCursors = new Map<string, { workspaceId: string; relativeDirectory: string }>();
  readonly #directoryEntryPaths = new Map<string, Set<string>>();
  readonly #pendingDirectoryCursors = new Set<string>();
  #bootstrap: BootstrapReply | null = null;
  #bootstrapAttempt = 0;
  #bootstrapGeneration = 0;
  #projectionSequence: number | null = null;

  constructor({ invoke, now = Date.now, requestId = () => crypto.randomUUID() }: DesktopHostClientOptions) {
    this.#invoke = invoke;
    this.#now = now;
    this.#requestId = requestId;
  }

  async bootstrap(): Promise<BootstrapReply> {
    const attempt = this.#bootstrapAttempt + 1;
    this.#bootstrapAttempt = attempt;
    this.#bootstrapGeneration += 1;
    this.#bootstrap = null;
    this.#projectionSequence = null;
    this.#directoryCursors.clear();
    this.#directoryEntryPaths.clear();
    this.#pendingDirectoryCursors.clear();
    const reply = parseBootstrapReply(await this.#invoke("host_bootstrap"));
    if (attempt !== this.#bootstrapAttempt) {
      return fail();
    }
    this.#bootstrap = reply;
    this.#projectionSequence = reply.projectionSequence;
    return reply;
  }

  async selectWorkspace(): Promise<WorkspaceSelection> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    if (
      bootstrap.bootMode !== "ready"
      || !bootstrap.supportedCommands.includes("workspace.select_folder")
    ) {
      throw new HostCapabilityError("Folder selection is unavailable in the current host mode.");
    }
    const requestId = this.#requestId();
    const envelope = buildWorkspaceSelectionEnvelope(bootstrap, requestId, this.#now());
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseWorkspaceSelectionReply(reply, requestId);
    const currentBootstrap = this.requireBootstrapGeneration(bootstrapGeneration);
    if (
      currentBootstrap.bootMode !== "ready"
      || !currentBootstrap.supportedCommands.includes("workspace.select_folder")
    ) {
      throw new HostCapabilityError("Folder selection is unavailable in the current host mode.");
    }
    this.advanceProjectionSequence(parsed.sequence);
    if (parsed.selection.kind === "workspace_selected") {
      const selectedWorkspace = parsed.selection.value;
      this.replaceWorkspaces([
        selectedWorkspace,
        ...currentBootstrap.workspaces.filter(
          ({ workspaceId }) => workspaceId !== selectedWorkspace.workspaceId,
        ),
      ]);
    }
    return parsed.selection;
  }

  async listWorkspaces(): Promise<WorkspaceProjection[]> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    if (!bootstrap.supportedCommands.includes("workspace.list")) {
      throw new HostCapabilityError("Local workspace status is unavailable in the current host mode.");
    }
    const requestId = this.#requestId();
    const envelope = buildWorkspaceListEnvelope(bootstrap, requestId, this.#now());
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseWorkspaceListReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(parsed.sequence);
    this.replaceWorkspaces(parsed.workspaces);
    return parsed.workspaces;
  }

  async revokeWorkspace(
    expectedWorkspaceValue: WorkspaceProjection,
  ): Promise<WorkspaceRevocationResult> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    const expectedWorkspace = parseWorkspace(expectedWorkspaceValue);
    const currentWorkspace = bootstrap.workspaces.find(
      ({ workspaceId }) => workspaceId === expectedWorkspace.workspaceId,
    );
    if (
      bootstrap.bootMode !== "ready"
      || !bootstrap.supportedCommands.includes("workspace.revoke")
      || !currentWorkspace
      || currentWorkspace.grantEpoch !== expectedWorkspace.grantEpoch
      || !sameWorkspaceIdentity(currentWorkspace, expectedWorkspace)
    ) {
      throw new HostCapabilityError("That workspace access is no longer available.");
    }

    const requestId = this.#requestId();
    const envelope = buildWorkspaceRevocationEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      expectedWorkspace.workspaceId,
    );
    const issuedAfterSequence = this.#projectionSequence;
    if (issuedAfterSequence === null) {
      return fail();
    }
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseWorkspaceRevocationReply(reply, requestId, expectedWorkspace);

    const currentBootstrap = this.requireBootstrapGeneration(bootstrapGeneration);
    const stillCurrent = currentBootstrap.workspaces.find(
      ({ workspaceId }) => workspaceId === expectedWorkspace.workspaceId,
    );
    if (
      stillCurrent
      && (
        stillCurrent.grantEpoch !== expectedWorkspace.grantEpoch
        || !sameWorkspaceIdentity(stillCurrent, expectedWorkspace)
      )
    ) {
      return fail();
    }

    // Validation and replay checks must complete before local authority state changes.
    this.advanceMutationSequence(parsed.sequence, issuedAfterSequence);
    this.clearWorkspaceTraversal(expectedWorkspace.workspaceId);
    const workspaces = currentBootstrap.workspaces.filter(
      ({ workspaceId }) => workspaceId !== expectedWorkspace.workspaceId,
    );
    this.replaceWorkspaces(workspaces);
    return { revoked: parsed.revoked, workspaces: [...workspaces] };
  }

  async listWorkspaceEntries(
    workspaceId: string,
    cursor: string | null = null,
    limit = 100,
  ): Promise<WorkspaceEntriesProjection> {
    const bootstrap = this.requireWorkspaceCommand(workspaceId, "workspace.list_entries");
    const bootstrapGeneration = this.#bootstrapGeneration;
    if (!Number.isInteger(limit) || limit < 1 || limit > workspaceReadLimits.entryPage) {
      throw new HostCapabilityError("The requested Explorer page size is outside the desktop limit.");
    }
    const cursorBinding = cursor === null
      ? { workspaceId, relativeDirectory: "." }
      : this.requireDirectoryCursor(cursor, workspaceId);
    if (cursor !== null) {
      if (this.#pendingDirectoryCursors.has(cursor)) {
        throw new HostCapabilityError("That Explorer page is already being requested.");
      }
      this.#pendingDirectoryCursors.add(cursor);
    }
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "workspace.list_entries",
      { workspaceId, cursor, limit },
    );
    let reply: unknown;
    try {
      reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    } finally {
      if (cursor !== null) {
        this.#pendingDirectoryCursors.delete(cursor);
        this.#directoryCursors.delete(cursor);
      }
    }
    const parsed = parseWorkspaceEntriesReply(reply, requestId, {
      workspaceId,
      relativeDirectory: cursorBinding.relativeDirectory,
      limit,
    });
    // A concurrent revocation must prevent an in-flight page from restoring stale cursors.
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireWorkspaceCommand(workspaceId, "workspace.list_entries");
    const isFreshRootPage = cursor === null;
    const directoryKey = this.directoryKey(workspaceId, cursorBinding.relativeDirectory);
    const observedPaths = isFreshRootPage
      ? new Set<string>()
      : new Set(this.#directoryEntryPaths.get(directoryKey) ?? []);
    for (const entry of parsed.projection.entries) {
      const foldedPath = entry.relativePath.toLocaleLowerCase("en-US");
      if (observedPaths.has(foldedPath)) {
        fail();
      }
      observedPaths.add(foldedPath);
    }
    if (
      cursor !== null && parsed.projection.nextCursor === cursor
      || (parsed.projection.entries.length === 0 && parsed.projection.nextCursor !== null)
    ) {
      fail();
    }

    const projectedCursorBindings = new Map<
      string,
      { workspaceId: string; relativeDirectory: string }
    >();
    if (parsed.projection.nextCursor) {
      projectedCursorBindings.set(parsed.projection.nextCursor, cursorBinding);
    }
    for (const entry of parsed.projection.entries) {
      if (!entry.childCursor) {
        continue;
      }
      const childBinding = { workspaceId, relativeDirectory: entry.relativePath };
      const projectedBinding = projectedCursorBindings.get(entry.childCursor);
      if (
        projectedBinding
        && (
          projectedBinding.workspaceId !== childBinding.workspaceId
          || projectedBinding.relativeDirectory !== childBinding.relativeDirectory
        )
      ) {
        fail();
      }
      projectedCursorBindings.set(entry.childCursor, childBinding);
    }
    for (const [projectedCursor, projectedBinding] of projectedCursorBindings) {
      const existingBinding = this.#directoryCursors.get(projectedCursor);
      if (
        existingBinding
        && !(isFreshRootPage && existingBinding.workspaceId === workspaceId)
        && (
          existingBinding.workspaceId !== projectedBinding.workspaceId
          || existingBinding.relativeDirectory !== projectedBinding.relativeDirectory
        )
      ) {
        fail();
      }
    }

    this.advanceProjectionSequence(parsed.sequence);
    if (isFreshRootPage) {
      this.clearWorkspaceTraversal(workspaceId);
    }
    this.#directoryEntryPaths.set(directoryKey, observedPaths);
    for (const [projectedCursor, projectedBinding] of projectedCursorBindings) {
      this.#directoryCursors.set(projectedCursor, projectedBinding);
    }
    return parsed.projection;
  }

  async readWorkspaceText(
    workspaceId: string,
    relativePathValue: string,
    maxBytes = 128 * 1024,
  ): Promise<WorkspaceTextProjection> {
    const bootstrap = this.requireWorkspaceCommand(workspaceId, "workspace.read_text");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const relativePath = asRelativePath(relativePathValue);
    if (!Number.isInteger(maxBytes) || maxBytes < 1 || maxBytes > workspaceReadLimits.readBytes) {
      throw new HostCapabilityError("The requested text preview size is outside the desktop limit.");
    }
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "workspace.read_text",
      { workspaceId, relativePath, maxBytes },
    );
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseWorkspaceTextReply(reply, requestId, relativePath, maxBytes);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireWorkspaceCommand(workspaceId, "workspace.read_text");
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async searchWorkspace(
    workspaceId: string,
    queryValue: string,
    maxResults = 100,
  ): Promise<WorkspaceSearchMatch[]> {
    const bootstrap = this.requireWorkspaceCommand(workspaceId, "workspace.search");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const query = queryValue.trim();
    if (
      query.length === 0
      || query.includes("\0")
      || hasUnpairedSurrogate(query)
      || utf8Length(query) > workspaceReadLimits.searchQueryBytes
      || !Number.isInteger(maxResults)
      || maxResults < 1
      || maxResults > workspaceReadLimits.searchResults
    ) {
      throw new HostCapabilityError("The search request is outside the desktop read limits.");
    }
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "workspace.search",
      { workspaceId, query, maxResults },
    );
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseSearchResultsReply(reply, requestId, maxResults);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireWorkspaceCommand(workspaceId, "workspace.search");
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.matches;
  }

  async scanBmad(workspaceId: string): Promise<BmadScanProjection> {
    const bootstrap = this.requireWorkspaceCommand(workspaceId, "bmad.scan");
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "bmad.scan",
      { workspaceId },
    );
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseBmadScanReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireWorkspaceCommand(workspaceId, "bmad.scan");
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async bmadLibrarySnapshot(cursor?: string | null): Promise<BmadLibrarySnapshot> {
    const bootstrap = this.requireBmadLibraryCommand();
    const bootstrapGeneration = this.#bootstrapGeneration;
    const normalizedCursor = asBmadCursor(cursor ?? null);
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "bmad.library.snapshot",
      { scope: "installed_method", cursor: normalizedCursor },
    );
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseBmadLibrarySnapshotReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadLibraryCommand();
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async modelAuthStatus(): Promise<ModelAuthStatusProjection> {
    return this.dispatchModelAuthCommand("model.auth.status");
  }

  async modelAuthSignIn(): Promise<ModelAuthStatusProjection> {
    return this.dispatchModelAuthCommand("model.auth.sign_in");
  }

  async modelAuthSignOut(): Promise<ModelAuthStatusProjection> {
    return this.dispatchModelAuthCommand("model.auth.sign_out");
  }

  async prepareBmadHelp(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
  ): Promise<BmadHelpContextReviewProjection> {
    const command = "bmad.help.prepare" as const;
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      command,
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const requestId = this.#requestId();
    const envelope = buildBmadModelEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      command,
      { workspaceId, workspaceGrantEpoch: asPositiveSafeInteger(workspaceGrantEpoch) },
    );
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseBmadHelpReviewReply(reply, requestId, workspaceId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(workspaceId, workspaceGrantEpoch, command);
    if (parsed.projection.workspaceGrantEpoch !== workspaceGrantEpoch) return fail();
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async approveBmadHelp(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    manifestHashValue: string,
  ): Promise<BmadHelpApprovedProjection> {
    const command = "bmad.help.approve" as const;
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      command,
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const manifestHash = asSha256(manifestHashValue);
    const requestId = this.#requestId();
    const envelope = buildBmadModelEnvelope(bootstrap, requestId, this.#now(), command, {
      workspaceId,
      workspaceGrantEpoch: asPositiveSafeInteger(workspaceGrantEpoch),
      manifestHash,
    });
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseBmadHelpApprovedReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(workspaceId, workspaceGrantEpoch, command);
    if (parsed.projection.manifestHash !== manifestHash) return fail();
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async cancelBmadHelp(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    manifestHashValue: string,
    decisionIdValue: string,
  ): Promise<BmadHelpCancelledProjection> {
    const command = "bmad.help.cancel" as const;
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      command,
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const manifestHash = asSha256(manifestHashValue);
    const decisionId = asContractId(decisionIdValue);
    const requestId = this.#requestId();
    const envelope = buildBmadModelEnvelope(bootstrap, requestId, this.#now(), command, {
      workspaceId,
      workspaceGrantEpoch: asPositiveSafeInteger(workspaceGrantEpoch),
      manifestHash,
      decisionId,
    });
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseBmadHelpCancelledReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(workspaceId, workspaceGrantEpoch, command);
    if (
      parsed.projection.manifestHash !== manifestHash
      || parsed.projection.decisionId !== decisionId
    ) return fail();
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async submitBmadHelp(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    manifestHashValue: string,
    decisionIdValue: string,
  ): Promise<BmadHelpRunCompletedProjection> {
    const command = "bmad.help.submit" as const;
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      command,
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const requestId = this.#requestId();
    const envelope = buildBmadModelEnvelope(bootstrap, requestId, this.#now(), command, {
      workspaceId,
      workspaceGrantEpoch: asPositiveSafeInteger(workspaceGrantEpoch),
      manifestHash: asSha256(manifestHashValue),
      decisionId: asContractId(decisionIdValue),
    });
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseBmadHelpCompletedReply(reply, requestId, workspaceId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(workspaceId, workspaceGrantEpoch, command);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async createBmadHelpRun(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    currentIntentValue: string,
  ): Promise<BmadHelpRunCreatedProjection> {
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      "run.create",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const currentIntent = asBmadHelpIntent(currentIntentValue);
    const requestId = this.#requestId();
    const envelope = buildBmadHelpRunEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      workspaceId,
      workspaceGrantEpoch,
      currentIntent,
    );
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseBmadHelpRunCreatedReply(reply, requestId, workspaceId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(workspaceId, workspaceGrantEpoch, "run.create");
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async latestBmadHelpRun(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
  ): Promise<LatestBmadHelpRunResult> {
    const bootstrap = this.requireBmadHelpWorkspaceCommand(
      workspaceIdValue,
      workspaceGrantEpoch,
      "bmad.help.latest",
    );
    const bootstrapGeneration = this.#bootstrapGeneration;
    const workspaceId = asContractId(workspaceIdValue);
    const requestId = this.#requestId();
    const envelope = buildLatestBmadHelpRunEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      workspaceId,
      workspaceGrantEpoch,
    );
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseLatestBmadHelpRunReply(
      reply,
      requestId,
      workspaceId,
      workspaceGrantEpoch,
    );
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireBmadHelpWorkspaceCommand(
      workspaceId,
      workspaceGrantEpoch,
      "bmad.help.latest",
    );
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.result;
  }

  async previewContext(
    workspaceId: string,
    relativePathValues: readonly string[],
  ): Promise<ContextPreviewProjection> {
    const bootstrap = this.requireWorkspaceCommand(workspaceId, "context.preview");
    const bootstrapGeneration = this.#bootstrapGeneration;
    if (
      relativePathValues.length === 0
      || relativePathValues.length > workspaceReadLimits.contextPaths
    ) {
      throw new HostCapabilityError("Select between 1 and 100 text files for context review.");
    }
    const relativePaths = relativePathValues.map(asRelativePath);
    assertUniqueRelativePaths(relativePaths);
    const requestId = this.#requestId();
    const envelope = buildReadOnlyEnvelope(
      bootstrap,
      requestId,
      this.#now(),
      "context.preview",
      { workspaceId, relativePaths },
    );
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseContextPreviewReply(
      reply,
      requestId,
      workspaceId,
      relativePaths,
    );
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireWorkspaceCommand(workspaceId, "context.preview");
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  async projectionSnapshot(scope: ProjectionScope = {}): Promise<ProjectionSnapshot> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    const request = this.buildProjectionRequest(scope, null);
    const reply = await this.#invoke("host_projection_snapshot", { body: JSON.stringify(request) });
    const snapshot = parseProjectionReply(
      reply,
      bootstrap.rendererSessionId,
      "snapshot",
    ) as ProjectionSnapshot;
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.advanceProjectionSequence(snapshot.sequence);
    if (snapshot.bootMode === "read_only_recovery") {
      this.adoptReadOnlyRecovery(snapshot.sequence);
    }
    return snapshot;
  }

  async projectionEvents(
    afterSequence: number,
    scope: ProjectionScope = {},
  ): Promise<ProjectionEvent[]> {
    const bootstrap = this.requireBootstrap();
    const bootstrapGeneration = this.#bootstrapGeneration;
    const request = this.buildProjectionRequest(scope, asUnsignedInteger(afterSequence));
    const reply = await this.#invoke("host_projection_events", { body: JSON.stringify(request) });
    const events = parseProjectionReply(
      reply,
      bootstrap.rendererSessionId,
      "events",
    ) as ProjectionEvent[];
    this.requireBootstrapGeneration(bootstrapGeneration);
    if (events.some(({ sequence }) => sequence <= afterSequence)) {
      return fail();
    }
    if (events.length > 0) {
      this.advanceProjectionSequence(events.at(-1)!.sequence);
    }
    const recoveryEvent = events.find(
      ({ event }) => event.type === "boot_state_changed"
        && event.projection.mode === "read_only_recovery",
    );
    if (recoveryEvent) {
      this.adoptReadOnlyRecovery(recoveryEvent.sequence);
    }
    return events;
  }

  private requireWorkspaceCommand(
    workspaceIdValue: string,
    command: Extract<
      RendererDispatchCommand,
      | "workspace.list_entries"
      | "workspace.read_text"
      | "workspace.search"
      | "bmad.scan"
      | "context.preview"
    >,
  ): BootstrapReply {
    const bootstrap = this.requireBootstrap();
    const workspaceId = asContractId(workspaceIdValue);
    if (
      bootstrap.bootMode !== "ready"
      || !bootstrap.supportedCommands.includes(command)
      || !bootstrap.workspaces.some((workspace) =>
        workspace.workspaceId === workspaceId && workspace.permissions === "read_only"
      )
    ) {
      throw new HostCapabilityError("That read-only workspace capability is unavailable.");
    }
    return bootstrap;
  }

  private requireBmadLibraryCommand(): BootstrapReply {
    const bootstrap = this.requireBootstrap();
    if (
      bootstrap.bootMode !== "ready"
      || !bootstrap.supportedCommands.includes("bmad.library.snapshot")
    ) {
      throw new HostCapabilityError("The Method library is unavailable in the current host mode.");
    }
    return bootstrap;
  }

  private async dispatchModelAuthCommand(
    command: "model.auth.status" | "model.auth.sign_in" | "model.auth.sign_out",
  ): Promise<ModelAuthStatusProjection> {
    const bootstrap = this.requireModelAuthCommand(command);
    const bootstrapGeneration = this.#bootstrapGeneration;
    const requestId = this.#requestId();
    const envelope = buildBmadModelEnvelope(bootstrap, requestId, this.#now(), command, {});
    const reply = await this.#invoke("host_dispatch", { body: JSON.stringify(envelope) });
    const parsed = parseModelAuthStatusReply(reply, requestId);
    this.requireBootstrapGeneration(bootstrapGeneration);
    this.requireModelAuthCommand(command);
    this.advanceProjectionSequence(parsed.sequence);
    return parsed.projection;
  }

  private requireModelAuthCommand(
    command: "model.auth.status" | "model.auth.sign_in" | "model.auth.sign_out",
  ): BootstrapReply {
    const bootstrap = this.requireBootstrap();
    if (bootstrap.bootMode !== "ready" || !bootstrap.supportedCommands.includes(command)) {
      throw new HostCapabilityError("Model identity is unavailable in the current host mode.");
    }
    return bootstrap;
  }

  private requireBmadHelpWorkspaceCommand(
    workspaceIdValue: string,
    workspaceGrantEpoch: number,
    command:
      | "bmad.help.prepare"
      | "bmad.help.approve"
      | "bmad.help.cancel"
      | "bmad.help.submit"
      | "bmad.help.latest"
      | "run.create",
  ): BootstrapReply {
    const bootstrap = this.requireBootstrap();
    const workspaceId = asContractId(workspaceIdValue);
    if (
      !Number.isSafeInteger(workspaceGrantEpoch)
      || workspaceGrantEpoch < 1
      || bootstrap.bootMode !== "ready"
      || !bootstrap.supportedCommands.includes(command)
      || !bootstrap.workspaces.some((workspace) =>
        workspace.workspaceId === workspaceId
        && workspace.grantEpoch === workspaceGrantEpoch
        && workspace.permissions === "read_only"
      )
    ) {
      throw new HostCapabilityError("Method guidance is unavailable for that workspace grant.");
    }
    return bootstrap;
  }

  private requireDirectoryCursor(
    cursorValue: string,
    workspaceId: string,
  ): { workspaceId: string; relativeDirectory: string } {
    const cursor = asNullableOpaqueCursor(cursorValue);
    if (cursor === null) {
      return fail();
    }
    const binding = this.#directoryCursors.get(cursor);
    if (!binding || binding.workspaceId !== workspaceId) {
      throw new HostCapabilityError("The Explorer page cursor is unavailable; refresh the workspace.");
    }
    return binding;
  }

  private directoryKey(workspaceId: string, relativeDirectory: string): string {
    return `${workspaceId}\u001f${relativeDirectory}`;
  }

  private clearWorkspaceTraversal(workspaceId: string): void {
    for (const [cursor, binding] of this.#directoryCursors) {
      if (binding.workspaceId === workspaceId) {
        this.#pendingDirectoryCursors.delete(cursor);
        this.#directoryCursors.delete(cursor);
      }
    }
    const keyPrefix = `${workspaceId}\u001f`;
    for (const key of this.#directoryEntryPaths.keys()) {
      if (key.startsWith(keyPrefix)) {
        this.#directoryEntryPaths.delete(key);
      }
    }
  }

  private replaceWorkspaces(workspaces: WorkspaceProjection[]): void {
    const bootstrap = this.requireBootstrap();
    const visibleWorkspaceIds = new Set(workspaces.map(({ workspaceId }) => workspaceId));
    for (const [cursor, binding] of this.#directoryCursors) {
      if (!visibleWorkspaceIds.has(binding.workspaceId)) {
        this.#pendingDirectoryCursors.delete(cursor);
        this.#directoryCursors.delete(cursor);
      }
    }
    for (const key of this.#directoryEntryPaths.keys()) {
      const separatorIndex = key.indexOf("\u001f");
      if (separatorIndex < 0 || !visibleWorkspaceIds.has(key.slice(0, separatorIndex))) {
        this.#directoryEntryPaths.delete(key);
      }
    }
    this.#bootstrap = { ...bootstrap, workspaces };
  }

  private requireBootstrap(): BootstrapReply {
    if (!this.#bootstrap) {
      throw new HostCapabilityError("The Windows host has not completed bootstrap.");
    }
    return this.#bootstrap;
  }

  private requireBootstrapGeneration(expectedGeneration: number): BootstrapReply {
    if (this.#bootstrapGeneration !== expectedGeneration) {
      return fail();
    }
    return this.requireBootstrap();
  }

  private advanceProjectionSequence(nextSequence: number): void {
    if (this.#projectionSequence !== null && nextSequence < this.#projectionSequence) {
      fail();
    }
    this.#projectionSequence = nextSequence;
  }

  private advanceMutationSequence(nextSequence: number, issuedAfterSequence: number): void {
    if (nextSequence <= issuedAfterSequence || this.#projectionSequence === null) {
      fail();
    }
    this.#projectionSequence = Math.max(this.#projectionSequence, nextSequence);
  }

  private adoptReadOnlyRecovery(sequence: number): void {
    if (!this.#bootstrap) {
      return fail();
    }
    this.#directoryCursors.clear();
    this.#directoryEntryPaths.clear();
    this.#pendingDirectoryCursors.clear();
    this.#bootstrap = {
      ...this.#bootstrap,
      bootMode: "read_only_recovery",
      supportedCommands: this.#bootstrap.supportedCommands.filter(
        (command) => command === "app.get_boot_state" || command === "workspace.list",
      ),
      projectionSequence: Math.max(this.#bootstrap.projectionSequence, sequence),
    };
  }

  private buildProjectionRequest(scope: ProjectionScope, afterSequence: number | null) {
    const bootstrap = this.requireBootstrap();
    return {
      schemaVersion: PROJECTION_REQUEST_SCHEMA,
      rendererSessionId: bootstrap.rendererSessionId,
      installationId: bootstrap.installationId,
      workspaceId: scope.workspaceId === undefined ? null : asContractId(scope.workspaceId),
      sessionId: null,
      afterSequence,
    };
  }
}

export type HostRuntime =
  | { kind: "browser_demo"; client: null; bootstrap: null }
  | { kind: "ready"; client: DesktopHostClient; bootstrap: BootstrapReply }
  | { kind: "read_only_recovery"; client: DesktopHostClient; bootstrap: BootstrapReply }
  | { kind: "unavailable"; client: null; bootstrap: null; message: string };

export interface HostRuntimeDependencies {
  isTauri?: () => boolean;
  loadInvoke?: () => Promise<TauriInvoke>;
  now?: () => number;
  requestId?: () => string;
}

function defaultIsTauri(): boolean {
  return typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined;
}

async function defaultLoadInvoke(): Promise<TauriInvoke> {
  const { invoke } = await import("@tauri-apps/api/core");
  return (command, args) => invoke<unknown>(command, args);
}

export async function initializeHostRuntime({
  isTauri = defaultIsTauri,
  loadInvoke = defaultLoadInvoke,
  now,
  requestId,
}: HostRuntimeDependencies = {}): Promise<HostRuntime> {
  if (!isTauri()) {
    return { kind: "browser_demo", client: null, bootstrap: null };
  }
  try {
    const client = new DesktopHostClient({
      invoke: await loadInvoke(),
      ...(now ? { now } : {}),
      ...(requestId ? { requestId } : {}),
    });
    const bootstrap = await client.bootstrap();
    return {
      kind: bootstrap.bootMode === "ready" ? "ready" : "read_only_recovery",
      client,
      bootstrap,
    };
  } catch {
    return {
      kind: "unavailable",
      client: null,
      bootstrap: null,
      message: "The signed Windows host could not be verified. Local actions remain unavailable.",
    };
  }
}

let defaultRuntimePromise: Promise<HostRuntime> | null = null;

export function getDefaultHostRuntime(): Promise<HostRuntime> {
  defaultRuntimePromise ??= initializeHostRuntime();
  return defaultRuntimePromise;
}

export function getSafeHostMessage(error: unknown): string {
  if (error instanceof HostCommandError || error instanceof HostCapabilityError) {
    return error.message;
  }
  return "The Windows host could not complete that request. Nothing was changed.";
}
