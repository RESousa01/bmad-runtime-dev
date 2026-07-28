/**
 * Classifying a host failure.
 *
 * `recovery_required` and `integrity_failure` are the host saying its own state
 * is not trustworthy. Everything else is an ordinary failure with a message for
 * the user.
 *
 * The distinction has to be made identically everywhere, because getting it
 * wrong is silent in the worst direction: a host demanding recovery reported as
 * a generic error leaves the app running, offering actions that cannot work,
 * against a host that has told it to stop. It was written out in three places
 * before this module existed.
 */

import { HostCommandError } from "../lib/hostClient";

/** The two codes that mean "stop trusting me", not "that did not work". */
export function isRecoveryFailure(error: unknown): boolean {
  return error instanceof HostCommandError
    && (error.details.code === "recovery_required"
      || error.details.code === "integrity_failure");
}
