import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { HostCommandError, type WorkspaceProjection } from "../../lib/hostClient";
import {
  createWorkspaceActions,
  isRecoveryFailure,
  useWorkspaceState,
  type WorkspaceActionPorts,
  type WorkspaceHostPort,
  type WorkspaceState,
} from "./workspaceController";

function workspace(id: string, overrides: Partial<WorkspaceProjection> = {}): WorkspaceProjection {
  return {
    workspaceId: id,
    projectId: `project_${id}`,
    displayName: id,
    grantEpoch: 0,
    permissions: "read_only",
    contextReadEpoch: 1,
    governedEditEpoch: 1,
    ...overrides,
  };
}

type HostErrorDetails = ConstructorParameters<typeof HostCommandError>[0];

/** A real host error, not a cast -- the classifier reads `details.code`. */
function hostError(code: HostErrorDetails["code"]): HostCommandError {
  return new HostCommandError({
    code,
    safeMessage: `Something went wrong (${code}).`,
    correlationId: "test-correlation",
    retryable: false,
  });
}

interface Harness {
  state: WorkspaceState;
  host: WorkspaceHostPort;
  recoveries: number;
  changes: number;
  rerender: () => void;
}

function harness(hostOverrides: Partial<WorkspaceHostPort> = {}): Harness {
  const h = { recoveries: 0, changes: 0 } as Harness;
  const { rerender, result } = renderHook(() => useWorkspaceState());
  h.host = {
    canActivate: true,
    canRemove: true,
    canSelect: true,
    enterReadOnlyRecovery: () => { h.recoveries += 1; },
    revokeWorkspace: () => Promise.resolve({ workspaces: [] }),
    selectWorkspace: () => Promise.resolve({ kind: "no_selection" }),
    ...hostOverrides,
  };
  Object.defineProperty(h, "state", { get: () => result.current });
  h.rerender = rerender;
  return h;
}

function actions(h: Harness, ports: Partial<WorkspaceActionPorts> = {}) {
  return createWorkspaceActions(h.state, {
    host: h.host,
    onActiveWorkspaceChanged: () => { h.changes += 1; },
    ...ports,
  });
}

describe("isRecoveryFailure", () => {
  it("recognises the two codes that mean the host distrusts its own state", () => {
    expect(isRecoveryFailure(hostError("recovery_required"))).toBe(true);
    expect(isRecoveryFailure(hostError("integrity_failure"))).toBe(true);
  });

  it("treats anything else as an ordinary failure", () => {
    expect(isRecoveryFailure(hostError("not_found"))).toBe(false);
    expect(isRecoveryFailure(new Error("network"))).toBe(false);
    expect(isRecoveryFailure(undefined)).toBe(false);
  });
});

describe("adopting a workspace list", () => {
  it("selects the first workspace when nothing is preserved", () => {
    const h = harness();
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], false));
    expect(h.state.activeWorkspaceId).toBe("a");
    expect(h.state.activeWorkspace?.workspaceId).toBe("a");
  });

  it("keeps the active workspace when it survives the new list", () => {
    const h = harness();
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], false));
    act(() => h.state.setActiveWorkspaceId("b"));
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], true));
    expect(h.state.activeWorkspaceId).toBe("b");
  });

  it("falls back to the first when the active workspace is gone", () => {
    // A grant revoked by the host, or a folder that moved. Staying pointed at
    // an id the host no longer projects would make every action fail.
    const h = harness();
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], false));
    act(() => h.state.setActiveWorkspaceId("b"));
    act(() => h.state.adoptWorkspaces([workspace("a")], true));
    expect(h.state.activeWorkspaceId).toBe("a");
  });

  it("clears the selection for an empty list", () => {
    const h = harness();
    act(() => h.state.adoptWorkspaces([workspace("a")], false));
    act(() => h.state.adoptWorkspaces([], false));
    expect(h.state.activeWorkspaceId).toBeNull();
    expect(h.state.activeWorkspace).toBeNull();
  });

  it("falls back to the first workspace when the selection names none of them", () => {
    const h = harness();
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], false));
    act(() => h.state.setActiveWorkspaceId("gone"));
    expect(h.state.activeWorkspace?.workspaceId).toBe("a");
  });
});

