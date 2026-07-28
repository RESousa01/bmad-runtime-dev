import { afterEach, describe, expect, it, vi } from "vitest";

import { HostCommandError } from "../../lib/hostClient";
import {
  startProjectionSubscription,
  type ProjectionEventLike,
  type ProjectionSubscriptionPorts,
} from "./projectionPoller";

type HostErrorCode = ConstructorParameters<typeof HostCommandError>[0]["code"];

function hostError(code: HostErrorCode, retryable = false): HostCommandError {
  return new HostCommandError({
    code, safeMessage: "host said no", correlationId: null, retryable,
  });
}

type Client = { readonly id: string };
const CLIENT: Client = { id: "client-1" };

interface Harness {
  ports: ProjectionSubscriptionPorts<Client>;
  generation: number;
  calls: string[];
  recoveries: { sequence: number }[];
  unavailable: number;
  rebounds: { bootMode: string }[];
}

function harness(overrides: Partial<ProjectionSubscriptionPorts<Client>> = {}): Harness {
  const h = {
    calls: [] as string[],
    generation: 1,
    rebounds: [] as { bootMode: string }[],
    recoveries: [] as { sequence: number }[],
    unavailable: 0,
  } as Harness;
  h.ports = {
    adoptReboundRuntime: (_client, bootstrap) => {
      h.calls.push("adopt");
      h.rebounds.push({ bootMode: bootstrap.bootMode });
    },
    bindingGeneration: () => h.generation,
    fetchEvents: async () => { h.calls.push("events"); return []; },
    fetchSnapshot: async () => {
      h.calls.push("snapshot");
      return { bootMode: "ready", sequence: 50 };
    },
    libraryWasRequested: () => true,
    markHostUnavailable: () => { h.calls.push("unavailable"); h.unavailable += 1; },
    markReadOnlyRecovery: (_client, sequence) => {
      h.calls.push("recovery");
      h.recoveries.push({ sequence });
    },
    pollIntervalMs: 100,
    reBootstrap: async () => {
      h.calls.push("bootstrap");
      return { bootMode: "ready", supportedCommands: ["bmad.library.snapshot"] };
    },
    refreshMethodLibrary: async () => { h.calls.push("library"); },
    refreshWorkspaces: async () => { h.calls.push("workspaces"); },
    runtime: { client: CLIENT, startSequence: 10 },
    ...overrides,
  };
  return h;
}

/** Advances past one poll interval and lets the async tick settle. */
async function tick(times = 1): Promise<void> {
  for (let i = 0; i < times; i += 1) {
    await vi.advanceTimersByTimeAsync(100);
  }
}

afterEach(() => { vi.useRealTimers(); });

describe("starting and stopping", () => {
  it("does nothing without a bound runtime", async () => {
    vi.useFakeTimers();
    const h = harness({ runtime: null });
    const stop = startProjectionSubscription(h.ports);
    await tick(3);
    expect(h.calls).toEqual([]);
    stop();
  });

  it("polls on the interval", async () => {
    vi.useFakeTimers();
    const h = harness();
    const stop = startProjectionSubscription(h.ports);
    await tick(3);
    expect(h.calls.filter((c) => c === "events")).toHaveLength(3);
    stop();
  });

  it("stops polling once torn down", async () => {
    vi.useFakeTimers();
    const h = harness();
    startProjectionSubscription(h.ports)();
    await tick(3);
    expect(h.calls).toEqual([]);
  });

  it("is safe to stop twice", () => {
    vi.useFakeTimers();
    const stop = startProjectionSubscription(harness().ports);
    expect(() => { stop(); stop(); }).not.toThrow();
  });

  /// The subscription belongs to one host binding. A binding replaced
  /// mid-flight makes every remaining step of the tick describe a host the app
  /// is no longer talking to.
  it("stops when the host binding is replaced", async () => {
    vi.useFakeTimers();
    const h = harness();
    const stop = startProjectionSubscription(h.ports);
    await tick(1);
    h.generation += 1;
    await tick(3);
    expect(h.calls.filter((c) => c === "events")).toHaveLength(1);
    stop();
  });
});

