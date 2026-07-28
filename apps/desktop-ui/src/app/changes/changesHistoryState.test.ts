import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { ChangesHistoryProjection } from "../../lib/hostClient";
import { useChangesHistoryState } from "./changesHistoryState";

function projection(
  overrides: Partial<ChangesHistoryProjection> = {},
): ChangesHistoryProjection {
  return {
    executions: [],
    openJournals: [],
    ...overrides,
  } as ChangesHistoryProjection;
}

describe("the Activity journal read state", () => {
  it("starts unread with nothing to show", () => {
    // Not "loaded and empty". The panel renders those identically as "No
    // activity yet", and someone checking whether a change could still be
    // rolled back was told it could not.
    const { result } = renderHook(() => useChangesHistoryState());
    expect(result.current.status).toBe("unread");
    expect(result.current.projection).toBeNull();
  });

  it("distinguishes never-asked from asked-and-empty", () => {
    const { result } = renderHook(() => useChangesHistoryState());
    act(() => result.current.beginRead());
    act(() => result.current.completeRead(projection()));
    expect(result.current.status).toBe("loaded");
    expect(result.current.projection).not.toBeNull();
  });

  it("locks out a second read while one is running", () => {
    // `loading` is the lock. The load-on-open effect fires only on `unread`,
    // so leaving the status alone during a read would make it a poll.
    const { result } = renderHook(() => useChangesHistoryState());
    act(() => result.current.beginRead());
    expect(result.current.status).toBe("loading");
  });

  /// The invariant four call sites used to hand-write, with a comment each and
  /// no test. Clearing the projection without resetting the status leaves it
  /// `loaded`, the load-on-open effect never fires, and Activity reports "No
  /// activity yet" immediately after a governed change is applied.
  it("returns to unread when invalidated, not just empty", () => {
    const { result } = renderHook(() => useChangesHistoryState());
    act(() => result.current.beginRead());
    act(() => result.current.completeRead(projection()));

    act(() => result.current.invalidate());

    expect(result.current.projection).toBeNull();
    expect(result.current.status).toBe("unread");
  });

  it("keeps the journal on screen when a refresh fails", () => {
    // A failed refresh must not blank a journal the user is already reading.
    // The status carries the failure instead.
    const { result } = renderHook(() => useChangesHistoryState());
    act(() => result.current.beginRead());
    const loaded = projection();
    act(() => result.current.completeRead(loaded));

    act(() => result.current.beginRead());
    act(() => result.current.failRead());

    expect(result.current.status).toBe("unavailable");
    expect(result.current.projection).toBe(loaded);
  });

  it("does not re-read after a failure until something invalidates it", () => {
    // `unavailable` is not `unread`, so the load-on-open effect stays quiet
    // rather than retrying a failing read on every drawer open.
    const { result } = renderHook(() => useChangesHistoryState());
    act(() => result.current.beginRead());
    act(() => result.current.failRead());
    expect(result.current.status).not.toBe("unread");

    act(() => result.current.invalidate());
    expect(result.current.status).toBe("unread");
  });

  it("tracks the refresh button's busy flag separately from the read status", () => {
    // The button reflects a user-initiated refresh; the status drives the
    // load-on-open effect. Conflating them made Refresh spin on a background
    // read the user did not ask for.
    const { result } = renderHook(() => useChangesHistoryState());
    act(() => result.current.setBusy(true));
    expect(result.current.busy).toBe(true);
    expect(result.current.status).toBe("unread");
  });
});
