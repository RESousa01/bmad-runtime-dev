/**
 * The workspace feature controller.
 *
 * FE-10 step 3. Three mutating actions -- pick a folder, activate one, revoke
 * one -- each repeated the same six-step shape by hand: guard on capability and
 * on the busy flag, clear the error, call the host, classify the failure, clear
 * the busy flag. The classification was the part that mattered and the part
 * most easily got wrong: a host answering `recovery_required` or
 * `integrity_failure` must put the app into read-only recovery, and anything
 * else is an ordinary message for the user. Miss that branch in one action and
 * a host demanding recovery is reported as a generic error while the app
 * carries on believing it is healthy.
 *
 * `runWorkspaceAction` below is that shape, written once.
 *
 * Split in two on purpose
 * -----------------------
 * `useWorkspaceState` owns the list and the selection and depends on nothing;
 * `createWorkspaceActions` needs a bound host and the recovery transition. The
 * bootstrap hook sits between them -- it publishes the workspace list a new
 * binding brings with it, and it provides the recovery transition -- so the
 * state has to be declared before it and the actions after. Two pieces makes
 * that ordering explicit instead of forcing a ref indirection.
 */

import { useCallback, useRef, useState } from "react";

import {
  getSafeHostMessage,
  HostCommandError,
  type WorkspaceProjection,
  type WorkspaceSelection,
} from "../../lib/hostClient";

export interface WorkspaceState {
  readonly workspaces: readonly WorkspaceProjection[];
  readonly activeWorkspaceId: string | null;
  /** The workspace in force: the selected one, or the first as a fallback. */
  readonly activeWorkspace: WorkspaceProjection | null;
  readonly selectionBusy: boolean;
  readonly removalBusyId: string | null;
  readonly actionError: string | null;
  readonly setActionError: (message: string | null) => void;
  readonly setActiveWorkspaceId: (id: string | null) => void;
  /** Publishes the list a newly adopted host binding brought with it. */
  readonly adoptWorkspaces: (
    workspaces: readonly WorkspaceProjection[],
    preserveActive: boolean,
  ) => void;
  /**
   * Moves a workspace to the front of the list and makes it active.
   *
   * Front, not appended: the folder the user just picked -- or just granted
   * edits on -- is the one they are about to work in. Replaces any existing
   * entry for the same id, because the incoming projection carries the newer
   * grant epoch and permissions.
   */
  readonly promoteWorkspace: (workspace: WorkspaceProjection) => void;
  /** Clears the busy flags and the error. Part of dropping a host binding. */
  readonly resetActionState: () => void;
  /** @internal Used by the actions built from this state. */
  readonly internals: WorkspaceStateInternals;
}

interface WorkspaceStateInternals {
  readonly busyRef: { current: boolean };
  readonly setWorkspaces: (
    update: readonly WorkspaceProjection[]
      | ((current: readonly WorkspaceProjection[]) => readonly WorkspaceProjection[]),
  ) => void;
  readonly setActiveWorkspaceIdWith: (
    update: (current: string | null) => string | null,
  ) => void;
  readonly setSelectionBusy: (busy: boolean) => void;
  readonly setRemovalBusyId: (id: string | null) => void;
}

