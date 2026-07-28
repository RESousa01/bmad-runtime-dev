import { describe, expect, it, vi } from "vitest";

import type {
  ActiveCapabilityRunState,
  CapabilityRouting,
  CapabilityRunUpdate,
} from "../../state/taskSessions";
import {
  anyPhase,
  approved,
  createCapabilityRunner,
  underReview,
  type CapabilityRunnerPorts,
} from "./capabilityController";

function run(
  phase: ActiveCapabilityRunState["phase"] = { kind: "selecting" },
): ActiveCapabilityRunState {
  return { capabilityId: "cap-1", label: "Draft a PRD", phase };
}

const workspace = { workspaceId: "w1", grantEpoch: 2 };

interface Harness {
  ports: CapabilityRunnerPorts;
  /** Tokens handed out, in order, so "captured before the await" is checkable. */
  captured: CapabilityRouting[];
  routed: { token: CapabilityRouting; update: CapabilityRunUpdate }[];
  /** Bumped between captures so two tokens are distinguishable. */
  activeTask: string;
}

function harness(overrides: Partial<CapabilityRunnerPorts> = {}): Harness {
  const h: Harness = {
    activeTask: "task-a",
    captured: [],
    ports: {} as CapabilityRunnerPorts,
    routed: [],
  };
  h.ports = {
    run: run(),
    sessions: {
      captureActive: () => {
        const token = { sessionId: h.activeTask } as unknown as CapabilityRouting;
        h.captured.push(token);
        return token;
      },
      route: (token, update) => {
        h.routed.push({ token, update });
        return true;
      },
    },
    workspace,
    ...overrides,
  };
  return h;
}

describe("routing a run to the task that started it", () => {
  /// The whole reason this module exists. A run belongs to the task it was
  /// started from, and the host call outlives the user's attention -- they
  /// switch tabs while a run is preparing.
  it("captures the routing token before awaiting the host", async () => {
    const h = harness();
    let capturedAtAwait = 0;
    await createCapabilityRunner(h.ports)(anyPhase, async () => {
      capturedAtAwait = h.captured.length;
      await Promise.resolve();
    });
    expect(capturedAtAwait).toBe(1);
  });

  it("applies the result through the token, not the session that is active later", async () => {
    const h = harness();
    await createCapabilityRunner(h.ports)(anyPhase, async ({ route }) => {
      // The user switches tabs while the host is working.
      h.activeTask = "task-b";
      await Promise.resolve();
      route(null);
    });
    expect(h.routed).toHaveLength(1);
    expect(h.routed[0]?.token).toBe(h.captured[0]);
    expect((h.routed[0]?.token as unknown as { sessionId: string }).sessionId).toBe("task-a");
  });

  it("routes a failure to the same task as a success", async () => {
    const h = harness();
    await createCapabilityRunner(h.ports)(anyPhase, async () => {
      h.activeTask = "task-b";
      throw new Error("host refused");
    });
    expect(h.routed).toHaveLength(1);
    expect((h.routed[0]?.token as unknown as { sessionId: string }).sessionId).toBe("task-a");
  });

  it("turns a thrown host error into an error phase rather than propagating", async () => {
    const h = harness();
    await expect(
      createCapabilityRunner(h.ports)(anyPhase, () => Promise.reject(new Error("boom"))),
    ).resolves.toBeUndefined();

    const update = h.routed[0]?.update;
    expect(typeof update).toBe("function");
    const next = (update as (c: ActiveCapabilityRunState | null) => ActiveCapabilityRunState | null)(
      run(),
    );
    expect(next?.phase.kind).toBe("error");
  });

  it("leaves a null run null when a failure arrives after it was cleared", () => {
    // The task was deleted while the host was working; there is nothing to
    // put an error phase on.
    const h = harness();
    return createCapabilityRunner(h.ports)(anyPhase, () => Promise.reject(new Error("boom")))
      .then(() => {
        const update = h.routed[0]?.update as
          (c: ActiveCapabilityRunState | null) => ActiveCapabilityRunState | null;
        expect(update(null)).toBeNull();
      });
  });
});

describe("phase guards", () => {
  it("refuses an action the run is not in the right phase for", async () => {
    const action = vi.fn();
    const h = harness({ run: run({ kind: "selecting" }) });
    await createCapabilityRunner(h.ports)(underReview, action);
    expect(action).not.toHaveBeenCalled();
  });

  it("does not capture a token for a refused action", async () => {
    // A refused action must leave no trace: capturing would claim routing for
    // work that never started.
    const h = harness({ run: run({ kind: "selecting" }) });
    await createCapabilityRunner(h.ports)(approved, vi.fn());
    expect(h.captured).toEqual([]);
    expect(h.routed).toEqual([]);
  });

  it("allows an approval only while under review", () => {
    expect(underReview({ kind: "review" } as ActiveCapabilityRunState["phase"])).toBe(true);
    expect(underReview({ kind: "selecting" })).toBe(false);
    expect(underReview({ kind: "preparing" })).toBe(false);
  });

  it("allows a submission only once approved", () => {
    expect(approved({ kind: "approved" } as ActiveCapabilityRunState["phase"])).toBe(true);
    expect(approved({ kind: "review" } as ActiveCapabilityRunState["phase"])).toBe(false);
  });
});

describe("preconditions", () => {
  it("does nothing with no run open", async () => {
    const action = vi.fn();
    const h = harness({ run: null });
    await createCapabilityRunner(h.ports)(anyPhase, action);
    expect(action).not.toHaveBeenCalled();
    expect(h.captured).toEqual([]);
  });

  it("does nothing with no workspace bound", async () => {
    // Every capability command carries a workspace id and grant epoch; without
    // one there is nothing the host would accept.
    const action = vi.fn();
    const h = harness({ workspace: null });
    await createCapabilityRunner(h.ports)(anyPhase, action);
    expect(action).not.toHaveBeenCalled();
  });

  it("hands the action the grant epoch, not just the workspace id", async () => {
    // The epoch is what the host checks: a stale one means the grant was
    // re-issued and the run must not be accepted under the old authority.
    const h = harness();
    let seen: typeof workspace | null = null;
    await createCapabilityRunner(h.ports)(anyPhase, async (context) => {
      seen = context.workspace;
      await Promise.resolve();
    });
    expect(seen).toEqual({ workspaceId: "w1", grantEpoch: 2 });
  });
});