describe("promoteWorkspace", () => {
  it("moves the workspace to the front and makes it active", () => {
    const h = harness();
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], false));
    act(() => h.state.promoteWorkspace(workspace("b")));
    expect(h.state.workspaces.map((w) => w.workspaceId)).toEqual(["b", "a"]);
    expect(h.state.activeWorkspaceId).toBe("b");
  });

  it("replaces the existing entry rather than duplicating it", () => {
    // The incoming projection carries the newer grant epoch and permissions --
    // this is how enabling governed edits publishes the re-granted workspace.
    const h = harness();
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], false));
    act(() => h.state.promoteWorkspace(
      workspace("a", { permissions: "governed_edits", grantEpoch: 2 }),
    ));
    expect(h.state.workspaces).toHaveLength(2);
    expect(h.state.workspaces[0]?.permissions).toBe("governed_edits");
    expect(h.state.workspaces[0]?.grantEpoch).toBe(2);
  });
});

describe("selectWorkspace", () => {
  it("promotes the picked folder", async () => {
    const h = harness({
      selectWorkspace: () => Promise.resolve({
        kind: "workspace_selected", value: workspace("picked"),
      }),
    });
    act(() => h.state.adoptWorkspaces([workspace("a")], false));
    await act(async () => { await actions(h).selectWorkspace(); });
    expect(h.state.workspaces.map((w) => w.workspaceId)).toEqual(["picked", "a"]);
    expect(h.state.activeWorkspaceId).toBe("picked");
  });

  it("treats a cancelled folder dialog as neither a change nor a failure", () => {
    const h = harness();
    return act(async () => {
      await actions(h).selectWorkspace();
      expect(h.state.workspaces).toEqual([]);
      expect(h.state.actionError).toBeNull();
    });
  });

  it("does nothing without the capability", async () => {
    const selectWorkspace = vi.fn();
    const h = harness({ canSelect: false, selectWorkspace });
    await act(async () => { await actions(h).selectWorkspace(); });
    expect(selectWorkspace).not.toHaveBeenCalled();
  });

  it("does nothing with no host bound", async () => {
    const h = harness();
    await act(async () => { await actions(h, { host: null }).selectWorkspace(); });
    expect(h.state.actionError).toBeNull();
  });
});

describe("failure classification", () => {
  // The single reason this controller exists. Each action repeated this by
  // hand; missing the branch in one of them reported a host demanding recovery
  // as an ordinary error while the app carried on believing it was healthy.
  for (const code of ["recovery_required", "integrity_failure"] as const) {
    it(`enters read-only recovery for ${code} from selectWorkspace`, async () => {
      const h = harness({ selectWorkspace: () => Promise.reject(hostError(code)) });
      await act(async () => { await actions(h).selectWorkspace(); });
      expect(h.recoveries).toBe(1);
      expect(h.state.actionError).toBeNull();
    });

    it(`enters read-only recovery for ${code} from removeWorkspace`, async () => {
      const h = harness({ revokeWorkspace: () => Promise.reject(hostError(code)) });
      act(() => h.state.adoptWorkspaces([workspace("a")], false));
      await act(async () => { await actions(h).removeWorkspace("a"); });
      expect(h.recoveries).toBe(1);
      expect(h.state.actionError).toBeNull();
    });
  }

  it("shows an ordinary failure to the user instead", async () => {
    const h = harness({ selectWorkspace: () => Promise.reject(hostError("unauthorized")) });
    await act(async () => { await actions(h).selectWorkspace(); });
    expect(h.recoveries).toBe(0);
    expect(h.state.actionError).not.toBeNull();
  });

  it("clears the busy flag after a failure", async () => {
    // A busy flag left set by a thrown action is a control the user can never
    // press again.
    const h = harness({ selectWorkspace: () => Promise.reject(hostError("unauthorized")) });
    await act(async () => { await actions(h).selectWorkspace(); });
    expect(h.state.selectionBusy).toBe(false);

    const recovered = harness({
      selectWorkspace: () => Promise.resolve({
        kind: "workspace_selected", value: workspace("later"),
      }),
    });
    await act(async () => { await actions(recovered).selectWorkspace(); });
    expect(recovered.state.activeWorkspaceId).toBe("later");
  });
});

