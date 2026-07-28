/**
 * A projection read from the host, with the staleness guard built in.
 *
 * FE-10 step 3. About and the retention manifest each wrote the same six lines:
 * capture the host binding generation, mark loading, await, compare, apply.
 * The comparison is the point -- these reads outlive the binding they were
 * started against, and a projection from a host that has since been replaced
 * describes a machine the app is no longer talking to. The retention manifest
 * is the sharper case: it lists what is still on disk, and showing a revoked
 * host's answer tells the user their data is somewhere it is not.
 *
 * The capture is done by `read`, before it awaits, so a caller cannot get the
 * ordering wrong. Reading the generation after the await -- the bug -- would
 * compare a value to itself and never detect anything.
 */

import { useCallback, useState } from "react";

/** Where a projection is in its lifecycle. */
export type ProjectionStatus = "unavailable" | "loading" | "ready";

export interface HostProjectionRead<T> {
  /**
   * The current host binding generation. Called once before the await and once
   * after; a difference means the binding was replaced and the result is
   * discarded.
   */
  readonly generation: () => number;
  readonly load: () => Promise<T>;
}

export interface HostProjection<T, Extra extends string = never> {
  readonly projection: T | null;
  readonly status: ProjectionStatus | Extra;
  readonly read: (options: HostProjectionRead<T>) => Promise<void>;
  /** For states this hook does not model -- "erasing", "erased". */
  readonly set: (projection: T | null, status: ProjectionStatus | Extra) => void;
  readonly setStatus: (status: ProjectionStatus | Extra) => void;
}

export function useHostProjection<T, Extra extends string = never>(): HostProjection<T, Extra> {
  const [projection, setProjection] = useState<T | null>(null);
  const [status, setStatus] = useState<ProjectionStatus | Extra>("unavailable");

  const read = useCallback(async ({ generation, load }: HostProjectionRead<T>) => {
    // Captured here rather than by the caller: this is the line that has to
    // come before the await, and putting it inside the helper is the only way
    // to make that true of every caller.
    const captured = generation();
    setStatus("loading");
    try {
      const value = await load();
      if (captured !== generation()) return;
      setProjection(value);
      setStatus("ready");
    } catch {
      // Also guarded. A read for the previous binding that rejects after the
      // new one is live would otherwise mark the *new* projection unavailable,
      // and nothing would retry it.
      if (captured !== generation()) return;
      setStatus("unavailable");
    }
  }, []);

  const set = useCallback((next: T | null, nextStatus: ProjectionStatus | Extra) => {
    setProjection(next);
    setStatus(nextStatus);
  }, []);

  return { projection, read, set, setStatus, status };
}
