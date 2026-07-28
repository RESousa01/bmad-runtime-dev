import { describe, expect, it, vi } from "vitest";

import { HostCommandError } from "../../lib/hostClient";
import type { BmadRequestEvent, BmadRequestState } from "../../lib/bmadModelProjection";
import {
  createGuidanceRequest,
  isInFlight,
  type GuidanceRequestPorts,
} from "./guidanceRequest";

function slot<T>(initial: T) {
  let held = initial;
  return { get: () => held, set: (value: T) => { held = value; } };
}

interface Harness {
  ports: GuidanceRequestPorts;
  events: BmadRequestEvent["type"][];
  generation: number;
  recoveries: number;
  intents: string[];
  /** What `applyEvent` should report back, per event type. */
  stateAfter: Map<string, BmadRequestState>;
  state: BmadRequestState;
}

const RUN = { runId: "run-1" };
const AUTHORITY = {
  workspaceId: "w1",
  workspaceGrantEpoch: 1,
  runId: "run-1",
  authEpoch: 3,
  rendererGeneration: 1,
  now: 0,
};

function harness(overrides: Partial<GuidanceRequestPorts> = {}): Harness {
  const h = {
    events: [] as BmadRequestEvent["type"][],
    generation: 0,
    intents: [] as string[],
    recoveries: 0,
    state: { kind: "idle", run: null } as BmadRequestState,
    stateAfter: new Map<string, BmadRequestState>(),
  } as Harness;

  h.ports = {
    applyEvent: (event) => {
      h.events.push(event.type);
      const next = h.stateAfter.get(event.type);
      if (next !== undefined) h.state = next;
      return h.state;
    },
    client: {
      createBmadHelpRun: vi.fn(async () => RUN),
      prepareBmadHelp: vi.fn(async () => ({ manifestHash: "m1" })),
      approveBmadHelp: vi.fn(async () => ({ decisionId: "d1" })),
      submitBmadHelp: vi.fn(async () => ({ ok: true })),
      cancelBmadHelp: vi.fn(),
      latestBmadHelpRun: vi.fn(),
      bmadWorkflowState: vi.fn(),
    } as unknown as GuidanceRequestPorts["client"],
    contextPaths: [],
    creation: slot<Promise<void> | null>(null),
    currentAuthority: () => AUTHORITY,
    currentGeneration: () => h.generation,
    currentState: () => h.state,
    enterReadOnlyRecovery: () => { h.recoveries += 1; },
    modelAuth: { status: "development_ready", authEpoch: 3 } as never,
    nextGeneration: () => ++h.generation,
    now: () => 0,
    operation: slot<"approve" | "cancel" | "submit" | null>(null),
    recordIntent: (intent) => { h.intents.push(intent); },
    workspace: { workspaceId: "w1", grantEpoch: 1 },
    ...overrides,
  };
  return h;
}

type HostErrorCode = ConstructorParameters<typeof HostCommandError>[0]["code"];

function hostError(code: HostErrorCode): HostCommandError {
  return new HostCommandError({
    code, safeMessage: "host said no", correlationId: null, retryable: false,
  });
}

describe("the creation lock", () => {
  /// A second caller is asking about the request already running. Handing them
  /// a fresh rejection would report a failure while the first was succeeding.
  it("hands a concurrent caller the same promise, not a new request", () => {
    const h = harness();
    const guidance = createGuidanceRequest(h.ports);
    const first = guidance.send("hello").catch(() => undefined);
    const second = guidance.send("hello again");
    expect(second).toBe(h.ports.creation.get());
    expect(h.ports.client!.createBmadHelpRun).toHaveBeenCalledTimes(1);
    return first;
  });

  it("records the intent once, not once per caller", () => {
    const h = harness();
    const guidance = createGuidanceRequest(h.ports);
    const running = guidance.send("hello").catch(() => undefined);
    void guidance.send("hello");
    expect(h.intents).toEqual(["hello"]);
    return running;
  });

  it("releases the lock when the request settles", async () => {
    const h = harness();
    await createGuidanceRequest(h.ports).send("hello").catch(() => undefined);
    expect(h.ports.creation.get()).toBeNull();
  });

  it("does not release a lock a second request has taken", async () => {
    /*
     * The real sequence, which needs three steps to reach: the first request
     * is in flight, something invalidates it and clears the slot, a second
     * request starts and claims the slot, and only then does the first settle.
     * Clearing unconditionally in that `finally` would unlock the second
     * request while it is still running.
     */
    const h = harness();
    let releaseFirst: (() => void) | undefined;
    h.ports.client!.createBmadHelpRun = vi.fn(() => new Promise((resolve) => {
      releaseFirst = () => resolve(RUN);
    })) as never;

    const guidance = createGuidanceRequest(h.ports);
    const first = guidance.send("first").catch(() => undefined);

    // Invalidation clears the slot, as `clearBmadHelpProjection` does.
    h.ports.creation.set(null);
    const second = Promise.resolve();
    h.ports.creation.set(second);

    releaseFirst?.();
    await first;
    expect(h.ports.creation.get()).toBe(second);
  });
});

