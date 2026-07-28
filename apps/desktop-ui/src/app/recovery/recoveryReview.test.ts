import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import {
  createRecoveryActions,
  useRecoveryReview,
  type RecoveryActionPorts,
  type RecoveryReview,
  type RecoveryReviewState,
} from "./recoveryReview";

function review(overrides: Partial<RecoveryReview> = {}): RecoveryReview {
  return {
    status: "review_required",
    recoveryApprovalId: "appr-1",
    displayedRecoveryHash: "hash-shown",
    expiresAt: Date.now() + 60_000,
    ...overrides,
  } as RecoveryReview;
}

interface Harness {
  state: RecoveryReviewState;
  ports: RecoveryActionPorts;
  decided: { recoveryApprovalId: string; displayedRecoveryHash: string; choice: string }[];
  prepared: string[];
  refreshes: number;
  failures: number;
  errors: (string | null)[];
  authorityCurrent: boolean;
}

function harness(
  prepareResult: unknown = review(),
  overrides: Partial<RecoveryActionPorts> = {},
): Harness {
  const h = {
    authorityCurrent: true,
    decided: [] as Harness["decided"],
    errors: [] as (string | null)[],
    failures: 0,
    prepared: [] as string[],
    refreshes: 0,
  } as Harness;
  const { result } = renderHook(() => useRecoveryReview());
  Object.defineProperty(h, "state", { get: () => result.current });
  h.ports = {
    authorityIsCurrent: () => h.authorityCurrent,
    host: {
      decide: async (input) => { h.decided.push(input as never); },
      prepare: async ({ journalId }) => {
        h.prepared.push(journalId);
        return prepareResult as never;
      },
    },
    journalHasReview: () => true,
    manualReviewMessage: () => "Keep this journal quarantined for manual review.",
    refreshHistory: async () => { h.refreshes += 1; },
    reportFailure: () => { h.failures += 1; },
    setError: (message) => { h.errors.push(message); },
    workspace: { workspaceId: "w1", grantEpoch: 1 },
    ...overrides,
  };
  return h;
}

describe("preparing a review", () => {
  it("opens the review the host prepared", async () => {
    const h = harness();
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", document.createElement("button"));
    });
    expect(h.state.review?.recoveryApprovalId).toBe("appr-1");
    expect(h.state.busy).toBe(false);
  });

  it("remembers the row that opened it, so focus can go back", async () => {
    const trigger = document.createElement("button");
    const h = harness();
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", trigger);
    });
    expect(h.state.returnFocusTarget).toBe(trigger);
  });

  it("refuses a journal the host did not offer a review for", async () => {
    // Asking for one it did not offer is a request it will refuse, surfaced as
    // an error the user cannot act on.
    const h = harness(review(), { journalHasReview: () => false });
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", document.createElement("button"));
    });
    expect(h.prepared).toEqual([]);
  });

  it("refuses to prepare underneath a review the user is reading", async () => {
    const h = harness();
    const actions = () => createRecoveryActions(h.state, h.ports);
    await act(async () => {
      await actions().prepare("j1", document.createElement("button"));
    });
    await act(async () => {
      await actions().prepare("j2", document.createElement("button"));
    });
    // Replacing it would swap what they are reading for something they did not
    // ask for.
    expect(h.prepared).toEqual(["j1"]);
  });

  it("does nothing without a host or a workspace", async () => {
    for (const ports of [{ host: null }, { workspace: null }] as Partial<RecoveryActionPorts>[]) {
      const h = harness(review(), ports);
      await act(async () => {
        await createRecoveryActions(h.state, h.ports)
          .prepare("j1", document.createElement("button"));
      });
      expect(h.state.review).toBeNull();
    }
  });

  it("discards a preparation whose workspace binding was replaced", async () => {
    const h = harness();
    h.ports = { ...h.ports, authorityIsCurrent: () => false };
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", document.createElement("button"));
    });
    expect(h.state.review).toBeNull();
  });
});

