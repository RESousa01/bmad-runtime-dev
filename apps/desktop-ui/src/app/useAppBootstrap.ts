/**
 * The host runtime lifecycle, lifted out of the composition root.
 *
 * FE-10 step 1. This is the state machine that decides whether Forge is
 * talking to a host at all: it loads the runtime once, adopts whatever comes
 * back, and owns the two ways a live binding degrades -- into read-only
 * recovery, or into unavailable. It was ~120 lines interleaved with forty other
 * pieces of App state, which made the one question worth asking about it --
 * "can a stale binding still write to the screen?" -- impossible to answer by
 * reading.
 *
 * The binding generation is the answer, and it is why this is a hook rather
 * than a reducer. Every long-lived async read in the app captures
 * `hostBindingGeneration` before it starts and compares it after it resolves;
 * a transition that rebinds the host bumps the counter, so every in-flight read
 * from the previous binding lands stale and is discarded. Losing that ordering
 * is how a dead host's workspace list overwrites a live one's.
 *
 * What this hook deliberately does *not* own
 * ------------------------------------------
 * The workspace list. Eight other places write it -- enabling edits, picking a
 * folder, removing one, the projection poller -- so owning it here would mean
 * handing the setters back out and calling that encapsulation. It is published
 * through the `adoptWorkspaces` port instead, which keeps the direction of
 * authority visible: bootstrap tells the workspace layer what the new binding
 * brought with it, and never reads it back.
 */

import { useCallback, useEffect, useRef, useState, type RefObject } from "react";

import type { HostRuntime, WorkspaceProjection } from "../lib/hostClient";
import { resetDefaultHostRuntime } from "../lib/hostClient";

/** The runtime, plus the state before the first load has settled. */
export type HostUiRuntime = HostRuntime | { kind: "loading" };

/** A runtime kind that has a live client behind it. */
type BoundRuntime = Extract<HostRuntime, { kind: "ready" | "read_only_recovery" }>;
export type BoundHostClient = BoundRuntime["client"];

const HOST_UNREACHABLE_MESSAGE =
  "Forge could not reach the Windows host. Local actions remain unavailable.";

const RECOVERY_NOTICE =
  "Forge entered read-only recovery. Workspace access was not changed.";

/** The workspace the browser preview stands in for a real one with. */
export interface AppBootstrapPorts {
  /** Resolves the host runtime. Injected so tests can supply one. */
  loadRuntime: () => Promise<HostRuntime>;
  /**
   * Discards everything scoped to the previous host binding.
   *
   * Always called with the new binding generation already claimed. That is the
   * ordering that matters: the clearers this composes start their own reads,
   * and a read started under the outgoing epoch would look current and be
   * applied on top of the new binding's data. (Whether it runs before or after
   * `setHostRuntime` is not observable -- React batches both into one render.)
   */
  invalidateHostBoundUi: () => void;
  /** Publishes the workspace list a newly adopted runtime brought with it. */
  adoptWorkspaces: (
    workspaces: readonly WorkspaceProjection[],
    preserveActive: boolean,
  ) => void;
  /** Surfaces the user-facing notice when a binding drops to read-only recovery. */
  reportRecoveryNotice: (message: string) => void;
  /** Workspaces to publish when the runtime is the browser preview. */
  browserDemoWorkspaces: readonly WorkspaceProjection[];
}

export interface AppBootstrap {
  readonly hostRuntime: HostUiRuntime;
  /**
   * Which host binding is current. Async work captures it before starting and
   * compares it after resolving; a mismatch means the result belongs to a host
   * that has since been replaced, and must not be applied.
   */
  readonly hostBindingGenerationRef: RefObject<number>;
  /** Publishes a freshly loaded runtime and everything it brought with it. */
  readonly adoptHostRuntime: (
    runtime: HostRuntime,
    preserveActiveWorkspace: boolean,
  ) => void;
  /** The host asked for recovery, or failed integrity. Keeps the client, drops its powers. */
  readonly markReadOnlyRecovery: (client: BoundHostClient, sequence: number) => void;
  /** The host stopped answering. */
  readonly markHostUnavailable: (client: BoundHostClient) => void;
  /** Re-runs the loader. The way back out of `unavailable`. */
  readonly retryHostRuntime: () => void;
  /**
   * Re-publishes a runtime after the caller has already bumped the binding
   * generation itself -- the renderer-session rebind path, which has to claim
   * the new generation before it awaits `bootstrap()`.
   */
  readonly republishHostRuntime: (runtime: BoundRuntime) => void;
}

