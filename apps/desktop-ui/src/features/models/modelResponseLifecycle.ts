/**
 * FND-053: the send/stop lifecycle around one task's model response.
 *
 * `send_model_response` on the host has exactly four outcomes today, and this
 * module keeps them four states rather than collapsing any pair:
 *
 * * `started` -- a run began; bind a `ModelStreamTimelineState` to it.
 * * `requires_explicit_choice` / `none_selectable` -- the workspace's own
 *   selection could not be resolved. Never retried automatically and never
 *   defaulted -- see `desktop_ipc::project_model_selection`'s own doc for why.
 * * `unavailable` -- there is no catalog at all.
 * * `feature_disabled` -- the one a user on *this* build will actually hit:
 *   `crates/desktop-app/src/commands/model.rs::send_model_response` always
 *   returns `LocalErrorCode::FeatureDisabled` right now, because the HTTPS
 *   transport is FND-045's job and runs after this one. It is a first-class
 *   state, not a caught exception rendered as a generic error -- the user
 *   gets a real message and the draft survives.
 *
 * Every transition preserves the draft implicitly: nothing in this module
 * ever clears or reads the composer's draft text, because none of these
 * outcomes is the one thing that is allowed to -- a *sent* message. A
 * `feature_disabled`, `unavailable`, or `requires_explicit_choice` result
 * means the send never happened, so there is nothing here to discard.
 */

import type {
  ModelResponseSendOutcome,
  ModelResponseStartedProjection,
  ModelSelectionProjection,
} from "../../lib/hostClient";
import type { ModelStreamTimelineState } from "./modelStreamTimeline";

export type ModelResponseLifecycleState =
  | { readonly kind: "idle" }
  | { readonly kind: "sending" }
  | { readonly kind: "active"; readonly started: ModelResponseStartedProjection; readonly timeline: ModelStreamTimelineState }
  | {
    readonly kind: "requires_explicit_choice";
    readonly selection: Extract<ModelSelectionProjection, { status: "requires_explicit_choice" }>;
  }
  | { readonly kind: "none_selectable" }
  | { readonly kind: "unavailable" }
  | {
    readonly kind: "feature_disabled";
    readonly message: string;
  }
  | {
    /** A retryable transport/provider failure distinct from `feature_disabled`
     * -- this one names an actual attempt that reached the provider and
     * failed, not a build where the lane is closed. */
    readonly kind: "provider_failed";
    readonly message: string;
    readonly retryable: boolean;
  };

export const initialModelResponseLifecycleState: ModelResponseLifecycleState = { kind: "idle" };

export type ModelResponseLifecycleEvent =
  | { readonly type: "send_started" }
  | { readonly type: "send_outcome"; readonly outcome: ModelResponseSendOutcome; readonly makeTimeline: (started: ModelResponseStartedProjection) => ModelStreamTimelineState }
  | { readonly type: "send_failed"; readonly message: string; readonly retryable: boolean }
  /** The host's own dispatch refusal for a build where the response lane is
   * not composed. Distinct from `send_failed`: nothing was attempted. */
  | { readonly type: "feature_disabled"; readonly message: string }
  | { readonly type: "stream_event_applied"; readonly timeline: ModelStreamTimelineState }
  | { readonly type: "stopped" }
  | { readonly type: "reset" };

export function transitionModelResponseLifecycle(
  state: ModelResponseLifecycleState,
  event: ModelResponseLifecycleEvent,
): ModelResponseLifecycleState {
  switch (event.type) {
    case "send_started":
      return state.kind === "idle"
        || state.kind === "requires_explicit_choice"
        || state.kind === "none_selectable"
        || state.kind === "unavailable"
        || state.kind === "feature_disabled"
        || state.kind === "provider_failed"
        ? { kind: "sending" }
        : state;
    case "send_outcome": {
      if (state.kind !== "sending") return state;
      const { outcome } = event;
      if (outcome.kind === "started") {
        return { kind: "active", started: outcome.started, timeline: event.makeTimeline(outcome.started) };
      }
      if (outcome.kind === "unavailable") {
        return { kind: "unavailable" };
      }
      // outcome.kind === "selection"
      if (outcome.selection.status === "requires_explicit_choice") {
        return { kind: "requires_explicit_choice", selection: outcome.selection };
      }
      if (outcome.selection.status === "none_selectable") {
        return { kind: "none_selectable" };
      }
      // A `selected` outcome from `send_model_response` without a `started`
      // acknowledgement is not representable -- the host only returns the
      // selection projection for the two "cannot answer yet" shapes. Treated
      // as unavailable rather than silently starting nothing.
      return { kind: "unavailable" };
    }
    case "send_failed":
      return state.kind === "sending"
        ? { kind: "provider_failed", message: event.message, retryable: event.retryable }
        : state;
    case "feature_disabled":
      return state.kind === "sending" ? { kind: "feature_disabled", message: event.message } : state;
    case "stream_event_applied":
      return state.kind === "active" ? { ...state, timeline: event.timeline } : state;
    case "stopped":
      return state.kind === "active" ? { kind: "idle" } : state;
    case "reset":
      return { kind: "idle" };
  }
}

/**
 * Whether a `retry` action should be offered.
 *
 * A `model_call_uncertain`-shaped failure -- the host's stream `retryable`
 * flag is `false` on a `failed` terminal -- must not be retried: the request
 * may already have reached the provider and a second send could duplicate a
 * side effect the first one already caused. `feature_disabled` is also never
 * retryable from here: retrying dispatches the identical command against the
 * identical unbuilt transport and would fail identically.
 */
export function modelResponseIsRetryable(state: ModelResponseLifecycleState): boolean {
  return state.kind === "provider_failed" && state.retryable;
}
