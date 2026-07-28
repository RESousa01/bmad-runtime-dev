import { describe, expect, it, vi } from "vitest";

import { HostCommandError } from "../../lib/hostClient";
import { createChangesRunner, type ChangesRunnerPorts } from "./changesRunner";
import { isRecoveryFailure } from "../hostFailure";

type HostErrorDetails = ConstructorParameters<typeof HostCommandError>[0];

function hostError(code: HostErrorDetails["code"]): HostCommandError {
  return new HostCommandError({
    code,
    safeMessage: `Something went wrong (${code}).`,
    correlationId: "c1",
    retryable: false,
  });
}

interface Harness {
  ports: ChangesRunnerPorts;
  generation: number;
  errors: (string | null)[];
  recoveries: number;
  resets: string[];
}

function harness(originTask = "task-a"): Harness {
  const h: Harness = {
    errors: [],
    generation: 0,
    ports: {} as ChangesRunnerPorts,
    recoveries: 0,
    resets: [],
  };
  h.ports = {
    currentGeneration: () => h.generation,
    enterReadOnlyRecovery: () => { h.recoveries += 1; },
    nextGeneration: () => ++h.generation,
    originTask,
    resetFlow: (task) => { h.resets.push(task); },
    setError: (message) => { h.errors.push(message); },
  };
  return h;
}

describe("superseding", () => {
  it("reports the run as current while nothing else has started", async () => {
    const h = harness();
    let current: boolean | null = null;
    await createChangesRunner(h.ports)(async ({ isCurrent }) => {
      await Promise.resolve();
      current = isCurrent();
    });
    expect(current).toBe(true);
  });

  it("reports the run as superseded once another flow starts", async () => {
    // A slow proposal must not land on top of a review the user is already
    // reading.
    const h = harness();
    let current: boolean | null = null;
    await createChangesRunner(h.ports)(async ({ isCurrent }) => {
      h.generation += 1; // another flow starts, or the workspace changes
      await Promise.resolve();
      current = isCurrent();
    });
    expect(current).toBe(false);
  });

  it("bumps the generation on entry, so starting a second flow supersedes the first", async () => {
    const h = harness();
    const runner = createChangesRunner(h.ports);
    const captured: (() => boolean)[] = [];
    await runner(async ({ isCurrent }) => {
      captured.push(isCurrent);
      await Promise.resolve();
    });
    await runner(async () => { await Promise.resolve(); });
    expect(captured[0]?.()).toBe(false);
  });
});

describe("addressing the originating task", () => {
  it("hands the action the task that was in view when it started", async () => {
    // A governed change is a diff the user is being asked to approve. Applying
    // it to whichever task is on screen after the await is asking them to
    // approve edits they did not request.
    const h = harness("task-a");
    let seen: string | null = null;
    await createChangesRunner(h.ports)(async ({ originTask }) => {
      await Promise.resolve();
      seen = originTask;
    });
    expect(seen).toBe("task-a");
  });

  it("resets the originating task's flow after a failure, not the visible one", async () => {
    const h = harness("task-a");
    await createChangesRunner(h.ports)(() => Promise.reject(hostError("conflict")));
    expect(h.resets).toEqual(["task-a"]);
  });
});

describe("failures", () => {
  it("clears the error before the action runs", async () => {
    const h = harness();
    await createChangesRunner(h.ports)(() => Promise.resolve());
    expect(h.errors[0]).toBeNull();
  });

  it("enters read-only recovery rather than showing an error", async () => {
    for (const code of ["recovery_required", "integrity_failure"] as const) {
      const h = harness();
      await createChangesRunner(h.ports)(() => Promise.reject(hostError(code)));
      expect(h.recoveries).toBe(1);
      // Not an error to print: the host is saying its own state is
      // untrustworthy, and the app has to change mode.
      expect(h.errors.filter((e) => e !== null)).toEqual([]);
    }
  });

  it("shows an ordinary failure to the user", async () => {
    const h = harness();
    await createChangesRunner(h.ports)(() => Promise.reject(hostError("conflict")));
    expect(h.recoveries).toBe(0);
    expect(h.errors.at(-1)).toBe("Something went wrong (conflict).");
  });

  it("puts the flow back to idle so a fresh review can be taken", async () => {
    // The host consumes a pending proposal on any decision attempt, so a
    // failed apply always needs a new review -- pretending the old one is
    // still live offers the user an approval the host will refuse.
    const h = harness();
    await createChangesRunner(h.ports)(() => Promise.reject(hostError("conflict")));
    expect(h.resets).toEqual(["task-a"]);
  });

  it("drops a failure from a superseded flow entirely", async () => {
    // Otherwise a failure from an abandoned proposal blanks the review the
    // current flow just produced.
    const h = harness();
    await createChangesRunner(h.ports)(async () => {
      h.generation += 1;
      await Promise.resolve();
      throw hostError("conflict");
    });
    expect(h.resets).toEqual([]);
    expect(h.errors.filter((e) => e !== null)).toEqual([]);
    expect(h.recoveries).toBe(0);
  });

  it("does not propagate the host error to the caller", async () => {
    const h = harness();
    await expect(
      createChangesRunner(h.ports)(() => Promise.reject(new Error("boom"))),
    ).resolves.toBeUndefined();
  });
});

describe("isRecoveryFailure", () => {
  it("is the single definition both runners use", () => {
    expect(isRecoveryFailure(hostError("recovery_required"))).toBe(true);
    expect(isRecoveryFailure(hostError("integrity_failure"))).toBe(true);
    expect(isRecoveryFailure(hostError("conflict"))).toBe(false);
    expect(isRecoveryFailure(new Error("network"))).toBe(false);
  });
});

describe("the happy path", () => {
  it("leaves the flow alone when the action succeeds", async () => {
    const action = vi.fn(() => Promise.resolve());
    const h = harness();
    await createChangesRunner(h.ports)(action);
    expect(action).toHaveBeenCalledTimes(1);
    expect(h.resets).toEqual([]);
    expect(h.recoveries).toBe(0);
  });
});
