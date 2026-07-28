import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type {
  BootstrapReply,
  DesktopHostClient,
  HostRuntime,
  WorkspaceProjection,
} from "../lib/hostClient";
import {
  hostUnreachableMessage,
  readOnlyRecoveryNotice,
  useAppBootstrap,
  type AppBootstrapPorts,
} from "./useAppBootstrap";

function workspace(id: string): WorkspaceProjection {
  return {
    workspaceId: id,
    projectId: `project_${id}`,
    displayName: id,
    grantEpoch: 0,
    permissions: "read_only",
    contextReadEpoch: 1,
    governedEditEpoch: 1,
  };
}

function bootstrapReply(overrides: Partial<BootstrapReply> = {}): BootstrapReply {
  return {
    schemaVersion: 1,
    rendererSessionId: "renderer-1",
    installationId: "install-1",
    windowLabel: "main",
    bootMode: "ready",
    supportedCommands: ["app.get_boot_state", "workspace.list", "workspace.select_folder"],
    workspaces: [workspace("w1"), workspace("w2")],
    projectionSequence: 7,
    ...overrides,
  } as BootstrapReply;
}

/** A stand-in client. Only its identity matters to this state machine. */
function fakeClient(): DesktopHostClient {
  return {} as DesktopHostClient;
}

function readyRuntime(client = fakeClient(), bootstrap = bootstrapReply()): HostRuntime {
  return { kind: "ready", client, bootstrap };
}

interface Harness {
  ports: AppBootstrapPorts;
  adopted: { workspaces: readonly WorkspaceProjection[]; preserveActive: boolean }[];
  invalidations: number;
  notices: string[];
  /** Order of side effects, so "invalidate before publish" is checkable. */
  trace: string[];
}

function harness(
  loadRuntime: () => Promise<HostRuntime>,
  demo: readonly WorkspaceProjection[] = [workspace("demo")],
): Harness {
  const h: Harness = {
    adopted: [],
    invalidations: 0,
    notices: [],
    trace: [],
    ports: {} as AppBootstrapPorts,
  };
  h.ports = {
    loadRuntime,
    browserDemoWorkspaces: demo,
    invalidateHostBoundUi: () => {
      h.invalidations += 1;
      h.trace.push("invalidate");
    },
    adoptWorkspaces: (workspaces, preserveActive) => {
      h.adopted.push({ workspaces, preserveActive });
      h.trace.push("workspaces");
    },
    reportRecoveryNotice: (message) => {
      h.notices.push(message);
      h.trace.push("notice");
    },
  };
  return h;
}

afterEach(() => { vi.restoreAllMocks(); });

