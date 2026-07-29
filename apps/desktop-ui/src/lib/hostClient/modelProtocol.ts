/**
 * Renderer-side mirror of `crates/desktop-ipc/src/model.rs`.
 *
 * Every type here is hand-written and re-validated rather than imported from
 * `@sapphirus/contracts` — that is the deliberate repo convention: the
 * renderer is untrusted, so it does not trust a shared type either, it
 * re-checks the host's own reply against the same rules the host used to
 * build it.
 *
 * Three invariants carried over from the Rust module, because a validator
 * that forgot one of them would defeat the point of re-validating at all:
 *
 * * A catalog or selection identifier is checked for the same disclosure
 *   markers the host scans for (`http://`, `.azure.com`, `bearer `, ...).
 *   The projection type has nowhere to carry a deployment or an endpoint, but
 *   a compromised or buggy host could still smuggle one into a name.
 * * `textDelta` and `argumentsDelta` are *not* disclosure-scanned. They are
 *   free-form model output and tool-call arguments, which may legitimately
 *   contain a URL or the word "bearer" without disclosing any connection
 *   authority. Only the control/bidi-override scan applies to them, exactly
 *   as in `check_stream_text` on the Rust side.
 * * `RequiresExplicitChoice` never carries a replacement profile. Collapsing
 *   it to a nullable "current selection" is exactly the bug this module
 *   exists to prevent: a user mid-draft must not discover afterwards that a
 *   different model answered.
 */

import {
  asBmadNonemptySafeText,
  asBoolean,
  asContractId,
  asRecord,
  assertExactKeys,
  asUnsignedInteger,
  fail,
  hasUnpairedSurrogate,
  hasUnsafeTextCharacter,
} from "./validation";
import { parseDispatchReply } from "./workspaceProtocol";

const MAX_IDENTIFIER_BYTES = 128;
const MAX_DISPLAY_NAME_BYTES = 128;
const MAX_DESCRIPTION_BYTES = 256;
const MAX_FAMILY_BYTES = 64;
const MAX_STATUS_DETAIL_BYTES = 160;
const MAX_VARIANT_DISPLAY_NAME_BYTES = 32;
const MAX_REASON_BYTES = 256;
const MAX_TOOL_NAME_BYTES = 128;
const MAX_TEXT_DELTA_BYTES = 4 * 1024;
const MAX_ARGUMENTS_DELTA_BYTES = 4 * 1024;

const MAX_PROFILES = 64;
const MAX_UNAVAILABLE_PROFILES = 64;
const MAX_VARIANTS_PER_PROFILE = 8;
const MIN_VARIANTS_PER_PROFILE = 1;

/// Mirrors `ProjectedLimits.contextTokens` / `.maxInputTokens` and
/// `MAX_CONTEXT_OR_INPUT_TOKENS` in `model.rs`.
const MAX_CONTEXT_OR_INPUT_TOKENS = 100_000_000;
/// Mirrors `ProjectedLimits.maxOutputTokens`, `ModelVariantProjection`'s own
/// `maxOutputTokens`, and `MAX_OUTPUT_TOKENS` in `model.rs`.
const MAX_OUTPUT_TOKENS = 10_000_000;
/// Mirrors `ProjectedLimits.maxAttachmentBytes` and `MAX_ATTACHMENT_BYTES` in
/// `model.rs`.
const MAX_ATTACHMENT_BYTES = 134_217_728;
/// The only provider a projected profile may name. A constant, not a lookup:
/// Azure authority is not representable here at all.
const MODEL_PROFILE_PROVIDER = "microsoft_foundry" as const;

export const MODEL_CATALOG_SCHEMA = "sapphirus.model-catalog-projection.v1" as const;
export const MODEL_SELECTION_SCHEMA = "sapphirus.model-selection-projection.v1" as const;
export const MODEL_STREAM_EVENT_SCHEMA = "sapphirus.model-stream-event.v1" as const;

/// Mirrors `DISCLOSURE_MARKERS` in `model.rs`. Kept identical so the renderer
/// cannot disagree with the host about what counts as a leaked connection
/// detail.
const DISCLOSURE_MARKERS = [
  "http://",
  "https://",
  ".azure.com",
  ".openai.com",
  "api-key",
  "apikey",
  "bearer ",
  "deployment=",
] as const;

function hasDisclosureMarker(value: string): boolean {
  const folded = value.toLowerCase();
  return DISCLOSURE_MARKERS.some((marker) => folded.includes(marker));
}

/// An identifier the host also disclosure-scans: a profile id, a variant id,
/// a catalog version, a tool-call id, a resolved profile/variant id.
function asModelIdentifier(value: unknown): string {
  if (
    typeof value !== "string"
    || value.length === 0
    || value.length > MAX_IDENTIFIER_BYTES
    || !/^[A-Za-z0-9._:-]+$/u.test(value)
    || hasDisclosureMarker(value)
  ) {
    return fail();
  }
  return value;
}

