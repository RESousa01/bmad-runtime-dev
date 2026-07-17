import {
  type BmadAvailability,
  type BmadBlockerCode,
  type BmadEntrypointKind,
  type BmadHelpConfidence,
  type BmadMenuTargetKind,
} from "../bmadProjection";
import {
  bmadAvailabilities,
  bmadBlockerCodes,
  bmadEntrypointKinds,
  bmadHelpConfidences,
  bmadMenuTargetKinds,
  bmadProjectionLimits,
} from "./bmadProtocolConstants";
import {
  type BootMode,
  HostProtocolError,
  type WorkspacePermission,
} from "./contracts";

export function fail(): never {
  throw new HostProtocolError();
}

export function asRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return fail();
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    return fail();
  }
  return value as Record<string, unknown>;
}

export function assertExactKeys(
  value: Record<string, unknown>,
  required: readonly string[],
): void {
  const actual = Object.keys(value).sort();
  const expected = [...required].sort();
  if (
    actual.length !== expected.length ||
    actual.some((key, index) => key !== expected[index])
  ) {
    fail();
  }
}

export function asBoundedString(value: unknown, maximumLength = 512): string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximumLength ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    return fail();
  }
  return value;
}

export function asContractId(value: unknown): string {
  const identifier = asBoundedString(value, 128);
  if (!/^[A-Za-z0-9._-]{3,128}$/u.test(identifier)) {
    return fail();
  }
  return identifier;
}

export function asUnsignedInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    return fail();
  }
  return value as number;
}

export function asBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    return fail();
  }
  return value;
}

export function utf8Length(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

export function hasUnpairedSurrogate(value: string): boolean {
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

export function asBmadCursor(value: unknown): string | null {
  if (value === null) {
    return null;
  }
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    utf8Length(value) > bmadProjectionLimits.cursorBytes ||
    !/^[\x21-\x7e]+$/u.test(value)
  ) {
    return fail();
  }
  return value;
}

export function asBmadIdentifier(value: unknown): string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    utf8Length(value) > bmadProjectionLimits.identifierBytes ||
    !/^[A-Za-z0-9._-]+$/u.test(value)
  ) {
    return fail();
  }
  return value;
}

export function asModelRegion(value: unknown): string {
  if (
    typeof value !== "string" ||
    value.length < 3 ||
    value.length > 64 ||
    !/^[a-z][a-z0-9-]+$/u.test(value)
  ) {
    return fail();
  }
  return value;
}

export function asNullableBmadIdentifier(value: unknown): string | null {
  return value === null ? null : asBmadIdentifier(value);
}

