/**
 * What the model was actually given, and when that measurement stops being true.
 *
 * FE-13 requires this to be separate from attachments, and the reason is that
 * they answer different questions. Attachments are what the *user chose to
 * send*. Model context is what the *model received* -- which also includes the
 * conversation so far, the bound agent's persona, and whatever the host adds --
 * measured after the host has assembled the request. Conflating them makes the
 * Context panel a list of the user's file picks and calls it usage, which is
 * wrong in both directions: it omits the transcript and it counts files the
 * host may have trimmed.
 *
 * Invalidation is the whole design
 * --------------------------------
 * A snapshot is a measurement of one request under one authority. Change the
 * task, the model, the route, or the prompt projection and the number on screen
 * describes a request that will never be made again. Showing a stale figure is
 * worse than showing none: the user budgets against it.
 *
 * So a snapshot carries the key it was taken under, and `isSnapshotCurrent`
 * compares -- rather than the shell trying to remember which of four changes
 * should have cleared it.
 */

/**
 * The authority a measurement was taken under.
 *
 * Every field is something that changes what the model receives. `promptEpoch`
 * stands for the prompt projection: the host bumps it when the assembled
 * request shape changes, so a persona edit invalidates without the renderer
 * needing to understand personas.
 */
export interface ModelContextKey {
  readonly taskId: string;
  readonly workspaceId: string;
  readonly workspaceGrantEpoch: number;
  /** Which provider and model. A route change re-prices everything. */
  readonly modelRoute: string;
  readonly promptEpoch: number;
}

/** A measurement of one assembled request. */
export interface ModelContextSnapshot {
  readonly key: ModelContextKey;
  /** Tokens the host reported for the assembled request. */
  readonly inputTokens: number;
  /** How much of the model's window that is, or null when the window is unknown. */
  readonly windowTokens: number | null;
  /** When it was measured, for "as of" rather than an implied "now". */
  readonly measuredAt: number;
}

export type ModelContextState =
  /** Never measured under the current key. */
  | { readonly kind: "unmeasured" }
  | { readonly kind: "measured"; readonly snapshot: ModelContextSnapshot }
  /**
   * Measured, then something changed that the measurement did not cover.
   *
   * The old snapshot is kept so the panel can say "as of the last send" rather
   * than going blank -- a number with a stated age is more use than nothing,
   * and blanking on every keystroke would make the panel flicker.
   */
  | { readonly kind: "stale"; readonly snapshot: ModelContextSnapshot };

export const unmeasuredModelContext: ModelContextState = { kind: "unmeasured" };

export function keysMatch(a: ModelContextKey, b: ModelContextKey): boolean {
  return a.taskId === b.taskId
    && a.workspaceId === b.workspaceId
    && a.workspaceGrantEpoch === b.workspaceGrantEpoch
    && a.modelRoute === b.modelRoute
    && a.promptEpoch === b.promptEpoch;
}

/** Whether a state still describes the request the given key would produce. */
export function isSnapshotCurrent(
  state: ModelContextState,
  key: ModelContextKey,
): boolean {
  return state.kind === "measured" && keysMatch(state.snapshot.key, key);
}

/** Records a measurement. */
export function measure(snapshot: ModelContextSnapshot): ModelContextState {
  return { kind: "measured", snapshot };
}

/**
 * Re-evaluates a state against the current key.
 *
 * Called wherever the key is derived, so nothing has to remember *which* change
 * invalidates: a measurement taken under a different key is stale by
 * definition. An already-stale state stays stale even if the key happens to
 * return to its old value -- the request in between may have changed the
 * transcript, and the old number no longer describes it.
 */
export function reconcile(
  state: ModelContextState,
  key: ModelContextKey,
): ModelContextState {
  if (state.kind === "unmeasured") return state;
  if (state.kind === "stale") return state;
  return keysMatch(state.snapshot.key, key) ? state : { kind: "stale", snapshot: state.snapshot };
}

/** The fraction of the model's window used, or null when the window is unknown. */
export function windowFraction(snapshot: ModelContextSnapshot): number | null {
  if (snapshot.windowTokens === null || snapshot.windowTokens <= 0) return null;
  return snapshot.inputTokens / snapshot.windowTokens;
}