export function useWorkspaceState(): WorkspaceState {
  const [workspaces, setWorkspaces] = useState<readonly WorkspaceProjection[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string | null>(null);
  const [selectionBusy, setSelectionBusy] = useState(false);
  const [removalBusyId, setRemovalBusyId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  /*
   * A ref rather than derived from the two busy flags, because it is read
   * synchronously at the top of an action to reject a second one. State read
   * during the same tick would still be the pre-click value, so two fast
   * clicks would both pass the guard.
   */
  const busyRef = useRef(false);

  const adoptWorkspaces = useCallback((
    next: readonly WorkspaceProjection[],
    preserveActive: boolean,
  ) => {
    setWorkspaces([...next]);
    setActiveWorkspaceId((current) => (
      preserveActive
      && current !== null
      && next.some(({ workspaceId }) => workspaceId === current)
        ? current
        : next[0]?.workspaceId ?? null
    ));
  }, []);

  const promoteWorkspace = useCallback((workspace: WorkspaceProjection) => {
    setWorkspaces((current) => [
      workspace,
      ...current.filter(({ workspaceId }) => workspaceId !== workspace.workspaceId),
    ]);
    setActiveWorkspaceId(workspace.workspaceId);
  }, []);

  const resetActionState = useCallback(() => {
    busyRef.current = false;
    setSelectionBusy(false);
    setRemovalBusyId(null);
    setActionError(null);
  }, []);

  const activeWorkspace = workspaces.find(
    ({ workspaceId }) => workspaceId === activeWorkspaceId,
  ) ?? workspaces[0] ?? null;

  return {
    actionError,
    activeWorkspace,
    activeWorkspaceId,
    adoptWorkspaces,
    promoteWorkspace,
    resetActionState,
    internals: {
      busyRef,
      setActiveWorkspaceIdWith: setActiveWorkspaceId,
      setRemovalBusyId,
      setSelectionBusy,
      setWorkspaces,
    },
    removalBusyId,
    selectionBusy,
    setActionError,
    setActiveWorkspaceId,
    workspaces,
  };
}

/**
 * What the actions need from the host.
 *
 * Narrower than the host client on purpose: this is the whole authority the
 * workspace feature holds, and it is three methods and three capability flags.
 * Handing the controller the full client would let a later change reach for
 * anything and would make that reach invisible in review.
 */
export interface WorkspaceHostPort {
  readonly canSelect: boolean;
  readonly canActivate: boolean;
  readonly canRemove: boolean;
  readonly selectWorkspace: () => Promise<WorkspaceSelection>;
  readonly revokeWorkspace: (
    expected: WorkspaceProjection,
  ) => Promise<{ readonly workspaces: readonly WorkspaceProjection[] }>;
  /** Puts the app into read-only recovery. Called for exactly two host codes. */
  readonly enterReadOnlyRecovery: () => void;
}

export interface WorkspaceActionPorts {
  /** Null whenever no live, ready host is bound -- every action then no-ops. */
  readonly host: WorkspaceHostPort | null;
  /** Runs when the active workspace actually changes, not on a re-selection. */
  readonly onActiveWorkspaceChanged: () => void;
}

export interface WorkspaceActions {
  readonly selectWorkspace: () => Promise<void>;
  readonly activateWorkspace: (workspaceId: string) => void;
  readonly removeWorkspace: (workspaceId: string) => Promise<void>;
}

/**
 * Classifies a host failure. The single reason this module exists.
 *
 * `recovery_required` and `integrity_failure` are the host saying its own state
 * is not trustworthy. Showing those as an ordinary error leaves the app running
 * against a host that has told it to stop.
 */
export function isRecoveryFailure(error: unknown): boolean {
  return error instanceof HostCommandError
    && (error.details.code === "recovery_required"
      || error.details.code === "integrity_failure");
}

export function createWorkspaceActions(
  state: WorkspaceState,
  ports: WorkspaceActionPorts,
): WorkspaceActions {
  const { busyRef, setActiveWorkspaceIdWith, setRemovalBusyId, setSelectionBusy, setWorkspaces } =
    state.internals;
  const { host, onActiveWorkspaceChanged } = ports;

  /** The shape every mutating workspace action shares. */
  async function runWorkspaceAction(
    allowed: boolean,
    setBusy: (busy: boolean) => void,
    action: (host: WorkspaceHostPort) => Promise<void>,
  ): Promise<void> {
    if (host === null || !allowed || busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    state.setActionError(null);
    try {
      await action(host);
    } catch (error) {
      if (isRecoveryFailure(error)) host.enterReadOnlyRecovery();
      else state.setActionError(getSafeHostMessage(error));
    } finally {
      // Always paired with the guard above. A busy flag left set by a thrown
      // action is a control the user can never press again.
      busyRef.current = false;
      setBusy(false);
    }
  }

  return {
    activateWorkspace(workspaceId: string) {
      if (
        host === null
        || !host.canActivate
        || busyRef.current
        // Activating a workspace the host has not projected would point the
        // whole session at an id no grant covers.
        || !state.workspaces.some((workspace) => workspace.workspaceId === workspaceId)
      ) {
        return;
      }
      state.setActionError(null);
      if (workspaceId !== state.activeWorkspaceId) {
        onActiveWorkspaceChanged();
      }
      state.setActiveWorkspaceId(workspaceId);
    },

    async removeWorkspace(workspaceId: string) {
      // Resolved before the guard so the host is asked to revoke the exact
      // projection this renderer is looking at, not an id it might have
      // re-issued for something else.
      const expected = state.workspaces.find(
        (workspace) => workspace.workspaceId === workspaceId,
      );
      if (expected === undefined) return;
      await runWorkspaceAction(
        host?.canRemove ?? false,
        (busy) => setRemovalBusyId(busy ? workspaceId : null),
        async (bound) => {
          const result = await bound.revokeWorkspace(expected);
          setWorkspaces([...result.workspaces]);
          setActiveWorkspaceIdWith((current) => {
            // The removed one cannot stay active; anything else the host still
            // projects can.
            if (
              current !== workspaceId
              && current !== null
              && result.workspaces.some((workspace) => workspace.workspaceId === current)
            ) {
              return current;
            }
            return result.workspaces[0]?.workspaceId ?? null;
          });
        },
      );
    },

    async selectWorkspace() {
      await runWorkspaceAction(
        host?.canSelect ?? false,
        setSelectionBusy,
        async (bound) => {
          const selection = await bound.selectWorkspace();
          // "No selection" is the user cancelling the folder dialog, which is
          // not a failure and must not surface as one.
          if (selection.kind !== "workspace_selected") return;
          state.promoteWorkspace(selection.value);
        },
      );
    },
  };
}
