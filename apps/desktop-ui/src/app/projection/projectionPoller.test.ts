import { describe, expect, it } from "vitest";

import { HostCommandError } from "../../lib/hostClient";
import {
  classifyProjectionFailure,
  decideProjectionOutcome,
  shouldReloadLibraryAfterRebind,
  type ProjectionEventLike,
} from "./projectionPoller";

type HostErrorCode = ConstructorParameters<typeof HostCommandError>[0]["code"];

function hostError(code: HostErrorCode, retryable = false): HostCommandError {
  return new HostCommandError({
    code, safeMessage: "host said no", correlationId: null, retryable,
  });
}

function event(type: string, sequence: number, extra: object = {}): ProjectionEventLike {
  return { event: { type, ...extra } as ProjectionEventLike["event"], sequence };
}

describe("deciding what a batch of events asks for", () => {
  it("asks for nothing when the batch is empty", () => {
    expect(decideProjectionOutcome([])).toEqual({
      cursor: null,
      enterReadOnlyRecovery: false,
      refreshMethodLibrary: false,
      refreshWorkspaces: false,
    });
  });

  it("does not advance the cursor on an empty batch", () => {
    // Advancing would move past a sequence the host never sent.
    expect(decideProjectionOutcome([]).cursor).toBeNull();
  });

  it("advances the cursor to the last event in the batch", () => {
    const outcome = decideProjectionOutcome([
      event("workspace_changed", 7),
      event("bmad.projection_changed", 9),
    ]);
    expect(outcome.cursor).toBe(9);
  });

  it("asks for a workspace refresh on a workspace change", () => {
    expect(decideProjectionOutcome([event("workspace_changed", 1)]).refreshWorkspaces).toBe(true);
  });

  it("asks for a library refresh on a projection change", () => {
    expect(decideProjectionOutcome([event("bmad.projection_changed", 1)]).refreshMethodLibrary)
      .toBe(true);
  });

  it("recognises the host entering recovery", () => {
    const outcome = decideProjectionOutcome([
      event("boot_state_changed", 1, { projection: { mode: "read_only_recovery" } }),
    ]);
    expect(outcome.enterReadOnlyRecovery).toBe(true);
  });

  it("ignores a boot-state change that is not recovery", () => {
    const outcome = decideProjectionOutcome([
      event("boot_state_changed", 1, { projection: { mode: "ready" } }),
    ]);
    expect(outcome.enterReadOnlyRecovery).toBe(false);
  });

  it("ignores event types this shell does not react to", () => {
    const outcome = decideProjectionOutcome([event("something_new", 3)]);
    expect(outcome).toMatchObject({
      cursor: 3, enterReadOnlyRecovery: false,
      refreshMethodLibrary: false, refreshWorkspaces: false,
    });
  });

  it("collects every request in one batch", () => {
    const outcome = decideProjectionOutcome([
      event("workspace_changed", 1),
      event("bmad.projection_changed", 2),
      event("boot_state_changed", 3, { projection: { mode: "read_only_recovery" } }),
    ]);
    expect(outcome).toEqual({
      cursor: 3,
      enterReadOnlyRecovery: true,
      refreshMethodLibrary: true,
      refreshWorkspaces: true,
    });
  });
});

describe("classifying a poll failure", () => {
  /// The most consequential decision in the poller. `conflict` is not a
  /// failure of the host: it is this renderer having fallen behind what the
  /// host can still replay, and the response is to catch up rather than to
  /// tear anything down.
  it("treats conflict as a cursor gap, not a recovery or an outage", () => {
    expect(classifyProjectionFailure(hostError("conflict"))).toEqual({ kind: "cursor_gap" });
  });

  it("treats the two recovery codes as recovery", () => {
    expect(classifyProjectionFailure(hostError("recovery_required"))).toEqual({ kind: "recovery" });
    expect(classifyProjectionFailure(hostError("integrity_failure"))).toEqual({ kind: "recovery" });
  });

  it("retries a failure the host says is retryable", () => {
    expect(classifyProjectionFailure(hostError("temporarily_unavailable", true)))
      .toEqual({ kind: "retry" });
  });

  it("treats anything else as the host having stopped answering", () => {
    expect(classifyProjectionFailure(hostError("internal"))).toEqual({ kind: "unavailable" });
    expect(classifyProjectionFailure(new Error("network"))).toEqual({ kind: "unavailable" });
  });

  it("does not let a retryable flag override a recovery code", () => {
    // The host asking for recovery is not something to retry past.
    expect(classifyProjectionFailure(hostError("recovery_required", true)))
      .toEqual({ kind: "recovery" });
  });
});

describe("reloading the library after a re-bootstrap", () => {
  const base = {
    bootMode: "ready",
    libraryWasRequested: true,
    supportedCommands: ["bmad.library.snapshot"],
  };

  it("reloads when it had been opened before the gap", () => {
    expect(shouldReloadLibraryAfterRebind(base)).toBe(true);
  });

  it("does not start fetching something the user never opened", () => {
    // Re-establishing authority is not a reason to begin work nobody asked
    // for.
    expect(shouldReloadLibraryAfterRebind({ ...base, libraryWasRequested: false })).toBe(false);
  });

  it("does not reload into a host that came back in recovery", () => {
    expect(shouldReloadLibraryAfterRebind({ ...base, bootMode: "read_only_recovery" }))
      .toBe(false);
  });

  it("does not reload when the rebound host no longer offers the command", () => {
    expect(shouldReloadLibraryAfterRebind({ ...base, supportedCommands: [] })).toBe(false);
  });
});
