import type {
  AboutProjection,
  OffboardingErasedProjection,
  PreferencesProjection,
  RetentionManifestProjection,
} from "./contracts";
import {
  asBoolean,
  asBoundedString,
  asContractId,
  assertExactKeys,
  asRecord,
  asUnsignedInteger,
  fail,
} from "./validation";
import { parseDispatchReply } from "./workspaceProtocol";

const PREFERENCES_SCHEMA = "desktop-preferences.v1" as const;

function asTheme(value: unknown): PreferencesProjection["theme"] {
  if (value === "light" || value === "dark" || value === "system") return value;
  return fail();
}

function asDensity(value: unknown): PreferencesProjection["density"] {
  if (value === "comfortable" || value === "compact") return value;
  return fail();
}

function asBootModeLabel(value: unknown): AboutProjection["bootMode"] {
  if (value === "ready" || value === "read_only_recovery") return value;
  return fail();
}

export function parsePreferences(value: unknown): PreferencesProjection {
  const preferences = asRecord(value);
  assertExactKeys(preferences, ["schemaVersion", "theme", "density", "updatedAt"]);
  if (preferences.schemaVersion !== PREFERENCES_SCHEMA) return fail();
  return {
    schemaVersion: PREFERENCES_SCHEMA,
    theme: asTheme(preferences.theme),
    density: asDensity(preferences.density),
    updatedAt:
      preferences.updatedAt === null
        ? null
        : asUnsignedInteger(preferences.updatedAt),
  };
}

export function parsePreferencesReply(
  value: unknown,
  requestId: string,
): { projection: PreferencesProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "preferences") return fail();
  return {
    projection: parsePreferences(parsed.data.value),
    sequence: parsed.sequence,
  };
}

const RETENTION_MANIFEST_SCHEMA = "sapphirus.retention-manifest.v1" as const;
const OFFBOARDING_ERASED_SCHEMA = "sapphirus.offboarding-erased.v1" as const;
const RETENTION_CATEGORY_LIMIT = 32;

function asRetentionCategoryLabel(value: unknown): string {
  const label = asBoundedString(value, 64);
  // Bounded category labels only — a path or identifier is a contract breach.
  if (!/^[a-z][a-z_]*$/.test(label)) return fail();
  return label;
}

export function parseRetentionManifest(
  value: unknown,
): RetentionManifestProjection {
  const manifest = asRecord(value);
  assertExactKeys(manifest, ["schemaVersion", "categories", "retainedBytes"]);
  if (manifest.schemaVersion !== RETENTION_MANIFEST_SCHEMA) return fail();
  if (!Array.isArray(manifest.categories)) return fail();
  if (manifest.categories.length > RETENTION_CATEGORY_LIMIT) return fail();
  return {
    schemaVersion: RETENTION_MANIFEST_SCHEMA,
    categories: manifest.categories.map((entry) => {
      const category = asRecord(entry);
      assertExactKeys(category, ["category", "count"]);
      return {
        category: asRetentionCategoryLabel(category.category),
        count: asUnsignedInteger(category.count),
      };
    }),
    retainedBytes: asUnsignedInteger(manifest.retainedBytes),
  };
}

export function parseRetentionManifestReply(
  value: unknown,
  requestId: string,
): { projection: RetentionManifestProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "retention_manifest") return fail();
  return {
    projection: parseRetentionManifest(parsed.data.value),
    sequence: parsed.sequence,
  };
}

export function parseOffboardingErased(
  value: unknown,
): OffboardingErasedProjection {
  const outcome = asRecord(value);
  assertExactKeys(outcome, ["schemaVersion", "status", "restartRequired"]);
  if (outcome.schemaVersion !== OFFBOARDING_ERASED_SCHEMA) return fail();
  if (outcome.status !== "erased") return fail();
  return {
    schemaVersion: OFFBOARDING_ERASED_SCHEMA,
    status: "erased",
    restartRequired: asBoolean(outcome.restartRequired),
  };
}

export function parseOffboardingErasedReply(
  value: unknown,
  requestId: string,
): { projection: OffboardingErasedProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "offboarding_erased") return fail();
  return {
    projection: parseOffboardingErased(parsed.data.value),
    sequence: parsed.sequence,
  };
}

export function parseAbout(value: unknown): AboutProjection {
  const about = asRecord(value);
  assertExactKeys(about, [
    "appVersion",
    "installationId",
    "bootMode",
    "foundationPackageName",
    "foundationPackageVersion",
    "inactiveBuilderPackageCount",
    "updateConfigured",
    "updateInstallAvailable",
  ]);
  return {
    appVersion: asBoundedString(about.appVersion, 64),
    installationId: asContractId(about.installationId),
    bootMode: asBootModeLabel(about.bootMode),
    foundationPackageName: asBoundedString(about.foundationPackageName, 128),
    foundationPackageVersion: asBoundedString(about.foundationPackageVersion, 64),
    inactiveBuilderPackageCount: asUnsignedInteger(about.inactiveBuilderPackageCount),
    updateConfigured: asBoolean(about.updateConfigured),
    updateInstallAvailable: asBoolean(about.updateInstallAvailable),
  };
}

export function parseAboutReply(
  value: unknown,
  requestId: string,
): { projection: AboutProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "about") return fail();
  return {
    projection: parseAbout(parsed.data.value),
    sequence: parsed.sequence,
  };
}
