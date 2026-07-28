import { describe, expect, it } from "vitest";

import { HostCommandError, type LocalHostError } from "./contracts";
import { getSafeHostMessage } from "./runtime";
import { parseLocalHostError } from "./workspaceProtocol";

/**
 * FE-11 acceptance: host errors remain lossless and inspectable.
 *
 * The renderer decides real things from an error's `code` -- whether to enter
 * read-only recovery, whether to re-bootstrap an expired renderer session,
 * whether a failure is worth retrying. A parser that widened a code it did not
 * recognise, or quietly dropped a field, would turn one of those decisions into
 * the wrong one silently. `correlationId` is the other half: without it a
 * report from a user cannot be matched to anything in the host's log.
 */

/** Every code the contract declares, taken from the parser's own union. */
const ALL_CODES: readonly LocalHostError["code"][] = [
  "invalid_request", "unauthorized", "conflict", "not_found", "resource_limit",
  "expired", "integrity_failure", "recovery_required", "temporarily_unavailable",
  "bmad_projection_unavailable", "bmad_projection_gap", "renderer_session_expired",
  "identity_unavailable", "authentication_required", "reauthentication_required",
  "tenant_mismatch", "entitlement_unavailable", "feature_disabled",
  "context_rejected", "context_drift", "consent_required", "consent_expired",
  "consent_binding_mismatch", "consent_already_consumed", "support_plane_offline",
  "transport_failed", "response_binding_mismatch", "invalid_model_output",
  "receipt_invalid", "internal",
];

function wireError(overrides: Partial<LocalHostError> = {}): unknown {
  return {
    code: "conflict",
    safeMessage: "That changed underneath you.",
    retryable: false,
    correlationId: "corr-1",
    ...overrides,
  };
}

describe("parsing preserves the whole error", () => {
  it("round-trips every declared code", () => {
    for (const code of ALL_CODES) {
      expect(parseLocalHostError(wireError({ code })).code).toBe(code);
    }
  });

  it("preserves the correlation id", () => {
    // Without it a user's report cannot be matched to anything in the host log.
    expect(parseLocalHostError(wireError({ correlationId: "abc-123" })).correlationId)
      .toBe("abc-123");
  });

  it("preserves an absent correlation id as null rather than inventing one", () => {
    expect(parseLocalHostError(wireError({ correlationId: null })).correlationId).toBeNull();
  });

  it("preserves the retryable flag in both directions", () => {
    expect(parseLocalHostError(wireError({ retryable: true })).retryable).toBe(true);
    expect(parseLocalHostError(wireError({ retryable: false })).retryable).toBe(false);
  });

  it("preserves the safe message verbatim", () => {
    const message = "The workspace grant was revoked while that was running.";
    expect(parseLocalHostError(wireError({ safeMessage: message })).safeMessage)
      .toBe(message);
  });
});

describe("parsing refuses rather than coerces", () => {
  it("rejects a code the contract does not declare", () => {
    // Widening an unrecognised code is how a host that asked for recovery gets
    // handled as an ordinary failure.
    expect(() => parseLocalHostError(wireError({ code: "made_up" as never }))).toThrow();
  });

  it("rejects an error carrying a field the contract does not declare", () => {
    // `assertExactKeys`, not a subset check: a field the renderer silently
    // ignores is a field the host thinks it delivered.
    expect(() => parseLocalHostError({ ...wireError() as object, extra: 1 })).toThrow();
  });

  it("rejects an error missing a required field", () => {
    for (const field of ["code", "safeMessage", "retryable", "correlationId"]) {
      const partial = { ...wireError() as Record<string, unknown> };
      delete partial[field];
      expect(() => parseLocalHostError(partial), field).toThrow();
    }
  });

  it("rejects a non-object", () => {
    for (const value of [null, undefined, "conflict", 42, []]) {
      expect(() => parseLocalHostError(value)).toThrow();
    }
  });
});

describe("the error stays inspectable after it is thrown", () => {
  it("carries the full details on the exception", () => {
    const details = parseLocalHostError(wireError({ code: "recovery_required" }));
    const error = new HostCommandError(details);
    expect(error.details).toEqual(details);
    expect(error.details.code).toBe("recovery_required");
  });

  it("uses the safe message as the exception message", () => {
    const error = new HostCommandError(parseLocalHostError(wireError()));
    expect(error.message).toBe("That changed underneath you.");
  });

  it("keeps the details reachable after getSafeHostMessage reads it", () => {
    // `getSafeHostMessage` is a display helper, not a discard. Callers that
    // show the message still branch on the code afterwards -- the recovery
    // classification does exactly that.
    const error = new HostCommandError(parseLocalHostError(wireError({
      code: "integrity_failure",
      correlationId: "corr-9",
    })));
    expect(getSafeHostMessage(error)).toBe("That changed underneath you.");
    expect(error.details.code).toBe("integrity_failure");
    expect(error.details.correlationId).toBe("corr-9");
  });

  it("does not leak an unknown error's shape into the user's message", () => {
    // The fallback is deliberately generic: a raw provider error can carry an
    // endpoint and part of the request.
    const message = getSafeHostMessage(new Error("POST https://internal/api key=sk-123"));
    expect(message).not.toContain("sk-123");
    expect(message).not.toContain("internal");
  });
});
