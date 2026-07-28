import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { useHostProjection } from "./hostProjection";

describe("reading a projection", () => {
  it("starts unavailable with nothing to show", () => {
    const { result } = renderHook(() => useHostProjection<string>());
    expect(result.current.status).toBe("unavailable");
    expect(result.current.projection).toBeNull();
  });

  it("marks loading, then publishes the result", async () => {
    // The status has to be observed between renders, not inside the load
    // closure: `result.current` there is still the pre-render value, because
    // React has not flushed the `setStatus("loading")` yet.
    const { result } = renderHook(() => useHostProjection<string>());
    let release: ((value: string) => void) | undefined;
    const pending = new Promise<string>((resolve) => { release = resolve; });

    let reading: Promise<void> | undefined;
    act(() => { reading = result.current.read({ generation: () => 1, load: () => pending }); });
    expect(result.current.status).toBe("loading");

    await act(async () => {
      release?.("v1.2.3");
      await reading;
    });
    expect(result.current.status).toBe("ready");
    expect(result.current.projection).toBe("v1.2.3");
  });

  it("reports a failed read as unavailable", async () => {
    const { result } = renderHook(() => useHostProjection<string>());
    await act(async () => {
      await result.current.read({
        generation: () => 1,
        load: () => Promise.reject(new Error("host gone")),
      });
    });
    expect(result.current.status).toBe("unavailable");
    expect(result.current.projection).toBeNull();
  });

  it("does not propagate the read failure to the caller", async () => {
    const { result } = renderHook(() => useHostProjection<string>());
    await act(async () => {
      await expect(result.current.read({
        generation: () => 1,
        load: () => Promise.reject(new Error("host gone")),
      })).resolves.toBeUndefined();
    });
  });
});

describe("the staleness guard", () => {
  /// The reason this hook exists. These reads outlive the binding they were
  /// started against, and a projection from a replaced host describes a machine
  /// the app is no longer talking to. For the retention manifest that means
  /// telling the user their data is somewhere it is not.
  it("discards a result from a binding that has been replaced", async () => {
    let generation = 1;
    const { result } = renderHook(() => useHostProjection<string>());
    await act(async () => {
      await result.current.read({
        generation: () => generation,
        load: async () => {
          generation = 2; // the host rebinds while the read is in flight
          return "from the old host";
        },
      });
    });
    expect(result.current.projection).toBeNull();
    expect(result.current.status).toBe("loading");
  });

  it("discards a failure from a binding that has been replaced", async () => {
    // Otherwise a read for the previous binding that rejects after the new one
    // is live marks the *new* projection unavailable, and nothing retries it.
    let generation = 1;
    const { result } = renderHook(() => useHostProjection<string>());
    await act(async () => {
      await result.current.read({ generation: () => 1, load: async () => "fresh" });
    });
    expect(result.current.status).toBe("ready");

    await act(async () => {
      await result.current.read({
        generation: () => generation,
        load: () => {
          generation = 2;
          return Promise.reject(new Error("old host"));
        },
      });
    });
    expect(result.current.projection).toBe("fresh");
    // The status is what the unguarded version gets wrong. It stays where the
    // new binding's read will set it -- not `unavailable`, which would report
    // the *new* host as unreadable because the *old* one failed, and nothing
    // would retry it.
    expect(result.current.status).not.toBe("unavailable");
  });

  it("captures the generation before awaiting, not after", async () => {
    // Reading it after the await compares a value to itself and never detects
    // anything -- which is why the capture is inside the hook rather than left
    // to each caller.
    const generation = vi.fn(() => 1);
    const { result } = renderHook(() => useHostProjection<string>());
    await act(async () => {
      await result.current.read({
        generation,
        load: async () => {
          expect(generation).toHaveBeenCalledTimes(1);
          return "v1";
        },
      });
    });
    expect(generation).toHaveBeenCalledTimes(2);
  });

  it("keeps a result whose binding never changed", async () => {
    const { result } = renderHook(() => useHostProjection<string>());
    await act(async () => {
      await result.current.read({ generation: () => 7, load: async () => "stable" });
    });
    expect(result.current.projection).toBe("stable");
    expect(result.current.status).toBe("ready");
  });
});

describe("statuses the hook does not model", () => {
  it("accepts a caller-supplied terminal status", async () => {
    // Offboarding needs "erasing" and "erased": the store is destroyed, so
    // "ready" and "unavailable" both describe it wrongly.
    const { result } = renderHook(() => useHostProjection<string, "erasing" | "erased">());
    act(() => result.current.setStatus("erasing"));
    expect(result.current.status).toBe("erasing");
    act(() => result.current.set(null, "erased"));
    expect(result.current.status).toBe("erased");
    expect(result.current.projection).toBeNull();
  });

  it("clears the projection and the status together", async () => {
    const { result } = renderHook(() => useHostProjection<string>());
    await act(async () => {
      await result.current.read({ generation: () => 1, load: async () => "v1" });
    });
    act(() => result.current.set(null, "unavailable"));
    expect(result.current.projection).toBeNull();
    expect(result.current.status).toBe("unavailable");
  });
});
