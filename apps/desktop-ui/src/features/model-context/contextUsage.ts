/**
 * What the Context surface is allowed to claim, and on whose authority.
 *
 * FE-16. The panel read `used / 64K`, where the 64K was
 * `workspaceReadLimits.contextBytes / 4` -- the workspace broker's file-read
 * cap, divided by a bytes-per-token guess. Nothing in that number is a property
 * of the model: it does not move when the route does, it is the same for every
 * provider, and a user budgeting a conversation against it is budgeting against
 * the file reader.
 *
 * There is no model window to swap in
 * -----------------------------------
 * `ModelAuthStatusProjection` carries the provider, the model id and a
 * destination label, and no token count. `ContextPreviewProjection.modelTarget`
 * is typed as the literal `null` and `workspaceProtocol` rejects anything else.
 * The host's only `context_window` is a profile *hash* -- binding provenance,
 * not capacity. So the window is genuinely unknown until a measurement arrives
 * carrying one, which is why `ModelContextSnapshot.windowTokens` was already
 * `number | null` before this module existed.
 *
 * Saying "unknown" is the point. The failure being fixed is not that 64K was
 * the wrong figure; it is that the surface stated a figure it had no basis for,
 * and the user cannot tell a measured number from a confident one by looking.
 *
 * So this module decides only which of specification 7.5's three verdicts --
 * reported, estimated, unavailable -- applies to the usage figure and to the
 * window. It is separate from the component because it is the half that can be
 * wrong without looking wrong.
 */

import {
  windowFraction,
  type ModelContextSnapshot,
  type ModelContextState,
} from "./modelContextTypes";

/** Where a displayed figure came from, in specification 7.5's vocabulary. */
export type UsageBasis =
  /** The host measured the request it assembled. */
  | "reported"
  /**
   * Summed from `estimatedTokens`, which the host derives as
   * `floor((byteCount + 3) / 4)`. No tokenizer ran, and the ratio is a
   * convention rather than a measurement, so it is wrong by an amount nobody
   * can bound from here.
   */
  | "estimated";

/**
 * The model's window, when there is one.
 *
 * A union rather than `windowTokens: number | null` beside `fraction: number |
 * null`, because those two are not independent: a fraction without a window is
 * a division by nothing, and a window without a fraction is a denominator the
 * meter cannot use. Two nullable fields let a caller render one and not the
 * other; one union makes that unrepresentable.
 */
export type ContextWindowExtent =
  | { readonly kind: "unavailable" }
  | {
    readonly kind: "known";
    readonly windowTokens: number;
    /** Used over window. Unclamped -- over-budget is information. */
    readonly fraction: number;
  };

export interface ContextUsage {
  readonly usedTokens: number;
  readonly basis: UsageBasis;
  readonly window: ContextWindowExtent;
  /**
   * True when a reported figure describes a request that would no longer be
   * made -- the route, the task or the prompt projection moved under it.
   *
   * Always false for an estimate, which is recomputed from the attachment list
   * on every render and so has no request to fall out of date with.
   */
  readonly stale: boolean;
}

function extentOf(snapshot: ModelContextSnapshot): ContextWindowExtent {
  const fraction = windowFraction(snapshot);
  // Both operands tested because they answer different questions.
  // `windowFraction` owns "is this window usable" -- it refuses zero and
  // negatives as well as absence -- and re-deriving that test here would be a
  // second place that could disagree about what unknown means. Naming
  // `windowTokens` again is only what lets the extent carry the number.
  return snapshot.windowTokens === null || fraction === null
    ? { kind: "unavailable" }
    : { fraction, kind: "known", windowTokens: snapshot.windowTokens };
}

/**
 * The verdict the surface renders.
 *
 * `attachedTokens` is the fallback, not a component of the answer: once the
 * host has measured an assembled request, that measurement already includes
 * the attachments along with the transcript and the persona, and adding the
 * local estimate on top would count the same files twice. A measurement
 * supersedes the estimate; it never supplements it.
 */
export function projectContextUsage(
  state: ModelContextState,
  attachedTokens: number,
): ContextUsage {
  if (state.kind === "unmeasured") {
    return {
      basis: "estimated",
      stale: false,
      usedTokens: attachedTokens,
      window: { kind: "unavailable" },
    };
  }
  const { snapshot } = state;
  return {
    basis: "reported",
    stale: state.kind === "stale",
    usedTokens: snapshot.inputTokens,
    window: extentOf(snapshot),
  };
}