describe("loading the runtime", () => {
  it("starts in loading and adopts what the loader returns", async () => {
    const runtime = readyRuntime();
    const h = harness(() => Promise.resolve(runtime));
    const { result } = renderHook(() => useAppBootstrap(h.ports));

    expect(result.current.hostRuntime.kind).toBe("loading");
    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("ready"));
    expect(h.adopted).toEqual([
      { workspaces: runtime.bootstrap!.workspaces, preserveActive: false },
    ]);
  });

  it("falls back to unavailable when the loader rejects", async () => {
    const h = harness(() => Promise.reject(new Error("no host")));
    const { result } = renderHook(() => useAppBootstrap(h.ports));

    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("unavailable"));
    expect(result.current.hostRuntime).toMatchObject({
      client: null,
      bootstrap: null,
      message: hostUnreachableMessage,
    });
    // An unavailable host still publishes a workspace list -- an empty one.
    // Leaving the previous host's workspaces on screen under a dead binding is
    // the failure this prevents.
    expect(h.adopted).toEqual([{ workspaces: [], preserveActive: false }]);
  });

  it("publishes the demo workspaces for the browser preview", async () => {
    const demo = [workspace("browser")];
    const h = harness(
      () => Promise.resolve({ kind: "browser_demo", client: null, bootstrap: null }),
      demo,
    );
    const { result } = renderHook(() => useAppBootstrap(h.ports));

    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("browser_demo"));
    expect(h.adopted[0]?.workspaces).toBe(demo);
  });

  it("claims the new binding generation before calling any port", async () => {
    // This is the ordering that is actually observable, and the one that
    // matters. The clearers `invalidateHostBoundUi` composes start their own
    // reads; a read started while the ref still held the outgoing epoch would
    // look current and be applied on top of the new binding's data.
    //
    // Whether `invalidateHostBoundUi` runs before or after the runtime is
    // published is *not* observable -- React batches both into one render --
    // so it is not asserted here.
    const seen: number[] = [];
    const h = harness(() => Promise.resolve(readyRuntime()));
    const ports: AppBootstrapPorts = {
      ...h.ports,
      invalidateHostBoundUi: () => {
        seen.push(bootstrap.current.hostBindingGenerationRef.current);
      },
      adoptWorkspaces: () => {
        seen.push(bootstrap.current.hostBindingGenerationRef.current);
      },
    };
    const { result: bootstrap } = renderHook(() => useAppBootstrap(ports));

    await waitFor(() => expect(seen.length).toBe(2));
    expect(seen).toEqual([1, 1]);
  });

  it("does not adopt a runtime that resolves after unmount", async () => {
    let settle: ((runtime: HostRuntime) => void) | undefined;
    const h = harness(() => new Promise<HostRuntime>((resolve) => { settle = resolve; }));
    const { unmount } = renderHook(() => useAppBootstrap(h.ports));

    unmount();
    await act(async () => {
      settle?.(readyRuntime());
      await Promise.resolve();
    });

    expect(h.adopted).toEqual([]);
  });
});

describe("the binding generation", () => {
  it("advances on every transition", async () => {
    const client = fakeClient();
    const h = harness(() => Promise.resolve(readyRuntime(client)));
    const { result } = renderHook(() => useAppBootstrap(h.ports));
    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("ready"));

    const afterAdopt = result.current.hostBindingGenerationRef.current;
    expect(afterAdopt).toBe(1);

    act(() => result.current.markReadOnlyRecovery(client, 9));
    expect(result.current.hostBindingGenerationRef.current).toBe(afterAdopt + 1);

    act(() => result.current.markHostUnavailable(client));
    expect(result.current.hostBindingGenerationRef.current).toBe(afterAdopt + 2);
  });

  it("does not advance when a runtime is republished after a rebind", async () => {
    // The rebind path claims its generation before awaiting `bootstrap()`.
    // Claiming a second one here would invalidate the read that is currently
    // succeeding.
    const client = fakeClient();
    const h = harness(() => Promise.resolve(readyRuntime(client)));
    const { result } = renderHook(() => useAppBootstrap(h.ports));
    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("ready"));

    const before = result.current.hostBindingGenerationRef.current;
    act(() => result.current.republishHostRuntime({
      kind: "ready", client, bootstrap: bootstrapReply({ projectionSequence: 11 }),
    }));

    expect(result.current.hostBindingGenerationRef.current).toBe(before);
    expect(result.current.hostRuntime).toMatchObject({ kind: "ready" });
  });
});