describe("a journal that needs manual review", () => {
  const manual = { status: "manual_review", reasonCode: "checkpoint_incomplete_or_inconsistent" };

  it("explains why, and does not open a review", async () => {
    const h = harness(manual);
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", document.createElement("button"));
    });
    expect(h.state.review).toBeNull();
    expect(h.errors.at(-1)).toContain("quarantined");
  });

  it("sets the message after refreshing, not before", async () => {
    // The refresh clears the error, so setting it first erases it before the
    // user sees it.
    const order: string[] = [];
    const h = harness(manual);
    h.ports = {
      ...h.ports,
      refreshHistory: async () => { order.push("refresh"); },
      setError: (message) => { order.push(message === null ? "clear" : "message"); },
    };
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", document.createElement("button"));
    });
    expect(order.indexOf("refresh")).toBeLessThan(order.lastIndexOf("message"));
  });
});

describe("deciding", () => {
  async function withOpenReview(): Promise<Harness> {
    const h = harness();
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", document.createElement("button"));
    });
    return h;
  }

  /// The consent binding. Recovering replays writes into the user's
  /// repository, so the host must be told which recovery was displayed -- if
  /// its idea has moved since, the hash mismatches and it refuses.
  it("sends back the hash of the review that was displayed", async () => {
    const h = await withOpenReview();
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).decide("approve" as never);
    });
    expect(h.decided).toHaveLength(1);
    expect(h.decided[0]?.displayedRecoveryHash).toBe("hash-shown");
    expect(h.decided[0]?.recoveryApprovalId).toBe("appr-1");
  });

  it("closes the review and re-reads the journal list", async () => {
    const h = await withOpenReview();
    const before = h.refreshes;
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).decide("approve" as never);
    });
    expect(h.state.review).toBeNull();
    expect(h.refreshes).toBe(before + 1);
  });

  it("does nothing with no review open", async () => {
    const h = harness();
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).decide("approve" as never);
    });
    expect(h.decided).toEqual([]);
  });

  it("reports a failure and puts the review back to a usable state", async () => {
    const h = await withOpenReview();
    h.ports = {
      ...h.ports,
      host: { ...h.ports.host!, decide: () => Promise.reject(new Error("refused")) },
    };
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).decide("approve" as never);
    });
    expect(h.failures).toBe(1);
    expect(h.state.busy).toBe(false);
    expect(h.state.review).toBeNull();
  });
});

describe("clearing", () => {
  it("drops everything and supersedes work in flight", async () => {
    const h = harness();
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", document.createElement("button"));
    });
    expect(h.state.review).not.toBeNull();

    act(() => h.state.clear());
    expect(h.state.review).toBeNull();
    expect(h.state.busy).toBe(false);
    expect(h.state.returnFocusTarget).toBeNull();
    expect(h.state.decisionInFlight()).toBe(false);
  });

  it("makes an in-flight preparation land stale", async () => {
    // The generation bump is what makes clearing safe: a preparation that
    // resolves afterwards must not reopen a review for the old binding.
    let release: ((value: RecoveryReview) => void) | undefined;
    const h = harness();
    h.ports = {
      ...h.ports,
      host: {
        ...h.ports.host!,
        prepare: () => new Promise((resolve) => { release = resolve; }),
      },
    };
    const running = act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", document.createElement("button"));
    });
    act(() => h.state.clear());
    release?.(review());
    await running;
    expect(h.state.review).toBeNull();
  });
});

describe("the two locks", () => {
  it("suppresses a history refresh while a decision is running", async () => {
    // Expiring or refreshing under the host call would drop the response.
    const h = harness();
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", document.createElement("button"));
    });
    let observed = false;
    h.ports = {
      ...h.ports,
      host: {
        ...h.ports.host!,
        decide: async () => { observed = h.state.decisionInFlight(); },
      },
    };
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).decide("approve" as never);
    });
    expect(observed).toBe(true);
    expect(h.state.decisionInFlight()).toBe(false);
  });

  it("refuses a second decision while one is in flight", async () => {
    const h = harness();
    await act(async () => {
      await createRecoveryActions(h.state, h.ports).prepare("j1", document.createElement("button"));
    });
    const decide = vi.fn(() => new Promise<void>(() => undefined));
    h.ports = { ...h.ports, host: { ...h.ports.host!, decide } };
    const actions = createRecoveryActions(h.state, h.ports);
    void actions.decide("approve" as never);
    void actions.decide("approve" as never);
    expect(decide).toHaveBeenCalledTimes(1);
  });
});