describe("reacting to a batch", () => {
  it("refreshes workspaces before entering recovery", async () => {
    // A batch can carry both, and the recovery transition ends the tick -- so
    // doing it first would drop the workspace list the same batch delivered.
    vi.useFakeTimers();
    const events: ProjectionEventLike[] = [
      { event: { type: "workspace_changed" }, sequence: 11 },
      {
        event: { type: "boot_state_changed", projection: { mode: "read_only_recovery" } },
        sequence: 12,
      },
    ];
    const h = harness({ fetchEvents: async () => events });
    const stop = startProjectionSubscription(h.ports);
    await tick(1);
    // Both, in order. Comparing indexOf alone passes when the workspace
    // refresh never happened at all, because -1 is less than everything.
    expect(h.calls).toEqual(["workspaces", "recovery"]);
    stop();
  });

  it("reports recovery at the cursor the batch advanced to", async () => {
    vi.useFakeTimers();
    const h = harness({
      fetchEvents: async () => [{
        event: { type: "boot_state_changed", projection: { mode: "read_only_recovery" } },
        sequence: 42,
      }],
    });
    const stop = startProjectionSubscription(h.ports);
    await tick(1);
    expect(h.recoveries[0]?.sequence).toBe(42);
    stop();
  });

  it("does not refresh the library the user never opened", async () => {
    vi.useFakeTimers();
    const h = harness({
      fetchEvents: async () => [{ event: { type: "bmad.projection_changed" }, sequence: 11 }],
      libraryWasRequested: () => false,
    });
    const stop = startProjectionSubscription(h.ports);
    await tick(1);
    expect(h.calls).not.toContain("library");
    stop();
  });
});

describe("recovering from a cursor gap", () => {
  const gap = () => Promise.reject(hostError("conflict"));

  /// The snapshot is summary-only, so it cannot reconstruct the session,
  /// approval, execution, evidence, connectivity and update events that fell in
  /// the gap. Advancing past them would leave the renderer showing a state it
  /// never derived, so the response is a fresh binding.
  it("re-bootstraps rather than skipping ahead", async () => {
    vi.useFakeTimers();
    const h = harness({ fetchEvents: gap });
    const stop = startProjectionSubscription(h.ports);
    await tick(1);
    expect(h.calls).toContain("snapshot");
    expect(h.calls).toContain("bootstrap");
    expect(h.rebounds).toEqual([{ bootMode: "ready" }]);
    stop();
  });

  it("does not re-bootstrap when the snapshot already says recovery", async () => {
    vi.useFakeTimers();
    const h = harness({
      fetchEvents: gap,
      fetchSnapshot: async () => ({ bootMode: "read_only_recovery", sequence: 77 }),
    });
    const stop = startProjectionSubscription(h.ports);
    await tick(1);
    expect(h.calls).not.toContain("bootstrap");
    expect(h.recoveries[0]?.sequence).toBe(77);
    stop();
  });

  it("treats a host that cannot re-bootstrap as one that stopped answering", async () => {
    vi.useFakeTimers();
    const h = harness({
      fetchEvents: gap,
      reBootstrap: () => Promise.reject(new Error("gone")),
    });
    const stop = startProjectionSubscription(h.ports);
    await tick(1);
    expect(h.unavailable).toBe(1);
    stop();
  });

  it("does not reload a library the user never opened after a rebind", async () => {
    vi.useFakeTimers();
    const h = harness({ fetchEvents: gap, libraryWasRequested: () => false });
    const stop = startProjectionSubscription(h.ports);
    await tick(1);
    expect(h.calls).not.toContain("library");
    stop();
  });
});

describe("failures", () => {
  it("marks the host unavailable on a non-retryable failure", async () => {
    vi.useFakeTimers();
    const h = harness({ fetchEvents: () => Promise.reject(hostError("internal")) });
    const stop = startProjectionSubscription(h.ports);
    await tick(1);
    expect(h.unavailable).toBe(1);
    stop();
  });

  it("leaves the binding alone on a retryable failure", async () => {
    vi.useFakeTimers();
    const h = harness({
      fetchEvents: () => Promise.reject(hostError("temporarily_unavailable", true)),
    });
    const stop = startProjectionSubscription(h.ports);
    await tick(1);
    expect(h.unavailable).toBe(0);
    expect(h.recoveries).toEqual([]);
    stop();
  });

  /// A poller that stops on one bad response looks exactly like a host that
  /// went quiet, and nothing would ever start it again.
  it("keeps polling after a failed tick", async () => {
    vi.useFakeTimers();
    let calls = 0;
    const h = harness({
      fetchEvents: async () => {
        calls += 1;
        if (calls === 1) throw hostError("temporarily_unavailable", true);
        return [];
      },
    });
    const stop = startProjectionSubscription(h.ports);
    await tick(3);
    expect(calls).toBeGreaterThan(1);
    stop();
  });

  it("does not reschedule after the binding has been replaced", async () => {
    vi.useFakeTimers();
    let fetches = 0;
    const h = harness({
      fetchEvents: async () => {
        fetches += 1;
        h.generation += 1;
        throw hostError("internal");
      },
    });
    const stop = startProjectionSubscription(h.ports);
    await tick(3);
    expect(fetches).toBe(1);
    // And the failure is not reported: it belongs to a binding that is gone.
    expect(h.unavailable).toBe(0);
    stop();
  });
});