/// Display text the host disclosure-scans: a display name or a summary.
function asModelDisplayText(value: unknown, maximumBytes: number): string {
  const text = asBmadNonemptySafeText(value, maximumBytes);
  if (hasDisclosureMarker(text)) {
    return fail();
  }
  return text;
}

/// Free-form stream text: a text delta or an arguments delta. Deliberately
/// not disclosure-scanned -- see the module doc.
function asStreamText(value: unknown, maximumBytes: number): string {
  if (
    typeof value !== "string"
    || value.length === 0
    || hasUnpairedSurrogate(value)
    || hasUnsafeTextCharacter(value)
  ) {
    return fail();
  }
  const byteLength = new TextEncoder().encode(value).byteLength;
  if (byteLength > maximumBytes) {
    return fail();
  }
  return value;
}

export type ModelCatalogState = "fresh" | "stale" | "degraded";

const modelCatalogStates: ReadonlySet<string> = new Set<ModelCatalogState>([
  "fresh",
  "stale",
  "degraded",
]);

function asModelCatalogState(value: unknown): ModelCatalogState {
  const state = asBmadNonemptySafeText(value, 16);
  if (!modelCatalogStates.has(state)) return fail();
  return state as ModelCatalogState;
}

/// Availability of a profile the user may invoke. Deliberately a separate
/// closed vocabulary from `ModelBlockedAvailability` -- the canonical contract
/// splits them so a selectable projection cannot describe itself as blocked.
export type ModelSelectableAvailability = "available" | "degraded";

const modelSelectableAvailabilities: ReadonlySet<string> = new Set<ModelSelectableAvailability>([
  "available",
  "degraded",
]);

/// How much a variant is asked to reason before answering.
export type ModelReasoningEffort = "none" | "low" | "medium" | "high";

const modelReasoningEfforts: ReadonlySet<string> = new Set<ModelReasoningEffort>([
  "none",
  "low",
  "medium",
  "high",
]);

/// What a model profile can do. Every field is a fact the organization
/// published, not a connection detail.
export interface ModelCapabilitiesProjection {
  readonly textInput: boolean;
  readonly imageInput: boolean;
  readonly pdfInput: boolean;
  readonly attachments: boolean;
  readonly toolCalling: boolean;
  readonly structuredOutput: boolean;
  readonly reasoning: boolean;
}

/// Limits Forge will actually enforce for one profile. Every field is
/// nullable and that nullability is load-bearing: `null` means the
/// organization published no qualified value for it, and callers must render
/// that as unknown, never as zero or unlimited.
export interface ModelLimitsProjection {
  readonly contextTokens: number | null;
  readonly maxInputTokens: number | null;
  readonly maxOutputTokens: number | null;
  readonly maxAttachmentBytes: number | null;
}

/// One named operating point of a profile.
export interface ModelVariantProjection {
  readonly variantId: string;
  readonly displayName: string;
  readonly reasoningEffort: ModelReasoningEffort;
  readonly maxOutputTokens: number;
}

export interface ModelProfileProjection {
  readonly profileId: string;
  readonly displayName: string;
  readonly description: string;
  readonly family: string | null;
  readonly provider: typeof MODEL_PROFILE_PROVIDER;
  readonly availability: ModelSelectableAvailability;
  readonly statusDetail: string | null;
  readonly capabilities: ModelCapabilitiesProjection;
  readonly limits: ModelLimitsProjection;
  readonly variants: readonly ModelVariantProjection[];
  readonly defaultVariantId: string;
  readonly qualifiedAt: string | null;
}

export type ModelBlockedAvailability =
  | "policy_blocked"
  | "not_entitled"
  | "unqualified"
  | "deployment_missing"
  | "catalog_stale";

const modelBlockedAvailabilities: ReadonlySet<string> = new Set<ModelBlockedAvailability>([
  "policy_blocked",
  "not_entitled",
  "unqualified",
  "deployment_missing",
  "catalog_stale",
]);

export type ModelRecoveryAction =
  | "refresh_catalog"
  | "request_entitlement"
  | "awaiting_qualification"
  | "contact_administrator"
  | "select_another_profile";

const modelRecoveryActions: ReadonlySet<string> = new Set<ModelRecoveryAction>([
  "refresh_catalog",
  "request_entitlement",
  "awaiting_qualification",
  "contact_administrator",
  "select_another_profile",
]);

export interface ModelUnavailableProfileProjection {
  readonly profileId: string;
  readonly displayName: string;
  readonly availability: ModelBlockedAvailability;
  readonly recoveryAction: ModelRecoveryAction;
}

