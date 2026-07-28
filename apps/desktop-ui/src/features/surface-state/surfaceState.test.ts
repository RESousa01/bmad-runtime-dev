import { describe, expect, it } from "vitest";

import type { LocalHostError } from "../../lib/hostClient";
import { isRecoveryFailure } from "../../app/hostFailure";
import { HostCommandError } from "../../lib/hostClient";
import {
  genericFallbackCodes,
  hostErrorPresentation,
  presentHostError,
  type LocalErrorCode,
} from "./surfaceState";

function hostError(
  code: LocalErrorCode,
  retryable = true,
): LocalHostError {
  return { code, correlationId: null, retryable, safeMessage: "The host said no." };
}

const everyCode = Object.keys(hostErrorPresentation) as LocalErrorCode[];

describe("the mapping is total", () => {
  /**
   * The plan's acceptance criterion, asserted at runtime as well as by the
   * type. The `Record` is what stops a new code compiling unmapped; this is
   * what stops one being mapped to nothing at all.
   */
  it("gives every host error code a state and a recovery", () => {
    expect(everyCode.length).toBeGreaterThan(25);
    for (const code of everyCode) {
      const presentation = hostErrorPresentation[code];
      expect(presentation.state).toBeTruthy();
      expect(presentation.recovery.kind).toBeTruthy();
    }
  });

  it("never says there is nothing to be done without saying why", () => {
    // The same rule the command registry's `unavailable` follows. Offering no
    // recovery and offering no explanation are different failures, and only
    // one of them is ever correct.
    for (const code of everyCode) {
      const { recovery } = hostErrorPresentation[code];
      if (recovery.kind !== "none") continue;
      expect(recovery.because.length).toBeGreaterThan(0);
      expect(recovery.because.endsWith(".")).toBe(true);
    }
  });

  it("lists exactly one generic fallback, and names it", () => {
    // "Falls to the generic fallback" has to be a decision someone made, not a
    // gap. A second unnoticed generic is how thirty codes become one sentence.
    expect(genericFallbackCodes()).toEqual(["internal"]);
  });
});

describe("agreement with the rest of the app", () => {
  /**
   * `app/hostFailure.ts` is the single predicate for "stop trusting the host".
   * This table has to draw the same line, because a code it presents as an
   * ordinary retryable error while `isRecoveryFailure` treats it as fatal would
   * leave the app offering a retry against a host that has told it to stop.
   */
  it("routes exactly the recovery failures to local-store repair", () => {
    for (const code of everyCode) {
      const fatal = isRecoveryFailure(new HostCommandError(hostError(code)));
      const repairs = hostErrorPresentation[code].recovery.kind === "recover_local_store";
      if (fatal) expect(repairs).toBe(true);
    }
    expect(hostErrorPresentation.recovery_required.recovery.kind).toBe("recover_local_store");
    expect(hostErrorPresentation.integrity_failure.recovery.kind).toBe("recover_local_store");
  });

  /// A BMAD gap is a constraint on one feature, not a failure of the app --
  /// audit finding F-05, and the reason chat availability is separate from
  /// guidance availability.
  it("treats a BMAD projection gap as a constraint, not an error", () => {
    expect(hostErrorPresentation.bmad_projection_gap.state).toBe("constraint");
  });

  it("treats a hit limit as a constraint, not an error", () => {
    // The app did what it was designed to do. Calling that a failure is how a
    // cap that silently dropped the user's files looked like a broken app.
    expect(hostErrorPresentation.resource_limit.state).toBe("constraint");
  });

  /// Consent already spent cannot be re-approved: that is consent given once
  /// and acted on twice. The recovery is a fresh review, never a retry.
  it("never offers a retry for a consumed consent", () => {
    expect(hostErrorPresentation.consent_already_consumed.recovery.kind).toBe("review_context");
  });

  it("never offers a retry for a rejected context", () => {
    // Sending the same selection again cannot work. The files are the problem.
    expect(hostErrorPresentation.context_rejected.recovery.kind).toBe("review_context");
  });
});

describe("presentHostError", () => {
  it("keeps a retry the host allows", () => {
    expect(presentHostError(hostError("transport_failed", true)).recovery.kind).toBe("retry");
  });

  /**
   * The host is the only thing that knows whether *this* occurrence could go
   * differently. The table says what kind of failure a code is; the flag
   * decides this one. A retry the host has already ruled out is a control that
   * cannot succeed, which this codebase has shipped three separate times.
   */
  it("withdraws a retry the host ruled out", () => {
    const presented = presentHostError(hostError("transport_failed", false));
    expect(presented.recovery.kind).toBe("none");
    if (presented.recovery.kind !== "none") return;
    expect(presented.recovery.because).toContain("final");
  });

  it("keeps the state when it withdraws the retry", () => {
    // The failure is no less an error for being final.
    expect(presentHostError(hostError("transport_failed", false)).state).toBe("error");
  });

  it("leaves a non-retry recovery alone whatever the flag says", () => {
    // `retryable: false` on a code whose recovery was never a retry says
    // nothing about that recovery, and must not remove it.
    for (const retryable of [true, false]) {
      expect(presentHostError(hostError("authentication_required", retryable)).recovery.kind)
        .toBe("reauthenticate");
      expect(presentHostError(hostError("recovery_required", retryable)).recovery.kind)
        .toBe("recover_local_store");
    }
  });

  it("handles every code without throwing", () => {
    for (const code of everyCode) {
      for (const retryable of [true, false]) {
        expect(presentHostError(hostError(code, retryable)).state).toBeTruthy();
      }
    }
  });
});
