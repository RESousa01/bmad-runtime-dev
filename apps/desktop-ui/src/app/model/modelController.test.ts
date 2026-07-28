import { describe, expect, it, vi } from "vitest";

import { HostCommandError, type ModelAuthStatusProjection } from "../../lib/hostClient";
import {
  createModelActions,
  modelUnavailableNotices,
  type ModelControllerPorts,
  type ModelHostPort,
} from "./modelController";

function status(
  overrides: Partial<ModelAuthStatusProjection> = {},
): ModelAuthStatusProjection {
  return {
    status: "development_ready",
    destinationLabel: "OpenAI gpt-5",
    authEpoch: 3,
    ...overrides,
  } as ModelAuthStatusProjection;
}

interface Harness {
  ports: ModelControllerPorts;
  busy: boolean[];
  notices: (string | null)[];
  adopted: ModelAuthStatusProjection[];
}

function harness(host: Partial<ModelHostPort> | null = {}): Harness {
  const h: Harness = { adopted: [], busy: [], notices: [], ports: {} as ModelControllerPorts };
  h.ports = {
    adoptAuthority: (next) => { h.adopted.push(next); },
    host: host === null ? null : {
      canSelectModel: true,
      canSignIn: true,
      canSignOut: true,
      setModelSelection: () => Promise.resolve(status()),
      signIn: () => Promise.resolve(status()),
      signOut: () => Promise.resolve(status({ status: "unavailable" })),
      ...host,
    },
    setBusy: (busy) => { h.busy.push(busy); },
    setNotice: (notice) => { h.notices.push(notice); },
  };
  return h;
}

const selection = { providerId: "openai", modelId: "gpt-5" } as const;
const connection = { ...selection, apiKey: "k", endpoint: null, region: null };

describe("changing the model authority", () => {
  // Each of these changes which provider and model the host will send to, so
  // everything derived from the old authority has to go with it: an approval
  // granted against the previous model is not an approval of this one.
  it("adopts the new authority when a model is selected", async () => {
    const h = harness();
    await createModelActions(h.ports).changeModelSelection(selection);
    expect(h.adopted).toHaveLength(1);
  });

  it("adopts the new authority when a provider connects", async () => {
    const h = harness();
    await createModelActions(h.ports).connectModelProvider(connection);
    expect(h.adopted).toHaveLength(1);
  });

  it("adopts the new authority when a provider disconnects", async () => {
    const h = harness();
    await createModelActions(h.ports).disconnectModelProvider();
    expect(h.adopted).toHaveLength(1);
    expect(h.adopted[0]?.status).toBe("unavailable");
  });

  it("does not adopt an authority when the host call fails", async () => {
    const h = harness({ signIn: () => Promise.reject(new Error("no")) });
    await createModelActions(h.ports).connectModelProvider(connection);
    expect(h.adopted).toEqual([]);
  });
});

describe("notices", () => {
  it("distinguishes a model that is ready from one that still needs a key", async () => {
    // Selecting is not connecting. Without this the user picks a model and
    // waits for a reply that will never come.
    const ready = harness({ setModelSelection: () => Promise.resolve(status()) });
    await createModelActions(ready.ports).changeModelSelection(selection);
    expect(ready.notices.at(-1)).toBe("OpenAI gpt-5 is ready.");

    const needsKey = harness({
      setModelSelection: () => Promise.resolve(status({ status: "unavailable" })),
    });
    await createModelActions(needsKey.ports).changeModelSelection(selection);
    expect(needsKey.notices.at(-1)).toContain("Enter its API key");
  });

  it("says the connection lasts for the session", async () => {
    // Literal: the host holds the key in memory and it is gone when the app
    // closes.
    const h = harness();
    await createModelActions(h.ports).connectModelProvider(connection);
    expect(h.notices.at(-1)).toBe("OpenAI gpt-5 is connected for this session.");
  });

  it("says the key was cleared on disconnect", async () => {
    const h = harness();
    await createModelActions(h.ports).disconnectModelProvider();
    expect(h.notices.at(-1)).toBe("Provider disconnected. The in-memory API key was cleared.");
  });

  it("reports a host failure through the safe message", async () => {
    // Not the raw error: a provider failure can carry the endpoint and enough
    // of the request to be worth not printing.
    const h = harness({
      signIn: () => Promise.reject(new HostCommandError({
        code: "unauthorized",
        safeMessage: "That API key was rejected.",
        correlationId: "c1",
        retryable: false,
      })),
    });
    await createModelActions(h.ports).connectModelProvider(connection);
    expect(h.notices.at(-1)).toBe("That API key was rejected.");
  });
});

describe("availability", () => {
  it("explains what is needed rather than naming the missing capability", async () => {
    const cases = [
      ["canSelectModel", modelUnavailableNotices.selection,
        (a: ReturnType<typeof createModelActions>) => a.changeModelSelection(selection)],
      ["canSignIn", modelUnavailableNotices.signIn,
        (a: ReturnType<typeof createModelActions>) => a.connectModelProvider(connection)],
      ["canSignOut", modelUnavailableNotices.signOut,
        (a: ReturnType<typeof createModelActions>) => a.disconnectModelProvider()],
    ] as const;

    for (const [flag, notice, run] of cases) {
      const h = harness({ [flag]: false });
      await run(createModelActions(h.ports));
      expect(h.notices.at(-1)).toBe(notice);
      // A message the user can act on, not an error code.
      expect(notice).toMatch(/^[A-Z].*\.$/u);
    }
  });

  it("does not call the host or mark busy when a capability is missing", async () => {
    const setModelSelection = vi.fn();
    const h = harness({ canSelectModel: false, setModelSelection });
    await createModelActions(h.ports).changeModelSelection(selection);
    expect(setModelSelection).not.toHaveBeenCalled();
    expect(h.busy).toEqual([]);
  });

  it("explains rather than doing nothing when no host is bound", async () => {
    const h = harness(null);
    await createModelActions(h.ports).disconnectModelProvider();
    expect(h.notices.at(-1)).toBe(modelUnavailableNotices.signOut);
  });
});

describe("the busy flag", () => {
  it("is set then cleared on success", async () => {
    const h = harness();
    await createModelActions(h.ports).connectModelProvider(connection);
    expect(h.busy).toEqual([true, false]);
  });

  it("is cleared after a failure", async () => {
    // A busy flag left set by a thrown action is a control the user can never
    // press again.
    const h = harness({ signIn: () => Promise.reject(new Error("no")) });
    await createModelActions(h.ports).connectModelProvider(connection);
    expect(h.busy).toEqual([true, false]);
  });
});