export function asBmadSafeText(value: unknown, maximumBytes: number): string {
  if (
    typeof value !== "string" ||
    utf8Length(value) > maximumBytes ||
    hasUnpairedSurrogate(value) ||
    /[\p{Cc}\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/u.test(value)
  ) {
    return fail();
  }
  return value;
}

export function asBmadNonemptySafeText(
  value: unknown,
  maximumBytes: number,
): string {
  const text = asBmadSafeText(value, maximumBytes);
  if (text.trim().length === 0) {
    return fail();
  }
  return text;
}

export function asBmadHelpIntent(value: unknown): string {
  if (
    typeof value !== "string" ||
    value.trim().length === 0 ||
    utf8Length(value) > bmadProjectionLimits.helpIntentBytes ||
    hasUnpairedSurrogate(value) ||
    /[\p{Cc}\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/u.test(value)
  ) {
    return fail();
  }
  return value;
}

export function asBmadAvailability(value: unknown): BmadAvailability {
  const availability = asBmadIdentifier(value) as BmadAvailability;
  if (!bmadAvailabilities.has(availability)) {
    return fail();
  }
  return availability;
}

export function asBmadEntrypointKind(value: unknown): BmadEntrypointKind {
  const entrypointKind = asBmadIdentifier(value) as BmadEntrypointKind;
  if (!bmadEntrypointKinds.has(entrypointKind)) {
    return fail();
  }
  return entrypointKind;
}

export function asBmadMenuTargetKind(value: unknown): BmadMenuTargetKind {
  const targetKind = asBmadIdentifier(value) as BmadMenuTargetKind;
  if (!bmadMenuTargetKinds.has(targetKind)) {
    return fail();
  }
  return targetKind;
}

export function asBmadHelpConfidence(value: unknown): BmadHelpConfidence {
  const confidence = asBmadIdentifier(value) as BmadHelpConfidence;
  if (!bmadHelpConfidences.has(confidence)) {
    return fail();
  }
  return confidence;
}

export function asBmadBlockerCode(value: unknown): BmadBlockerCode {
  const blockerCode = asBmadIdentifier(value) as BmadBlockerCode;
  if (!bmadBlockerCodes.has(blockerCode)) {
    return fail();
  }
  return blockerCode;
}

export function parseBmadBlockerCodes(value: unknown): BmadBlockerCode[] {
  if (!Array.isArray(value) || value.length > bmadBlockerCodes.size) {
    return fail();
  }
  const blockerCodes = value.map(asBmadBlockerCode);
  if (new Set(blockerCodes).size !== blockerCodes.length) {
    return fail();
  }
  return blockerCodes;
}

export function asNullableBmadBlockerCode(
  value: unknown,
): BmadBlockerCode | null {
  return value === null ? null : asBmadBlockerCode(value);
}

export function assertUniqueIdentities(identities: readonly string[]): void {
  if (new Set(identities).size !== identities.length) {
    fail();
  }
}

export function asNullableOpaqueCursor(value: unknown): string | null {
  if (value === null) {
    return null;
  }
  const cursor = asBoundedString(value, 64);
  if (!/^cursor_[0-9A-HJKMNP-TV-Z]{26}$/u.test(cursor)) {
    return fail();
  }
  return cursor;
}

export function asSha256(value: unknown): string {
  const digest = asBoundedString(value, 71);
  if (!/^sha256:[0-9a-f]{64}$/u.test(digest)) {
    return fail();
  }
  return digest;
}

export function isWindowsReservedSegment(segment: string): boolean {
  const deviceName = segment
    .split(".", 1)[0]!
    .replace(/[. ]+$/u, "")
    .toLocaleUpperCase("en-US");
  return /^(?:CON|PRN|AUX|NUL|CLOCK\$|CONIN\$|CONOUT\$|(?:COM|LPT)[1-9¹²³])$/u.test(
    deviceName,
  );
}

export function asRelativePath(value: unknown): string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    utf8Length(value) > 1024
  ) {
    return fail();
  }
  if (
    value.startsWith("/") ||
    value.includes("\\") ||
    value.includes(":") ||
    /[<>"|?*]/u.test(value) ||
    /\p{C}/u.test(value) ||
    hasUnpairedSurrogate(value)
  ) {
    return fail();
  }
  const segments = value.split("/");
  if (
    segments.some(
      (segment) =>
        segment.length === 0 ||
        segment.length > 255 ||
        segment === "." ||
        segment === ".." ||
        segment.endsWith(".") ||
        segment.endsWith(" ") ||
        isWindowsReservedSegment(segment),
    )
  ) {
    return fail();
  }
  return value;
}

export function asTextContent(value: unknown, maximumBytes: number): string {
  if (
    typeof value !== "string" ||
    value.includes("\0") ||
    hasUnpairedSurrogate(value) ||
    utf8Length(value) > maximumBytes
  ) {
    return fail();
  }
  return value;
}

export function asSingleLineText(
  value: unknown,
  maximumLength: number,
): string {
  const text = asBoundedString(value, maximumLength);
  if (/\p{C}/u.test(text) || hasUnpairedSurrogate(text)) {
    return fail();
  }
  return text;
}

export function assertUniqueRelativePaths(paths: readonly string[]): void {
  const folded = new Set<string>();
  for (const path of paths) {
    const key = path.toLocaleLowerCase("en-US");
    if (folded.has(key)) {
      fail();
    }
    folded.add(key);
  }
}

export function isImmediateChild(
  relativePath: string,
  relativeDirectory: string,
): boolean {
  if (relativeDirectory === ".") {
    return !relativePath.includes("/");
  }
  const prefix = `${relativeDirectory}/`;
  if (!relativePath.startsWith(prefix)) {
    return false;
  }
  return !relativePath.slice(prefix.length).includes("/");
}

export function asNullableContractId(value: unknown): string | null {
  return value === null ? null : asContractId(value);
}

export function asBootMode(value: unknown): BootMode {
  if (value !== "ready" && value !== "read_only_recovery") {
    return fail();
  }
  return value;
}

export function asWorkspacePermission(value: unknown): WorkspacePermission {
  if (value !== "read_only" && value !== "governed_edits") {
    return fail();
  }
  return value;
}

export function asSafeDisplayName(value: unknown): string {
  const displayName = asBoundedString(value, 255);
  if (
    displayName !== displayName.trim() ||
    displayName.includes("/") ||
    displayName.includes("\\") ||
    displayName.includes(":") ||
    /\p{C}/u.test(displayName) ||
    displayName === "." ||
    displayName === ".."
  ) {
    return fail();
  }
  return displayName;
}

export function asRendererSafeMessage(value: unknown): string {
  const message = asBoundedString(value, 512);
  if (
    /\p{C}/u.test(message) ||
    /(?:\\|[A-Za-z]:\/|file:\/\/|(?:^|[^A-Za-z0-9])\/(?:[^\s]|$))/iu.test(
      message,
    )
  ) {
    return fail();
  }
  return message;
}