export interface ModelCatalogProjection {
  readonly schemaVersion: typeof MODEL_CATALOG_SCHEMA;
  readonly workspaceId: string;
  readonly workspaceGrantEpoch: number;
  readonly appEpoch: number;
  readonly catalogEpoch: number;
  readonly catalogVersion: string;
  readonly catalogState: ModelCatalogState;
  readonly defaultProfileId: string | null;
  readonly profiles: readonly ModelProfileProjection[];
  readonly unavailableProfiles: readonly ModelUnavailableProfileProjection[];
  readonly generatedAt: number;
  readonly expiresAt: number;
  readonly refreshedAt: number;
}

/// Mirrors `check_utc_instant` in `model.rs`: the canonical `UtcInstant`
/// shape is `^[0-9]{4}-(0[1-9]|1[0-2])-([0-2][0-9]|3[01])T([01][0-9]|2[0-3]):
/// [0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$`, a fixed 24-byte width that admits no
/// variation, so a single frozen regex is a complete and honest mirror of the
/// Rust byte-position validator -- not an approximation of it.
const UTC_INSTANT_PATTERN =
  /^[0-9]{4}-(0[1-9]|1[0-2])-([0-2][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$/u;

/// Carried through as the canonical `UtcInstant` string rather than parsed
/// into a numeric instant: the renderer only ever displays this value, and a
/// string preserves the guarantee that what is shown is exactly what the
/// catalog published.
function asUtcInstant(value: unknown): string {
  if (typeof value !== "string" || !UTC_INSTANT_PATTERN.test(value)) {
    return fail();
  }
  return value;
}

/// A nullable limit: `null` stays `null`, never collapses to `0`. Mirrors
/// `check_limits` in `model.rs`, including the per-field ceiling.
function asNullableLimit(value: unknown, maximumBytes: number): number | null {
  if (value === null) return null;
  const parsed = asUnsignedInteger(value);
  if (parsed === 0 || parsed > maximumBytes) return fail();
  return parsed;
}

function parseModelCapabilities(value: unknown): ModelCapabilitiesProjection {
  const capabilities = asRecord(value);
  assertExactKeys(capabilities, [
    "textInput",
    "imageInput",
    "pdfInput",
    "attachments",
    "toolCalling",
    "structuredOutput",
    "reasoning",
  ]);
  return {
    textInput: asBoolean(capabilities.textInput),
    imageInput: asBoolean(capabilities.imageInput),
    pdfInput: asBoolean(capabilities.pdfInput),
    attachments: asBoolean(capabilities.attachments),
    toolCalling: asBoolean(capabilities.toolCalling),
    structuredOutput: asBoolean(capabilities.structuredOutput),
    reasoning: asBoolean(capabilities.reasoning),
  };
}

function parseModelLimits(value: unknown): ModelLimitsProjection {
  const limits = asRecord(value);
  assertExactKeys(limits, [
    "contextTokens",
    "maxInputTokens",
    "maxOutputTokens",
    "maxAttachmentBytes",
  ]);
  const contextTokens = asNullableLimit(limits.contextTokens, MAX_CONTEXT_OR_INPUT_TOKENS);
  const maxInputTokens = asNullableLimit(limits.maxInputTokens, MAX_CONTEXT_OR_INPUT_TOKENS);
  const maxOutputTokens = asNullableLimit(limits.maxOutputTokens, MAX_OUTPUT_TOKENS);
  const maxAttachmentBytes = limits.maxAttachmentBytes === null
    ? null
    : (() => {
      const parsed = asUnsignedInteger(limits.maxAttachmentBytes);
      if (parsed > MAX_ATTACHMENT_BYTES) return fail();
      return parsed;
    })();
  return { contextTokens, maxInputTokens, maxOutputTokens, maxAttachmentBytes };
}

function parseModelVariant(value: unknown): ModelVariantProjection {
  const variant = asRecord(value);
  assertExactKeys(variant, ["variantId", "displayName", "reasoningEffort", "maxOutputTokens"]);
  const reasoningEffort = asBmadNonemptySafeText(variant.reasoningEffort, 16);
  if (!modelReasoningEfforts.has(reasoningEffort)) return fail();
  const maxOutputTokens = asUnsignedInteger(variant.maxOutputTokens);
  if (maxOutputTokens === 0 || maxOutputTokens > MAX_OUTPUT_TOKENS) return fail();
  return {
    variantId: asModelIdentifier(variant.variantId),
    displayName: asModelDisplayText(variant.displayName, MAX_VARIANT_DISPLAY_NAME_BYTES),
    reasoningEffort: reasoningEffort as ModelReasoningEffort,
    maxOutputTokens,
  };
}

function parseModelProfile(value: unknown): ModelProfileProjection {
  const profile = asRecord(value);
  assertExactKeys(profile, [
    "profileId",
    "displayName",
    "description",
    "family",
    "provider",
    "availability",
    "statusDetail",
    "capabilities",
    "limits",
    "variants",
    "defaultVariantId",
    "qualifiedAt",
  ]);
  const profileId = asModelIdentifier(profile.profileId);
  const displayName = asModelDisplayText(profile.displayName, MAX_DISPLAY_NAME_BYTES);
  const description = asModelDisplayText(profile.description, MAX_DESCRIPTION_BYTES);
  const family = profile.family === null
    ? null
    : asModelDisplayText(profile.family, MAX_FAMILY_BYTES);
  if (profile.provider !== MODEL_PROFILE_PROVIDER) return fail();
  const availability = asBmadNonemptySafeText(profile.availability, 16);
  if (!modelSelectableAvailabilities.has(availability)) return fail();
  const statusDetail = profile.statusDetail === null
    ? null
    : asModelDisplayText(profile.statusDetail, MAX_STATUS_DETAIL_BYTES);
  const capabilities = parseModelCapabilities(profile.capabilities);
  const limits = parseModelLimits(profile.limits);
  if (
    !Array.isArray(profile.variants)
    || profile.variants.length < MIN_VARIANTS_PER_PROFILE
    || profile.variants.length > MAX_VARIANTS_PER_PROFILE
  ) {
    return fail();
  }
  const variants = profile.variants.map(parseModelVariant);
  const variantIds = variants.map((variant) => variant.variantId);
  if (new Set(variantIds).size !== variantIds.length) return fail();
  const defaultVariantId = asModelIdentifier(profile.defaultVariantId);
  if (!variantIds.includes(defaultVariantId)) return fail();
  const qualifiedAt = profile.qualifiedAt === null ? null : asUtcInstant(profile.qualifiedAt);
  return {
    profileId,
    displayName,
    description,
    family,
    provider: MODEL_PROFILE_PROVIDER,
    availability: availability as ModelSelectableAvailability,
    statusDetail,
    capabilities,
    limits,
    variants,
    defaultVariantId,
    qualifiedAt,
  };
}

function parseUnavailableModelProfile(value: unknown): ModelUnavailableProfileProjection {
  const profile = asRecord(value);
  assertExactKeys(profile, ["profileId", "displayName", "availability", "recoveryAction"]);
  const availability = asBmadNonemptySafeText(profile.availability, 32);
  if (!modelBlockedAvailabilities.has(availability)) return fail();
  const recoveryAction = asBmadNonemptySafeText(profile.recoveryAction, 32);
  if (!modelRecoveryActions.has(recoveryAction)) return fail();
  return {
    profileId: asModelIdentifier(profile.profileId),
    displayName: asModelDisplayText(profile.displayName, MAX_DISPLAY_NAME_BYTES),
    availability: availability as ModelBlockedAvailability,
    recoveryAction: recoveryAction as ModelRecoveryAction,
  };
}

export function parseModelCatalog(value: unknown): ModelCatalogProjection {
  const catalog = asRecord(value);
  assertExactKeys(catalog, [
    "schemaVersion",
    "workspaceId",
    "workspaceGrantEpoch",
    "appEpoch",
    "catalogEpoch",
    "catalogVersion",
    "catalogState",
    "defaultProfileId",
    "profiles",
    "unavailableProfiles",
    "generatedAt",
    "expiresAt",
    "refreshedAt",
  ]);
  if (catalog.schemaVersion !== MODEL_CATALOG_SCHEMA) return fail();
  const workspaceGrantEpoch = asUnsignedInteger(catalog.workspaceGrantEpoch);
  const appEpoch = asUnsignedInteger(catalog.appEpoch);
  const catalogEpoch = asUnsignedInteger(catalog.catalogEpoch);
  if (workspaceGrantEpoch === 0 || appEpoch === 0 || catalogEpoch === 0) return fail();
  const generatedAt = asUnsignedInteger(catalog.generatedAt);
  const expiresAt = asUnsignedInteger(catalog.expiresAt);
  const refreshedAt = asUnsignedInteger(catalog.refreshedAt);
  if (expiresAt <= generatedAt) return fail();
  if (!Array.isArray(catalog.profiles) || catalog.profiles.length > MAX_PROFILES) return fail();
  if (
    !Array.isArray(catalog.unavailableProfiles)
    || catalog.unavailableProfiles.length > MAX_UNAVAILABLE_PROFILES
  ) {
    return fail();
  }
  const profiles = catalog.profiles.map(parseModelProfile);
  const unavailableProfiles = catalog.unavailableProfiles.map(parseUnavailableModelProfile);
  const allIds = [...profiles.map((p) => p.profileId), ...unavailableProfiles.map((p) => p.profileId)];
  if (new Set(allIds).size !== allIds.length) return fail();
  const defaultProfileId = catalog.defaultProfileId === null
    ? null
    : asModelIdentifier(catalog.defaultProfileId);
  if (
    defaultProfileId !== null
    && !profiles.some((profile) => profile.profileId === defaultProfileId)
  ) {
    return fail();
  }
  return {
    schemaVersion: MODEL_CATALOG_SCHEMA,
    workspaceId: asContractId(catalog.workspaceId),
    workspaceGrantEpoch,
    appEpoch,
    catalogEpoch,
    catalogVersion: asModelIdentifier(catalog.catalogVersion),
    catalogState: asModelCatalogState(catalog.catalogState),
    defaultProfileId,
    profiles,
    unavailableProfiles,
    generatedAt,
    expiresAt,
    refreshedAt,
  };
}

export type ModelSelectionSource =
  | "current_selection"
  | "recent_selection"
  | "catalog_default"
  | "first_selectable_profile";

const modelSelectionSources: ReadonlySet<string> = new Set<ModelSelectionSource>([
  "current_selection",
  "recent_selection",
  "catalog_default",
  "first_selectable_profile",
]);

export type ModelSelectionBlocked = "profile_removed" | "profile_unavailable" | "variant_removed";

const modelSelectionBlockedReasons: ReadonlySet<string> = new Set<ModelSelectionBlocked>([
  "profile_removed",
  "profile_unavailable",
  "variant_removed",
]);

/// Mirrors `ModelSelectionOutcomeInput` one variant for one variant.
/// `RequiresExplicitChoice` never carries a `profileId`/`variantId` -- see the
/// module doc.
export type ModelSelectionProjection =
  | {
    readonly schemaVersion: typeof MODEL_SELECTION_SCHEMA;
    readonly workspaceId: string;
    readonly workspaceGrantEpoch: number;
    readonly appEpoch: number;
    readonly catalogEpoch: number;
    readonly catalogVersion: string;
    readonly status: "selected";
    readonly profileId: string;
    readonly variantId: string;
    readonly source: ModelSelectionSource;
  }
  | {
    readonly schemaVersion: typeof MODEL_SELECTION_SCHEMA;
    readonly workspaceId: string;
    readonly workspaceGrantEpoch: number;
    readonly appEpoch: number;
    readonly catalogEpoch: number;
    readonly catalogVersion: string;
    readonly status: "requires_explicit_choice";
    readonly previousProfileId: string;
    readonly previousVariantId: string;
    readonly blocked: ModelSelectionBlocked;
  }
  | {
    readonly schemaVersion: typeof MODEL_SELECTION_SCHEMA;
    readonly workspaceId: string;
    readonly workspaceGrantEpoch: number;
    readonly appEpoch: number;
    readonly catalogEpoch: number;
    readonly catalogVersion: string;
    readonly status: "none_selectable";
  };

export function parseModelSelection(value: unknown): ModelSelectionProjection {
  const selection = asRecord(value);
  assertExactKeys(selection, [
    "schemaVersion",
    "workspaceId",
    "workspaceGrantEpoch",
    "appEpoch",
    "catalogEpoch",
    "catalogVersion",
    "status",
    "profileId",
    "variantId",
    "source",
    "previousProfileId",
    "previousVariantId",
    "blocked",
  ]);
  if (selection.schemaVersion !== MODEL_SELECTION_SCHEMA) return fail();
  const workspaceId = asContractId(selection.workspaceId);
  const workspaceGrantEpoch = asUnsignedInteger(selection.workspaceGrantEpoch);
  const appEpoch = asUnsignedInteger(selection.appEpoch);
  const catalogEpoch = asUnsignedInteger(selection.catalogEpoch);
  if (workspaceGrantEpoch === 0 || appEpoch === 0 || catalogEpoch === 0) return fail();
  const catalogVersion = asModelIdentifier(selection.catalogVersion);
  const base = { workspaceId, workspaceGrantEpoch, appEpoch, catalogEpoch, catalogVersion };
  const status = asBmadNonemptySafeText(selection.status, 32);
  if (status === "selected") {
    if (
      selection.previousProfileId !== null || selection.previousVariantId !== null
      || selection.blocked !== null
    ) {
      return fail();
    }
    const source = asBmadNonemptySafeText(selection.source, 32);
    if (!modelSelectionSources.has(source)) return fail();
    return {
      schemaVersion: MODEL_SELECTION_SCHEMA,
      ...base,
      status: "selected",
      profileId: asModelIdentifier(selection.profileId),
      variantId: asModelIdentifier(selection.variantId),
      source: source as ModelSelectionSource,
    };
  }
  if (status === "requires_explicit_choice") {
    if (selection.profileId !== null || selection.variantId !== null || selection.source !== null) {
      return fail();
    }
    const blocked = asBmadNonemptySafeText(selection.blocked, 32);
    if (!modelSelectionBlockedReasons.has(blocked)) return fail();
    return {
      schemaVersion: MODEL_SELECTION_SCHEMA,
      ...base,
      status: "requires_explicit_choice",
      previousProfileId: asModelIdentifier(selection.previousProfileId),
      previousVariantId: asModelIdentifier(selection.previousVariantId),
      blocked: blocked as ModelSelectionBlocked,
    };
  }
  if (status === "none_selectable") {
    if (
      selection.profileId !== null || selection.variantId !== null || selection.source !== null
      || selection.previousProfileId !== null || selection.previousVariantId !== null
      || selection.blocked !== null
    ) {
      return fail();
    }
    return { schemaVersion: MODEL_SELECTION_SCHEMA, ...base, status: "none_selectable" };
  }
  return fail();
}

export type ModelStreamTerminal = "completed" | "refused" | "failed" | "cancelled";

const modelStreamTerminals: ReadonlySet<string> = new Set<ModelStreamTerminal>([
  "completed",
  "refused",
  "failed",
  "cancelled",
]);

export type ModelStreamCancellationSource = "user" | "host" | "provider";

const modelStreamCancellationSources: ReadonlySet<string> = new Set<ModelStreamCancellationSource>([
  "user",
  "host",
  "provider",
]);

export interface ModelReceiptSummaryProjection {
  readonly receiptId: string;
  readonly eventCount: number;
  readonly outputByteCount: number;
  readonly truncated: boolean;
  readonly recordedAt: number;
}

function parseModelReceiptSummary(value: unknown): ModelReceiptSummaryProjection {
  const receipt = asRecord(value);
  assertExactKeys(receipt, [
    "receiptId",
    "eventCount",
    "outputByteCount",
    "truncated",
    "recordedAt",
  ]);
  return {
    receiptId: asContractId(receipt.receiptId),
    eventCount: asUnsignedInteger(receipt.eventCount),
    outputByteCount: asUnsignedInteger(receipt.outputByteCount),
    truncated: asBoolean(receipt.truncated),
    recordedAt: asUnsignedInteger(receipt.recordedAt),
  };
}

/// Mirrors `ModelStreamEventBodyProjection`. Each terminal shape carries
/// exactly its own evidence, mirroring the shape-consistency check
/// `project_terminal` runs on the Rust side -- a client that trusted an
/// inconsistent terminal (e.g. `cancelled` with a receipt) could show a run as
/// both cancelled and answered.
export type ModelStreamEventBody =
  | { readonly kind: "started"; readonly resolvedProfileId: string; readonly resolvedVariantId: string }
  | {
    readonly kind: "output_progress";
    readonly outputIndex: number;
    readonly textDelta: string;
    readonly deltaByteCount: number;
  }
  | { readonly kind: "tool_call_started"; readonly toolCallId: string; readonly toolName: string }
  | {
    readonly kind: "tool_call_arguments";
    readonly toolCallId: string;
    readonly argumentsDelta: string;
    readonly deltaByteCount: number;
  }
  | { readonly kind: "tool_call_completed"; readonly toolCallId: string; readonly argumentsByteCount: number }
  | {
    readonly kind: "terminal";
    readonly terminal: "completed";
    readonly receipt: ModelReceiptSummaryProjection;
    readonly cancellationSource: null;
    readonly reasonCode: null;
    readonly retryable: false;
  }
  | {
    readonly kind: "terminal";
    readonly terminal: "refused";
    readonly receipt: null;
    readonly cancellationSource: null;
    readonly reasonCode: string;
    readonly retryable: false;
  }
  | {
    readonly kind: "terminal";
    readonly terminal: "failed";
    readonly receipt: null;
    readonly cancellationSource: null;
    readonly reasonCode: string | null;
    readonly retryable: boolean;
  }
  | {
    readonly kind: "terminal";
    readonly terminal: "cancelled";
    readonly receipt: null;
    readonly cancellationSource: ModelStreamCancellationSource;
    readonly reasonCode: null;
    readonly retryable: false;
  };

export interface ModelStreamEventProjection {
  readonly schemaVersion: typeof MODEL_STREAM_EVENT_SCHEMA;
  readonly workspaceId: string;
  readonly workspaceGrantEpoch: number;
  readonly appEpoch: number;
  readonly taskId: string;
  readonly runId: string;
  readonly sequence: number;
  readonly occurredAt: number;
  readonly body: ModelStreamEventBody;
}

function parseModelStreamEventBody(value: Record<string, unknown>): ModelStreamEventBody {
  assertExactKeys(value, ["kind", "value"]);
  const kind = asBmadNonemptySafeText(value.kind, 32);
  const body = asRecord(value.value);
  switch (kind) {
    case "started": {
      assertExactKeys(body, ["resolvedProfileId", "resolvedVariantId"]);
      return {
        kind: "started",
        resolvedProfileId: asModelIdentifier(body.resolvedProfileId),
        resolvedVariantId: asModelIdentifier(body.resolvedVariantId),
      };
    }
    case "output_progress": {
      assertExactKeys(body, ["outputIndex", "textDelta", "deltaByteCount"]);
      const deltaByteCount = asUnsignedInteger(body.deltaByteCount);
      if (deltaByteCount === 0) return fail();
      return {
        kind: "output_progress",
        outputIndex: asUnsignedInteger(body.outputIndex),
        textDelta: asStreamText(body.textDelta, MAX_TEXT_DELTA_BYTES),
        deltaByteCount,
      };
    }
    case "tool_call_started": {
      assertExactKeys(body, ["toolCallId", "toolName"]);
      return {
        kind: "tool_call_started",
        toolCallId: asModelIdentifier(body.toolCallId),
        toolName: asModelDisplayText(body.toolName, MAX_TOOL_NAME_BYTES),
      };
    }
    case "tool_call_arguments": {
      assertExactKeys(body, ["toolCallId", "argumentsDelta", "deltaByteCount"]);
      const deltaByteCount = asUnsignedInteger(body.deltaByteCount);
      if (deltaByteCount === 0) return fail();
      return {
        kind: "tool_call_arguments",
        toolCallId: asModelIdentifier(body.toolCallId),
        argumentsDelta: asStreamText(body.argumentsDelta, MAX_ARGUMENTS_DELTA_BYTES),
        deltaByteCount,
      };
    }
    case "tool_call_completed": {
      assertExactKeys(body, ["toolCallId", "argumentsByteCount"]);
      return {
        kind: "tool_call_completed",
        toolCallId: asModelIdentifier(body.toolCallId),
        argumentsByteCount: asUnsignedInteger(body.argumentsByteCount),
      };
    }
    case "terminal": {
      assertExactKeys(body, [
        "terminal",
        "receipt",
        "cancellationSource",
        "reasonCode",
        "retryable",
      ]);
      const terminal = asBmadNonemptySafeText(body.terminal, 16);
      if (!modelStreamTerminals.has(terminal)) return fail();
      const retryable = asBoolean(body.retryable);
      const reasonCode = body.reasonCode === null
        ? null
        : asModelDisplayText(body.reasonCode, MAX_REASON_BYTES);
      const cancellationSource = body.cancellationSource === null
        ? null
        : asBmadNonemptySafeText(body.cancellationSource, 16);
      if (cancellationSource !== null && !modelStreamCancellationSources.has(cancellationSource)) {
        return fail();
      }
      const receipt = body.receipt === null ? null : parseModelReceiptSummary(body.receipt);
      if (terminal === "completed") {
        if (receipt === null || cancellationSource !== null || reasonCode !== null || retryable) {
          return fail();
        }
        return { kind: "terminal", terminal, receipt, cancellationSource: null, reasonCode: null, retryable: false };
      }
      if (terminal === "refused") {
        if (receipt !== null || cancellationSource !== null || retryable || reasonCode === null) {
          return fail();
        }
        return { kind: "terminal", terminal, receipt: null, cancellationSource: null, reasonCode, retryable: false };
      }
      if (terminal === "failed") {
        if (receipt !== null || cancellationSource !== null) return fail();
        return { kind: "terminal", terminal, receipt: null, cancellationSource: null, reasonCode, retryable };
      }
      if (terminal !== "cancelled") return fail();
      if (receipt !== null || cancellationSource === null || reasonCode !== null || retryable) {
        return fail();
      }
      return {
        kind: "terminal",
        terminal,
        receipt: null,
        cancellationSource: cancellationSource as ModelStreamCancellationSource,
        reasonCode: null,
        retryable: false,
      };
    }
    default:
      return fail();
  }
}

export function parseModelStreamEvent(value: unknown): ModelStreamEventProjection {
  const record = asRecord(value);
  assertExactKeys(record, [
    "schemaVersion",
    "workspaceId",
    "workspaceGrantEpoch",
    "appEpoch",
    "taskId",
    "runId",
    "sequence",
    "occurredAt",
    "kind",
    "value",
  ]);
  if (record.schemaVersion !== MODEL_STREAM_EVENT_SCHEMA) return fail();
  const workspaceGrantEpoch = asUnsignedInteger(record.workspaceGrantEpoch);
  const appEpoch = asUnsignedInteger(record.appEpoch);
  const sequence = asUnsignedInteger(record.sequence);
  if (workspaceGrantEpoch === 0 || appEpoch === 0 || sequence === 0) return fail();
  const body = parseModelStreamEventBody({ kind: record.kind, value: record.value });
  return {
    schemaVersion: MODEL_STREAM_EVENT_SCHEMA,
    workspaceId: asContractId(record.workspaceId),
    workspaceGrantEpoch,
    appEpoch,
    taskId: asContractId(record.taskId),
    runId: asContractId(record.runId),
    sequence,
    occurredAt: asUnsignedInteger(record.occurredAt),
    body,
  };
}

/// The acknowledgement that one task-bound response began. Mirrors
/// `ModelResponseStartedWire`. It names the run and the resolved profile so
/// the renderer can bind a stream subscription and show which model is
/// answering -- nothing here names an endpoint, a deployment, or a
/// credential.
export interface ModelResponseStartedProjection {
  readonly taskId: string;
  readonly runId: string;
  readonly appEpoch: number;
  readonly profileId: string;
  readonly variantId: string;
  readonly catalogEpoch: number;
  readonly startedAt: number;
}

function parseModelResponseStarted(value: unknown): ModelResponseStartedProjection {
  const started = asRecord(value);
  assertExactKeys(started, [
    "taskId",
    "runId",
    "appEpoch",
    "profileId",
    "variantId",
    "catalogEpoch",
    "startedAt",
  ]);
  return {
    taskId: asContractId(started.taskId),
    runId: asContractId(started.runId),
    appEpoch: asUnsignedInteger(started.appEpoch),
    profileId: asModelIdentifier(started.profileId),
    variantId: asModelIdentifier(started.variantId),
    catalogEpoch: asUnsignedInteger(started.catalogEpoch),
    startedAt: asUnsignedInteger(started.startedAt),
  };
}

/// The acknowledgement that one task-bound response was stopped. `stopped` is
/// `false` when the run had already reached a terminal state, so a second
/// stop reads as "already finished" rather than a success the user caused.
export interface ModelResponseStoppedProjection {
  readonly taskId: string;
  readonly runId: string;
  readonly appEpoch: number;
  readonly stopped: boolean;
  readonly stoppedAt: number;
}

function parseModelResponseStopped(value: unknown): ModelResponseStoppedProjection {
  const stopped = asRecord(value);
  assertExactKeys(stopped, ["taskId", "runId", "appEpoch", "stopped", "stoppedAt"]);
  return {
    taskId: asContractId(stopped.taskId),
    runId: asContractId(stopped.runId),
    appEpoch: asUnsignedInteger(stopped.appEpoch),
    stopped: asBoolean(stopped.stopped),
    stoppedAt: asUnsignedInteger(stopped.stoppedAt),
  };
}

/// The renderer's view of `HostCommandData::ModelCatalog` /
/// `::ModelCatalogUnavailable`. A distinct `unavailable` case rather than an
/// empty catalog: "we could not ask" and "the operator publishes no models"
/// are different facts.
export type ModelCatalogOutcome =
  | { readonly kind: "catalog"; readonly catalog: ModelCatalogProjection }
  | { readonly kind: "unavailable" };

export function parseModelCatalogReply(
  value: unknown,
  requestId: string,
): { outcome: ModelCatalogOutcome; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  if (parsed.data.kind === "model_catalog_unavailable") {
    assertExactKeys(parsed.data, ["kind"]);
    return { outcome: { kind: "unavailable" }, sequence: parsed.sequence };
  }
  if (parsed.data.kind !== "model_catalog") return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  return {
    outcome: { kind: "catalog", catalog: parseModelCatalog(parsed.data.value) },
    sequence: parsed.sequence,
  };
}

export function parseModelSelectionReply(
  value: unknown,
  requestId: string,
): { projection: ModelSelectionProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "model_selection") return fail();
  return { projection: parseModelSelection(parsed.data.value), sequence: parsed.sequence };
}

/// The renderer's view of `send_model_response`'s three shapes: either the
/// response started, or the workspace's own selection could not be resolved
/// (the same "cannot answer yet" outcomes `model.selection.get` reports),
/// or there is no catalog at all. `RequiresExplicitChoice` reaching here
/// means the same thing it means everywhere else in this module: nothing
/// picked a replacement, the caller must ask the user.
export type ModelResponseSendOutcome =
  | { readonly kind: "started"; readonly started: ModelResponseStartedProjection }
  | { readonly kind: "selection"; readonly selection: ModelSelectionProjection }
  | { readonly kind: "unavailable" };

export function parseModelResponseSendReply(
  value: unknown,
  requestId: string,
): { outcome: ModelResponseSendOutcome; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  if (parsed.data.kind === "model_catalog_unavailable") {
    assertExactKeys(parsed.data, ["kind"]);
    return { outcome: { kind: "unavailable" }, sequence: parsed.sequence };
  }
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind === "model_response_started") {
    return {
      outcome: { kind: "started", started: parseModelResponseStarted(parsed.data.value) },
      sequence: parsed.sequence,
    };
  }
  if (parsed.data.kind === "model_selection") {
    return {
      outcome: { kind: "selection", selection: parseModelSelection(parsed.data.value) },
      sequence: parsed.sequence,
    };
  }
  return fail();
}

export function parseModelResponseStopReply(
  value: unknown,
  requestId: string,
): { projection: ModelResponseStoppedProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "model_response_stopped") return fail();
  return { projection: parseModelResponseStopped(parsed.data.value), sequence: parsed.sequence };
}
