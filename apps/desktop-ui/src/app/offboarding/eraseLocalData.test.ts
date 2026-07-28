import { describe, expect, it, vi } from "vitest";

import {
  eraseLocalData,
  eraseUnavailableNotice,
  localDataDiscards,
  type EraseLocalDataPorts,
} from "./eraseLocalData";

interface Harness {
  ports: EraseLocalDataPorts;
  statuses: string[];
  notices: (string | null)[];
  discards: number;
  order: string[];
}

function harness(overrides: Partial<EraseLocalDataPorts> = {}): Harness {
  const h: Harness = {
    discards: 0,
    notices: [],
    order: [],
    ports: {} as EraseLocalDataPorts,
    statuses: [],
  };
  h.ports = {
    discardEverything: () => { h.discards += 1; h.order.push("discard"); },
    erase: async () => { h.order.push("erase"); },
    safeMessage: () => "The host refused.",
    setNotice: (notice) => { h.notices.push(notice); },
    setStatus: (status) => { h.statuses.push(status); h.order.push(`status:${status}`); },
    ...overrides,
  };
  return h;
}

describe("erasing", () => {
  it("passes the confirmation through to the host", async () => {
    const erase = vi.fn(async () => undefined);
    const h = harness({ erase });
    await eraseLocalData("ERASE", h.ports);
    expect(erase).toHaveBeenCalledWith("ERASE");
  });

  it("shows the erasing state and then the erased one", async () => {
    const h = harness();
    await eraseLocalData("ERASE", h.ports);
    expect(h.statuses).toEqual(["erasing", "erased"]);
  });

  it("marks the status erased before discarding", async () => {
    // `erased` is terminal and stops the retention manifest being re-read,
    // which would replace the confirmation with "unavailable".
    const h = harness();
    await eraseLocalData("ERASE", h.ports);
    expect(h.order.indexOf("status:erased")).toBeLessThan(h.order.indexOf("discard"));
  });

  it("discards the renderer's copy of what the host destroyed", async () => {
    const h = harness();
    await eraseLocalData("ERASE", h.ports);
    expect(h.discards).toBe(1);
  });
});

describe("when the erase fails", () => {
  /// The one lie this feature must never tell. An erase that failed destroyed
  /// nothing, and clearing the screen would say the data is gone when it is
  /// still on disk.
  it("does not discard anything", async () => {
    const h = harness({ erase: () => Promise.reject(new Error("refused")) });
    await eraseLocalData("ERASE", h.ports);
    expect(h.discards).toBe(0);
  });

  it("returns to ready rather than unavailable", async () => {
    // The host answered; it just refused. "Unavailable" would say the command
    // is not there at all.
    const h = harness({ erase: () => Promise.reject(new Error("refused")) });
    await eraseLocalData("ERASE", h.ports);
    expect(h.statuses.at(-1)).toBe("ready");
  });

  it("explains through the safe message", async () => {
    const h = harness({ erase: () => Promise.reject(new Error("raw internals")) });
    await eraseLocalData("ERASE", h.ports);
    expect(h.notices.at(-1)).toBe("The host refused.");
  });

  it("does not propagate the failure to the caller", async () => {
    const h = harness({ erase: () => Promise.reject(new Error("refused")) });
    await expect(eraseLocalData("ERASE", h.ports)).resolves.toBeUndefined();
  });
});

describe("when the host cannot erase", () => {
  it("says so and touches nothing", async () => {
    const h = harness({ erase: null });
    await eraseLocalData("ERASE", h.ports);
    expect(h.notices).toEqual([eraseUnavailableNotice]);
    expect(h.statuses).toEqual([]);
    expect(h.discards).toBe(0);
  });
});

describe("the discard checklist", () => {
  /// The list exists so a reviewer can compare it against what the host
  /// actually erases. An earlier version covered only the workspace-bound set
  /// and claimed that was the same thing; it was not, and Home went on
  /// listing folders as granted that had just been revoked.
  it("names the things the workspace reset does not reach", () => {
    const named = localDataDiscards.map(({ what }) => what).join(" | ");
    for (const item of ["tasks", "workspace grants", "model authentication"]) {
      expect(named, item).toContain(item);
    }
  });

  it("gives every entry a reason, not just a name", () => {
    for (const { what, why } of localDataDiscards) {
      expect(why.length, what).toBeGreaterThan(20);
      expect(why, what).toMatch(/^[A-Z].*\.$/su);
    }
  });

  it("lists nothing twice", () => {
    const names = localDataDiscards.map(({ what }) => what);
    expect(new Set(names).size).toBe(names.length);
  });
});
