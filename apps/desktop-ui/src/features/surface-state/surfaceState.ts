/**
 * The five states every surface owes the user, and what each host failure means.
 *
 * FE-27, task D1. The audit's finding is not that surfaces lack error handling;
 * it is that each one invented its own, so the same host failure read as a
 * different problem depending on which panel the user happened to be looking
 * at. `recovery_required` was written out in three places before
 * `app/hostFailure.ts` existed, and each spelling drew the line somewhere
 * slightly different.
 *
 * Two things live here. The five states, so a surface cannot quietly ship four
 * of them; and the map from every host error code to the state it produces and
 * the recovery it offers, so that answer is given once.
 *
 * Why a `Record` and not a switch
 * -------------------------------
 * The plan's acceptance criterion is that every `LocalErrorCode` variant is
 * either mapped or explicitly listed as falling to the generic fallback.
 * `Record<LocalErrorCode, ...>` is that criterion, enforced by the compiler
 * rather than by whoever reviews the next change: a code added to the contract
 * fails the build here until someone decides what it means to a user. A switch
 * with a `default` would have silently absorbed it, which is exactly how thirty
 * error codes come to be presented as "Something went wrong".
 */

import type { LocalHostError } from "../../lib/hostClient";

/** Every code the host can return. Derived, so it cannot drift from the contract. */
export type LocalErrorCode = LocalHostError["code"];

/**
 * The five states, in the audit's own vocabulary.
 *
 * `constraint` is the one that gets dropped, and it is the one that matters
 * most for this app: a limit the user has hit -- a truncated pick, a context
 * cap, a retention ceiling -- is not an error and not an empty state. Reporting
 * it as either is how a cap that silently dropped the user's files came to look
 * like a successful attach.
 */
export type SurfaceStateKind =
  | "happy"
  | "empty"
  | "loading"
  | "error"
  | "constraint";

/**
 * What the user can do about it.
 *
 * `none` carries a required `because`, following the same rule as the command
 * registry's `unavailable`: a surface may say there is nothing to be done, but
 * it may not say so without saying why. Offering no recovery and offering no
 * explanation are different failures, and only one of them is sometimes
 * correct.
 */
export type SurfaceRecovery =
  /** The same action, again. Only ever offered when the host says retryable. */
  | { readonly kind: "retry" }
  /** Sign in again, or supply a key. */
  | { readonly kind: "reauthenticate" }
  /** Choose the workspace folder again; the grant is gone or has moved. */
  | { readonly kind: "regrant_workspace" }
  /** Look at the context selection again before re-sending. */
  | { readonly kind: "review_context" }
  /** The local authority store needs repair before anything else works. */
  | { readonly kind: "recover_local_store" }
  /** Something in Settings has to change first. */
  | { readonly kind: "open_settings" }
  | { readonly kind: "none"; readonly because: string };

export interface HostErrorPresentation {
  readonly state: SurfaceStateKind;
  readonly recovery: SurfaceRecovery;
  /**
   * True when this code has no presentation of its own and is deliberately
   * shown through the generic fallback.
   *
   * Marked rather than omitted, because the plan asks for these to be listed
   * explicitly. A code that falls through unmarked is indistinguishable from
   * one nobody considered.
   */
  readonly generic?: boolean;
}

/**
 * Every host error code, and what it means to someone looking at a screen.
 *
 * The states are assigned on one rule: `error` means the action failed and the
 * app is otherwise fine; `constraint` means the app is working exactly as
 * designed and the user has met a limit; `empty` means there is nothing to
 * show and nothing went wrong. A failure that leaves the app untrustworthy --
 * `integrity_failure`, `recovery_required` -- is an error whose recovery is not
 * "try again" but "repair the store", which is why those two are the pair
 * `hostFailure.ts` singles out.
 */
