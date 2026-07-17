import {
  type BmadActivation,
  type BmadAssetKind,
  type BmadAssetProjection,
  type BmadScanProjection,
  type BmadStatus,
  BOOTSTRAP_SCHEMA,
  type BootstrapReply,
  type ContextItemProjection,
  type ContextPreviewProjection,
  type DesktopHostCommand,
  desktopHostCommands,
  DISPATCH_REPLY_SCHEMA,
  HostCommandError,
  type LocalHostError,
  type WorkspaceEntriesProjection,
  type WorkspaceEntryKind,
  type WorkspaceProjection,
  workspaceReadLimits,
  type WorkspaceSearchMatch,
  type WorkspaceSelection,
  type WorkspaceTextProjection,
  type WorkspaceTreeEntry,
} from "./contracts";
import {
  asBoolean,
  asBootMode,
  asBoundedString,
  asContractId,
  asNullableContractId,
  asNullableOpaqueCursor,
  asRecord,
  asRelativePath,
  asRendererSafeMessage,
  asSafeDisplayName,
  assertExactKeys,
  assertUniqueRelativePaths,
  asSha256,
  asSingleLineText,
  asTextContent,
  asUnsignedInteger,
  asWorkspacePermission,
  fail,
  isImmediateChild,
  utf8Length,
} from "./validation";