describe("degrading a live binding", () => {
  it("keeps the client but narrows its powers in read-only recovery", async () => {
    const client = fakeClient();
    const h = harness(() => Promise.resolve(readyRuntime(client)));
    const { result } = renderHook(() => useAppBootstrap(h.ports));
    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("ready"));

    act(() => result.current.markReadOnlyRecovery(client, 9));

    const runtime = result.current.hostRuntime;
    expect(runtime.kind).toBe("read_only_recovery");
    if (runtime.kind !== "read_only_recovery") throw new Error("unreachable");
    // The two reads that let the user *see* recovery survive; everything that
    // could change state does not.
    expect(runtime.bootstrap.supportedCommands)
      .toEqual(["app.get_boot_state", "workspace.list"]);
    expect(runtime.bootstrap.bootMode).toBe("read_only_recovery");
    expect(runtime.client).toBe(client);
    expect(h.notices).toEqual([readOnlyRecoveryNotice]);
  });

  it("never rewinds the projection cursor", async () => {
    // The sequence is a high-water mark. Rewinding it would replay events the
    // renderer has already applied.
    const client = fakeClient();
    const h = harness(() =>
      Promise.resolve(readyRuntime(client, bootstrapReply({ projectionSequence: 20 }))));
    const { result } = renderHook(() => useAppBootstrap(h.ports));
    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("ready"));

    act(() => result.current.markReadOnlyRecovery(client, 5));

    const runtime = result.current.hostRuntime;
    if (runtime.kind !== "read_only_recovery") throw new Error("unreachable");
    expect(runtime.bootstrap.projectionSequence).toBe(20);
  });

  it("drops to unavailable and releases the client", async () => {
    const client = fakeClient();
    const h = harness(() => Promise.resolve(readyRuntime(client)));
    const { result } = renderHook(() => useAppBootstrap(h.ports));
    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("ready"));

    act(() => result.current.markHostUnavailable(client));

    expect(result.current.hostRuntime).toEqual({
      kind: "unavailable",
      client: null,
      bootstrap: null,
      message: hostUnreachableMessage,
    });
  });

  it("ignores a failure reported by a client that is no longer bound", async () => {
    // The guard that makes the whole design work: a slow read from a replaced
    // binding must not tear down the live one.
    const stale = fakeClient();
    const live = fakeClient();
    const h = harness(() => Promise.resolve(readyRuntime(live)));
    const { result } = renderHook(() => useAppBootstrap(h.ports));
    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("ready"));

    act(() => result.current.markHostUnavailable(stale));
    expect(result.current.hostRuntime.kind).toBe("ready");

    act(() => result.current.markReadOnlyRecovery(stale, 3));
    expect(result.current.hostRuntime.kind).toBe("ready");
    expect(h.notices).toEqual([readOnlyRecoveryNotice]);
  });

  it("can degrade a binding that is already in recovery", async () => {
    const client = fakeClient();
    const h = harness(() => Promise.resolve(readyRuntime(client)));
    const { result } = renderHook(() => useAppBootstrap(h.ports));
    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("ready"));

    act(() => result.current.markReadOnlyRecovery(client, 9));
    act(() => result.current.markHostUnavailable(client));

    expect(result.current.hostRuntime.kind).toBe("unavailable");
  });
});

describe("retrying", () => {
  it("re-runs the loader and can reach a host that was unavailable", async () => {
    // `unavailable` was terminal for the life of the window: the default loader
    // memoises its promise, so a host that failed to verify once could never be
    // retried.
    const runtime = readyRuntime();
    const loadRuntime = vi.fn<() => Promise<HostRuntime>>()
      .mockRejectedValueOnce(new Error("not yet"))
      .mockResolvedValueOnce(runtime);
    const h = harness(loadRuntime);
    const { result } = renderHook(() => useAppBootstrap(h.ports));

    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("unavailable"));

    act(() => result.current.retryHostRuntime());
    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("ready"));
    expect(loadRuntime).toHaveBeenCalledTimes(2);
  });

  it("shows loading again while the retry is in flight", async () => {
    let settle: ((runtime: HostRuntime) => void) | undefined;
    const loadRuntime = vi.fn<() => Promise<HostRuntime>>()
      .mockRejectedValueOnce(new Error("not yet"))
      .mockImplementationOnce(() => new Promise((resolve) => { settle = resolve; }));
    const h = harness(loadRuntime);
    const { result } = renderHook(() => useAppBootstrap(h.ports));
    await waitFor(() => expect(result.current.hostRuntime.kind).toBe("unavailable"));

    act(() => result.current.retryHostRuntime());
    expect(result.current.hostRuntime.kind).toBe("loading");

    await act(async () => {
      settle?.(readyRuntime());
      await Promise.resolve();
    });
    expect(result.current.hostRuntime.kind).toBe("ready");
  });
});