export const hostErrorPresentation: Record<LocalErrorCode, HostErrorPresentation> = {
  // The renderer sent something the host would not accept. The user did not do
  // this and cannot fix it, so the honest recovery is none, with a reason.
  invalid_request: {
    recovery: { because: "This is a fault in Forge, not something you can change.", kind: "none" },
    state: "error",
  },
  unauthorized: { recovery: { kind: "reauthenticate" }, state: "error" },
  // Two callers raced. Re-reading is the fix, and it is the user's to make.
  conflict: { recovery: { kind: "retry" }, state: "error" },
  not_found: {
    recovery: { because: "It is no longer where Forge last saw it.", kind: "none" },
    state: "empty",
  },
  // A cap, not a fault. The app did what it was designed to do.
  resource_limit: { recovery: { kind: "review_context" }, state: "constraint" },
  expired: { recovery: { kind: "retry" }, state: "error" },
  // The two that mean "stop trusting me". See `app/hostFailure.ts`, which is
  // the single predicate for this pair -- these entries must agree with it.
  integrity_failure: { recovery: { kind: "recover_local_store" }, state: "error" },
  recovery_required: { recovery: { kind: "recover_local_store" }, state: "error" },
  temporarily_unavailable: { recovery: { kind: "retry" }, state: "error" },
  // BMAD being incomplete is not chat being broken -- audit finding F-05, and
  // the reason `capabilityMatrix` separates the two. A gap is a constraint on
  // one feature, not a failure of the app.
  bmad_projection_unavailable: { recovery: { kind: "retry" }, state: "error" },
  bmad_projection_gap: {
    recovery: {
      because: "This skill is not projected into the local runtime.",
      kind: "none",
    },
    state: "constraint",
  },
  renderer_session_expired: { recovery: { kind: "retry" }, state: "error" },
  identity_unavailable: { recovery: { kind: "reauthenticate" }, state: "error" },
  authentication_required: { recovery: { kind: "reauthenticate" }, state: "error" },
  reauthentication_required: { recovery: { kind: "reauthenticate" }, state: "error" },
  tenant_mismatch: { recovery: { kind: "open_settings" }, state: "error" },
  entitlement_unavailable: { recovery: { kind: "open_settings" }, state: "constraint" },
  feature_disabled: {
    recovery: { because: "This build does not include that feature.", kind: "none" },
    state: "constraint",
  },
  // The host refused the files, not the message. Sending the same selection
  // again cannot work, so the recovery is to change it.
  context_rejected: { recovery: { kind: "review_context" }, state: "error" },
  context_drift: { recovery: { kind: "review_context" }, state: "constraint" },
  consent_required: { recovery: { kind: "review_context" }, state: "constraint" },
  consent_expired: { recovery: { kind: "review_context" }, state: "constraint" },
  consent_binding_mismatch: { recovery: { kind: "review_context" }, state: "error" },
  // Already spent. Re-approving the same decision would be consent given once
  // and acted on twice, so the recovery is a fresh review, never a retry.
  consent_already_consumed: { recovery: { kind: "review_context" }, state: "error" },
  support_plane_offline: { recovery: { kind: "retry" }, state: "error" },
  transport_failed: { recovery: { kind: "retry" }, state: "error" },
  // The reply did not belong to the request. Retrying is safe and is the only
  // thing that can help, because nothing about the request was wrong.
  response_binding_mismatch: { recovery: { kind: "retry" }, state: "error" },
  invalid_model_output: { recovery: { kind: "retry" }, state: "error" },
  receipt_invalid: { recovery: { kind: "recover_local_store" }, state: "error" },
  // The genuine fallback, and the only one. Marked so the contract document can
  // list it as such rather than leaving a reader to infer it.
  internal: { generic: true, recovery: { kind: "retry" }, state: "error" },
};

/**
 * What to show for one host failure.
 *
 * The host's own `retryable` flag overrides the table. The table says what kind
 * of failure a code *is*; the flag says whether this particular occurrence
 * could go differently, and the host is the only thing that knows. Offering a
 * retry the host has already ruled out is a control that cannot succeed, which
 * this codebase has shipped before in three separate places.
 */
export function presentHostError(error: LocalHostError): HostErrorPresentation {
  const mapped = hostErrorPresentation[error.code];
  if (mapped.recovery.kind !== "retry" || error.retryable) return mapped;
  return {
    ...mapped,
    recovery: {
      because: "The host reported this as final, so sending it again would not help.",
      kind: "none",
    },
  };
}

/** Codes shown through the generic fallback rather than a presentation of their own. */
export function genericFallbackCodes(): readonly LocalErrorCode[] {
  return (Object.keys(hostErrorPresentation) as LocalErrorCode[])
    .filter((code) => hostErrorPresentation[code].generic === true);
}