describe("removeWorkspace", () => {
  it("keeps the active workspace when a different one is revoked", async () => {
    const h = harness({
      revokeWorkspace: () => Promise.resolve({ workspaces: [workspace("b")] }),
    });
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], false));
    act(() => h.state.setActiveWorkspaceId("b"));
    await act(async () => { await actions(h).removeWorkspace("a"); });
    expect(h.state.activeWorkspaceId).toBe("b");
  });

  it("moves to the first remaining workspace when the active one is revoked", async () => {
    const h = harness({
      revokeWorkspace: () => Promise.resolve({ workspaces: [workspace("b")] }),
    });
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], false));
    await act(async () => { await actions(h).removeWorkspace("a"); });
    expect(h.state.activeWorkspaceId).toBe("b");
  });

  it("clears the selection when the last workspace is revoked", async () => {
    const h = harness({ revokeWorkspace: () => Promise.resolve({ workspaces: [] }) });
    act(() => h.state.adoptWorkspaces([workspace("a")], false));
    await act(async () => { await actions(h).removeWorkspace("a"); });
    expect(h.state.activeWorkspaceId).toBeNull();
  });

  it("revokes the projection the renderer is looking at, not just an id", async () => {
    // The host checks the grant epoch it is handed. Sending a bare id would let
    // a stale renderer revoke a grant that has since been re-issued.
    const revokeWorkspace = vi.fn(() => Promise.resolve({ workspaces: [] }));
    const h = harness({ revokeWorkspace });
    const projection = workspace("a", { grantEpoch: 4 });
    act(() => h.state.adoptWorkspaces([projection], false));
    await act(async () => { await actions(h).removeWorkspace("a"); });
    expect(revokeWorkspace).toHaveBeenCalledWith(projection);
  });

  it("ignores a workspace the host has not projected", async () => {
    const revokeWorkspace = vi.fn(() => Promise.resolve({ workspaces: [] }));
    const h = harness({ revokeWorkspace });
    await act(async () => { await actions(h).removeWorkspace("never-seen"); });
    expect(revokeWorkspace).not.toHaveBeenCalled();
  });

  it("clears the removal busy id when the revoke fails", async () => {
    const h = harness({ revokeWorkspace: () => Promise.reject(hostError("unauthorized")) });
    act(() => h.state.adoptWorkspaces([workspace("a")], false));
    await act(async () => { await actions(h).removeWorkspace("a"); });
    expect(h.state.removalBusyId).toBeNull();
  });
});

describe("activateWorkspace", () => {
  it("switches the active workspace and reports the change", () => {
    const h = harness();
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], false));
    act(() => actions(h).activateWorkspace("b"));
    expect(h.state.activeWorkspaceId).toBe("b");
    // Changes flows, history and capability runs are all workspace-scoped.
    expect(h.changes).toBe(1);
  });

  it("does not report a change when the same workspace is re-selected", () => {
    // Re-selecting would otherwise discard the review the user is looking at.
    const h = harness();
    act(() => h.state.adoptWorkspaces([workspace("a")], false));
    act(() => actions(h).activateWorkspace("a"));
    expect(h.changes).toBe(0);
  });

  it("refuses a workspace the host has not projected", () => {
    // Pointing the session at an id no grant covers.
    const h = harness();
    act(() => h.state.adoptWorkspaces([workspace("a")], false));
    act(() => actions(h).activateWorkspace("smuggled"));
    expect(h.state.activeWorkspaceId).toBe("a");
    expect(h.changes).toBe(0);
  });

  it("does nothing without the capability", () => {
    const h = harness({ canActivate: false });
    act(() => h.state.adoptWorkspaces([workspace("a"), workspace("b")], false));
    act(() => actions(h).activateWorkspace("b"));
    expect(h.state.activeWorkspaceId).toBe("a");
  });
});

describe("resetActionState", () => {
  it("clears the busy flags and the error when a host binding is dropped", async () => {
    const h = harness({ selectWorkspace: () => Promise.reject(hostError("unauthorized")) });
    await act(async () => { await actions(h).selectWorkspace(); });
    expect(h.state.actionError).not.toBeNull();

    act(() => h.state.resetActionState());
    expect(h.state.actionError).toBeNull();
    expect(h.state.selectionBusy).toBe(false);
    expect(h.state.removalBusyId).toBeNull();
  });
});