describe("preconditions", () => {
  it("refuses with no client", async () => {
    const h = harness({ client: null });
    await expect(createGuidanceRequest(h.ports).send("hi")).rejects.toThrow(/unavailable/u);
  });

  it("refuses with no workspace", async () => {
    const h = harness({ workspace: null });
    await expect(createGuidanceRequest(h.ports).send("hi")).rejects.toThrow(/unavailable/u);
  });

  it("refuses when the model is not ready", async () => {
    const h = harness({ modelAuth: { status: "unavailable" } as never });
    await expect(createGuidanceRequest(h.ports).send("hi")).rejects.toThrow(/unavailable/u);
  });

  it("refuses while another approve or submit is running", async () => {
    // Reachable on its own from the review UI, so it is a separate lock from
    // the creation one.
    const h = harness({ operation: slot<"approve" | "cancel" | "submit" | null>("submit") });
    await expect(createGuidanceRequest(h.ports).send("hi")).rejects.toThrow(/unavailable/u);
  });

  it("does not record an intent for a refused request", async () => {
    const h = harness({ client: null });
    await createGuidanceRequest(h.ports).send("hi").catch(() => undefined);
    expect(h.intents).toEqual([]);
  });
});

describe("the generation guard", () => {
  /// Every await is a place the authority behind the request can change: a new
  /// task, a workspace switch, a model change, a host rebind.
  it("rejects a run created under an authority that has since changed", async () => {
    const h = harness();
    h.ports.client!.createBmadHelpRun = vi.fn(async () => {
      h.generation += 1; // something invalidated the request mid-flight
      return RUN;
    }) as never;
    await expect(createGuidanceRequest(h.ports).send("hi"))
      .rejects.toThrow(/no longer belongs/u);
    expect(h.ports.client!.prepareBmadHelp).not.toHaveBeenCalled();
  });

  it("rejects a prepared review whose authority changed during preparation", async () => {
    const h = harness();
    h.ports.client!.prepareBmadHelp = vi.fn(async () => {
      h.generation += 1;
      return { manifestHash: "m1" };
    }) as never;
    await expect(createGuidanceRequest(h.ports).send("hi"))
      .rejects.toThrow(/model connection changed/u);
  });

  it("does not report a failure from a superseded request", async () => {
    // It would overwrite the state of whatever replaced it.
    const h = harness();
    h.ports.client!.createBmadHelpRun = vi.fn(async () => {
      h.generation += 5;
      throw hostError("conflict");
    }) as never;
    await createGuidanceRequest(h.ports).send("hi").catch(() => undefined);
    expect(h.events).not.toContain("unavailable");
  });

  it("reports a failure from the request that is still current", async () => {
    const h = harness();
    h.ports.client!.createBmadHelpRun = vi.fn(() => Promise.reject(hostError("conflict"))) as never;
    await createGuidanceRequest(h.ports).send("hi").catch(() => undefined);
    expect(h.events).toContain("unavailable");
  });
});