export function useAppBootstrap(ports: AppBootstrapPorts): AppBootstrap {
  const {
    adoptWorkspaces,
    browserDemoWorkspaces,
    invalidateHostBoundUi,
    loadRuntime,
    reportRecoveryNotice,
  } = ports;

  const [hostRuntime, setHostRuntime] = useState<HostUiRuntime>({ kind: "loading" });
  const [attempt, setAttempt] = useState(0);
  const hostBindingGenerationRef = useRef(0);

  const adoptHostRuntime = useCallback((
    runtime: HostRuntime,
    preserveActiveWorkspace: boolean,
  ) => {
    hostBindingGenerationRef.current += 1;
    invalidateHostBoundUi();
    setHostRuntime(runtime);
    const workspaces = runtime.kind === "browser_demo"
      ? browserDemoWorkspaces
      : runtime.kind === "ready" || runtime.kind === "read_only_recovery"
        ? runtime.bootstrap.workspaces
        : [];
    adoptWorkspaces(workspaces, preserveActiveWorkspace);
  }, [adoptWorkspaces, browserDemoWorkspaces, invalidateHostBoundUi]);

  const republishHostRuntime = useCallback((runtime: BoundRuntime) => {
    setHostRuntime(runtime);
  }, []);

  /*
   * `useCallback`, not a plain declaration, because both markers are captured
   * by long-lived effects -- the projection poller and the method-guidance
   * recovery effect. A declaration is a new function on every render, so it can
   * never be an honest dependency: naming it would restart the poller on every
   * render, and omitting it leaves the poller holding the closure from whichever
   * render happened to mount it. That closure reads `invalidateHostBoundUi` from
   * a stale render, so the transition into read-only recovery could invalidate
   * the wrong host binding's UI.
   */
  const markReadOnlyRecovery = useCallback((
    client: BoundHostClient,
    sequence: number,
  ) => {
    hostBindingGenerationRef.current += 1;
    invalidateHostBoundUi();
    setHostRuntime((current) => {
      // Identity check on the client, not just the kind: a report from a
      // binding that has already been replaced must not degrade the live one.
      if (
        (current.kind !== "ready" && current.kind !== "read_only_recovery")
        || current.client !== client
      ) {
        return current;
      }
      return {
        kind: "read_only_recovery",
        client,
        bootstrap: {
          ...current.bootstrap,
          bootMode: "read_only_recovery",
          // The command list is narrowed rather than the client being dropped.
          // Recovery is a state the user can see and act on, and seeing it
          // needs the two reads that describe it.
          supportedCommands: current.bootstrap.supportedCommands.filter(
            (command) => command === "app.get_boot_state" || command === "workspace.list",
          ),
          projectionSequence: Math.max(current.bootstrap.projectionSequence, sequence),
        },
      };
    });
    reportRecoveryNotice(RECOVERY_NOTICE);
  }, [invalidateHostBoundUi, reportRecoveryNotice]);

  /** Stabilised for the same reason as `markReadOnlyRecovery` above. */
  const markHostUnavailable = useCallback((client: BoundHostClient) => {
    hostBindingGenerationRef.current += 1;
    invalidateHostBoundUi();
    setHostRuntime((current) => {
      if (
        (current.kind !== "ready" && current.kind !== "read_only_recovery")
        || current.client !== client
      ) {
        return current;
      }
      return {
        kind: "unavailable",
        client: null,
        bootstrap: null,
        message: HOST_UNREACHABLE_MESSAGE,
      };
    });
  }, [invalidateHostBoundUi]);

  useEffect(() => {
    let isCurrent = true;
    void loadRuntime()
      .then((runtime) => {
        if (!isCurrent) return;
        adoptHostRuntime(runtime, false);
      })
      .catch(() => {
        if (!isCurrent) return;
        adoptHostRuntime({
          kind: "unavailable",
          client: null,
          bootstrap: null,
          message: HOST_UNREACHABLE_MESSAGE,
        }, false);
      });
    return () => { isCurrent = false; };
  }, [adoptHostRuntime, attempt, loadRuntime]);

  /*
   * The way back out of `unavailable`.
   *
   * Clearing the memoised default runtime is what makes the retry real:
   * without it the loader hands back the same settled failure forever, and the
   * button looks broken rather than the host looking absent.
   */
  const retryHostRuntime = useCallback(() => {
    resetDefaultHostRuntime();
    setHostRuntime({ kind: "loading" });
    setAttempt((current) => current + 1);
  }, []);

  return {
    adoptHostRuntime,
    hostBindingGenerationRef,
    hostRuntime,
    markHostUnavailable,
    markReadOnlyRecovery,
    republishHostRuntime,
    retryHostRuntime,
  };
}

export const hostUnreachableMessage = HOST_UNREACHABLE_MESSAGE;
export const readOnlyRecoveryNotice = RECOVERY_NOTICE;