export function parseWorkspace(value: unknown): WorkspaceProjection {
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

export function parseSupportedCommands(value: unknown): DesktopHostCommand[] {
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
    reply.schemaVersion !== BOOTSTRAP_SCHEMA ||
    !Array.isArray(reply.workspaces) ||
    reply.workspaces.length > 256
  ) {
    return fail();
  }
  const workspaces = reply.workspaces.map(parseWorkspace);
  if (
    new Set(workspaces.map(({ workspaceId }) => workspaceId)).size !==
    workspaces.length
  ) {
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
      supportedCommands.length !== recoveryCommands.size ||
      supportedCommands.some((command) => !recoveryCommands.has(command))
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

export function parseLocalHostError(value: unknown): LocalHostError {
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

export function parseDispatchReply(
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
    assertExactKeys(reply, [
      "schemaVersion",
      "requestId",
      "sequence",
      "status",
      "error",
    ]);
    const error = parseLocalHostError(reply.error);
    const isUnboundExpiredRenderer =
      reply.requestId === null &&
      error.code === "renderer_session_expired" &&
      error.correlationId === null;
    if (reply.requestId !== requestId && !isUnboundExpiredRenderer) {
      return fail();
    }
    asUnsignedInteger(reply.sequence);
    throw new HostCommandError(error);
  }
  if (reply.status !== "ok") {
    return fail();
  }
  assertExactKeys(reply, [
    "schemaVersion",
    "requestId",
    "sequence",
    "status",
    "receipt",
    "data",
  ]);
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

export function parseWorkspaceSelectionReply(
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
    selection: {
      kind: "workspace_selected",
      value: parseWorkspace(data.value),
    },
    sequence: parsed.sequence,
  };
}

export function parseWorkspaceListReply(
  value: unknown,
  requestId: string,
): { workspaces: WorkspaceProjection[]; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const { data } = parsed;
  assertExactKeys(data, ["kind", "value"]);
  if (
    data.kind !== "workspace_list" ||
    !Array.isArray(data.value) ||
    data.value.length > 256
  ) {
    return fail();
  }
  const workspaces = data.value.map(parseWorkspace);
  if (
    new Set(workspaces.map(({ workspaceId }) => workspaceId)).size !==
    workspaces.length
  ) {
    return fail();
  }
  return { workspaces, sequence: parsed.sequence };
}

export function sameWorkspaceIdentity(
  left: WorkspaceProjection,
  right: WorkspaceProjection,
): boolean {
  return (
    left.workspaceId === right.workspaceId &&
    left.projectId === right.projectId &&
    left.displayName === right.displayName &&
    left.permissions === right.permissions
  );
}

export function parseWorkspaceRevocationReply(
  value: unknown,
  requestId: string,
  expected: WorkspaceProjection,
): { revoked: WorkspaceProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const { data } = parsed;
  assertExactKeys(data, ["kind", "value"]);
  if (
    data.kind !== "workspace_revoked" ||
    expected.grantEpoch >= Number.MAX_SAFE_INTEGER
  ) {
    return fail();
  }
  const revoked = parseWorkspace(data.value);
  if (
    !sameWorkspaceIdentity(revoked, expected) ||
    revoked.grantEpoch !== expected.grantEpoch + 1
  ) {
    return fail();
  }
  return { revoked, sequence: parsed.sequence };
}

export function parseWorkspaceEntry(
  value: unknown,
  relativeDirectory: string,
): WorkspaceTreeEntry {
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

export function parseWorkspaceEntriesReply(
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
    projection.workspaceId !== expected.workspaceId ||
    !Array.isArray(projection.entries) ||
    projection.entries.length > expected.limit
  ) {
    return fail();
  }
  const entries = projection.entries.map((entry) =>
    parseWorkspaceEntry(entry, expected.relativeDirectory),
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

export function parseWorkspaceTextReply(
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
    (!truncated && byteCount !== contentBytes) ||
    (truncated && byteCount <= contentBytes)
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

export function parseSearchResultsReply(
  value: unknown,
  requestId: string,
  maximumResults: number,
): { matches: WorkspaceSearchMatch[]; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (
    data.kind !== "search_results" ||
    !Array.isArray(data.value) ||
    data.value.length > maximumResults
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
  const identities = matches.map(
    ({ relativePath, line }) =>
      `${relativePath.toLocaleLowerCase("en-US")}:${line}`,
  );
  if (new Set(identities).size !== identities.length) {
    return fail();
  }
  return { matches, sequence: parsed.sequence };
}

export function parseBmadScanReply(
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
  if (
    !statuses.has(status) ||
    !Array.isArray(projection.assets) ||
    projection.assets.length > 256
  ) {
    return fail();
  }
  const methodKinds = new Set<BmadAssetKind>([
    "method_configuration",
    "agent",
    "workflow",
  ]);
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
      (methodKinds.has(assetKind) && activation !== "read_only") ||
      (draftKinds.has(assetKind) && activation !== "inactive_draft")
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
  const hasDraft = assets.some(
    ({ activation }) => activation === "inactive_draft",
  );
  const expectedStatus: BmadStatus = hasMethod
    ? hasDraft
      ? "method_and_builder_drafts_detected"
      : "method_detected"
    : hasDraft
      ? "builder_drafts_detected"
      : "not_detected";
  if (status !== expectedStatus) {
    return fail();
  }
  return {
    projection: { status, assets, truncated: asBoolean(projection.truncated) },
    sequence: parsed.sequence,
  };
}

export function rustLineCount(content: string): number {
  if (content.length === 0) {
    return 1;
  }
  const lineFeeds = content.match(/\n/gu)?.length ?? 0;
  return Math.max(1, lineFeeds + (content.endsWith("\n") ? 0 : 1));
}

export function parseContextPreviewReply(
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
    projection.workspaceId !== workspaceId ||
    !Array.isArray(projection.items) ||
    projection.items.length !== selectedPaths.length ||
    projection.modelTarget !== null
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
    const content = asTextContent(
      item.content,
      workspaceReadLimits.contextBytes,
    );
    const byteCount = asUnsignedInteger(item.byteCount);
    const estimatedTokens = asUnsignedInteger(item.estimatedTokens);
    const startLine = asUnsignedInteger(item.startLine);
    const endLine = asUnsignedInteger(item.endLine);
    if (
      item.reason !== "Selected for this task" ||
      item.classification !== "source" ||
      !Array.isArray(item.redactions) ||
      item.redactions.length !== 0 ||
      byteCount !== utf8Length(content) ||
      estimatedTokens !== Math.floor((byteCount + 3) / 4) ||
      startLine !== 1 ||
      endLine !== rustLineCount(content)
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
  const summedTokens = items.reduce(
    (sum, item) => sum + item.estimatedTokens,
    0,
  );
  if (
    totalBytes !== summedBytes ||
    totalBytes > workspaceReadLimits.contextBytes ||
    estimatedTokens !== summedTokens
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