describe("recovery classification", () => {
  it("enters read-only recovery rather than showing an error", async () => {
    for (const code of ["recovery_required", "integrity_failure"] as const) {
      const h = harness();
      h.ports.client!.createBmadHelpRun = vi.fn(() => Promise.reject(hostError(code))) as never;
      await createGuidanceRequest(h.ports).send("hi").catch(() => undefined);
      expect(h.recoveries, code).toBe(1);
      expect(h.events, code).not.toContain("unavailable");
    }
  });

  it("shows an ordinary failure instead", async () => {
    const h = harness();
    h.ports.client!.createBmadHelpRun = vi.fn(() => Promise.reject(hostError("conflict"))) as never;
    await createGuidanceRequest(h.ports).send("hi").catch(() => undefined);
    expect(h.recoveries).toBe(0);
  });
});

describe("approveAndSubmit", () => {
  /*
   * Matches `AUTHORITY` field for field. `bmadRequestAuthorityIsCurrent`
   * compares all five plus an expiry, so a fixture that differs anywhere fails
   * the *first* check and never reaches the one under test -- which is how the
   * first version of this test passed for the wrong reason.
   */
  const requestAuthority = {
    workspaceId: "w1",
    workspaceGrantEpoch: 1,
    runId: "run-1",
    sessionId: "s1",
    authEpoch: 3,
    rendererGeneration: 1,
    manifestHash: "m1",
    expiresAt: 1_000,
  };
  const prepared = {
    kind: "review_required",
    run: { workspaceId: "w1", runId: "run-1", sessionId: "s1" },
    runProjection: null,
    review: { manifestHash: "m1" },
    authority: requestAuthority,
  } as unknown as Extract<BmadRequestState, { kind: "review_required" }>;

  it("refuses when the authority no longer matches", async () => {
    // Approving one manifest and submitting under another is a write the host
    // would accept and the user never sanctioned.
    const h = harness({ currentAuthority: () => null });
    await expect(createGuidanceRequest(h.ports).approveAndSubmit(prepared))
      .rejects.toThrow(/authority changed/u);
    expect(h.events).toContain("authority_invalidated");
    expect(h.ports.client!.approveBmadHelp).not.toHaveBeenCalled();
  });

  it("refuses while another operation holds the lock", async () => {
    const h = harness({ operation: slot<"approve" | "cancel" | "submit" | null>("approve") });
    await expect(createGuidanceRequest(h.ports).approveAndSubmit(prepared))
      .rejects.toThrow(/can no longer be sent/u);
  });

  it("re-checks the authority between approving and submitting", async () => {
    /*
     * The grant epoch moves while the generation does not, so only the
     * authority re-check can catch it. Bumping the generation instead would be
     * caught by the generation guard and the test would pass with the
     * re-check deleted -- which is what the first version of this test did.
     */
    let authority = { ...AUTHORITY, rendererGeneration: 1 };
    const h = harness({ currentAuthority: () => authority });
    h.generation = 1;
    h.stateAfter.set("approve_started", {
      kind: "approving", run: prepared.run, runProjection: null,
      review: prepared.review, authority: requestAuthority,
    } as unknown as BmadRequestState);
    h.ports.client!.approveBmadHelp = vi.fn(async () => {
      // The workspace grant was re-issued while the approval was in flight.
      authority = { ...authority, workspaceGrantEpoch: 2 };
      return { decisionId: "d1" };
    }) as never;
    await expect(createGuidanceRequest(h.ports).approveAndSubmit(prepared))
      .rejects.toThrow(/authority changed before sending/u);
    expect(h.ports.client!.submitBmadHelp).not.toHaveBeenCalled();
  });

  it("always releases the operation lock", async () => {
    const h = harness({ currentAuthority: () => AUTHORITY });
    h.ports.client!.approveBmadHelp = vi.fn(() => Promise.reject(hostError("conflict"))) as never;
    await createGuidanceRequest(h.ports).approveAndSubmit(prepared).catch(() => undefined);
    // A lock left held by a thrown operation makes the feature permanently
    // unusable for the rest of the session.
    expect(h.ports.operation.get()).toBeNull();
  });
});

describe("isInFlight", () => {
  it("covers exactly the states with an outstanding approval or submission", () => {
    for (const kind of ["creating", "approving", "approved", "submitting"]) {
      expect(isInFlight({ kind } as BmadRequestState), kind).toBe(true);
    }
    for (const kind of ["idle", "review_required", "completed", "unavailable"]) {
      expect(isInFlight({ kind } as BmadRequestState), kind).toBe(false);
    }
  });
});
