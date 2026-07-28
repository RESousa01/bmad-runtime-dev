/**
 * The Activity journal's read state.
 *
 * Two values that only make sense together: the projection, and whether it has
 * actually been read. `projection === null` used to mean both "never asked" and
 * "asked, nothing there", and the Activity panel rendered the pair identically
 * as "No activity yet" -- so someone checking whether a change could still be
 * rolled back was told it could not.
 *
 * `status` separates them, and every transition lives here so the two cannot
 * disagree. The one that kept being written by hand is `invalidate`: clearing
 * the projection without setting the status back to `unread` leaves it
 * `loaded`, the load-on-open effect never fires, and Activity reports "No
 * activity yet" immediately after a governed change is applied -- the exact
 * scenario that effect exists to fix. That pair was written out at four call
 * sites, each carrying a comment explaining why it had to stay together, and
 * nothing tested it.
 */

import { useCallback, useState } from "react";

import type { ChangesHistoryProjection } from "../../lib/hostClient";

export type ChangesHistoryStatus = "unread" | "loading" | "loaded" | "unavailable";

export interface ChangesHistoryState {
  readonly projection: ChangesHistoryProjection | null;
  readonly status: ChangesHistoryStatus;
  /** True while a user-initiated refresh is running, for the Refresh button. */
  readonly busy: boolean;
  readonly setBusy: (busy: boolean) => void;
  /** A read is starting. Doubles as the lock that stops a second one. */
  readonly beginRead: () => void;
  readonly completeRead: (projection: ChangesHistoryProjection) => void;
  readonly failRead: () => void;
  /**
   * The journal is stale and must be re-read.
   *
   * Both halves, always -- which is the whole reason this is a function.
   */
  readonly invalidate: () => void;
}

export function useChangesHistoryState(): ChangesHistoryState {
  const [projection, setProjection] = useState<ChangesHistoryProjection | null>(null);
  const [status, setStatus] = useState<ChangesHistoryStatus>("unread");
  const [busy, setBusy] = useState(false);

  const invalidate = useCallback(() => {
    setProjection(null);
    setStatus("unread");
  }, []);

  const beginRead = useCallback(() => { setStatus("loading"); }, []);

  const completeRead = useCallback((next: ChangesHistoryProjection) => {
    setProjection(next);
    setStatus("loaded");
  }, []);

  const failRead = useCallback(() => {
    // The projection is left alone: a failed refresh should not blank a
    // journal the user is already reading. The status carries the failure.
    setStatus("unavailable");
  }, []);

  return { beginRead, busy, completeRead, failRead, invalidate, projection, setBusy, status };
}
