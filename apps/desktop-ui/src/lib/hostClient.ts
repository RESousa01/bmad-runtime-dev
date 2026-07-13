const BOOTSTRAP_SCHEMA = "desktop-bootstrap.v1" as const;
const COMMAND_SCHEMA = "desktop-ipc-command.v1" as const;
const DISPATCH_REPLY_SCHEMA = "desktop-dispatch-reply.v1" as const;
const PROJECTION_REQUEST_SCHEMA = "desktop-projection-request.v1" as const;
const PROJECTION_REPLY_SCHEMA = "desktop-projection-reply.v1" as const;

export const desktopHostCommands = [
  "app.get_boot_state",
  "workspace.select_folder",
  "workspace.list",
  "workspace.revoke",
  "workspace.list_entries",
  "workspace.read_text",
  "workspace.search",
  "bmad.scan",
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
  | { type: "workspace_changed"; projection: { workspaceId: string } };

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
  | "context.preview";

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
): { data: Record<string, unknown>; sequence: number } {
  const reply = asRecord(value);
  if (reply.schemaVersion !== DISPATCH_REPLY_SCHEMA) {
    return fail();
  }
  if (reply.status === "error") {
    assertExactKeys(reply, ["schemaVersion", "requestId", "sequence", "status", "error"]);
    if (reply.requestId !== requestId) {
      return fail();
    }
    asUnsignedInteger(reply.sequence);
    throw new HostCommandError(parseLocalHostError(reply.error));
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
  asUnsignedInteger(receipt.acceptedAt);
  asNullableContractId(receipt.operationId);

  return {
    data: asRecord(reply.data),
    sequence: asUnsignedInteger(reply.sequence),
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

function buildReadOnlyEnvelope<TCommand extends Exclude<
  RendererDispatchCommand,
  "workspace.select_folder" | "workspace.list" | "workspace.revoke"
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
