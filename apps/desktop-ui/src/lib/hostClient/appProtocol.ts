import type { AboutProjection, PreferencesProjection } from "./contracts";
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
