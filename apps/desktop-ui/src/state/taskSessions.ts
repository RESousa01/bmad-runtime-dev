// Per-task execution state for Forge v2 tasks.
//
// Forge renders multiple task tabs, but the capability-run execution state used
// to live in a single app-global slot: a slow completion that resolved after
// the user switched tabs was written under whichever task was *visible*, not the
// task that issued it. This module makes that state task-owned.
//
// Each task session has its own capability run plus a monotonic `generation`.
// A caller captures a routing token at request start and hands it back at
// completion; the result is applied only if the owning task's generation is
// unchanged (it was not superseded by a newer run) and the task still exists (it
// was not closed). Otherwise the late result is dropped. That is what keeps a
// task A completion from ever landing on task B.
//
// The store is deliberately immutable: every mutator returns a new store, so the
// React hook below can drive re-renders and the core stays trivially testable
// without mounting React. Session identity here is the durable renderer task id;
// promoting it to a host-issued session id is part of the deferred Agent-session
// ownership milestone (`session.create` is currently a fail-closed host stub).

import { useCallback, useRef, useState } from "react";

import type { CapabilityRunPhase } from "../components/BmadCapabilityPanel";

export interface ActiveCapabilityRunState {
  readonly capabilityId: string;
  readonly label: string;
  readonly phase: CapabilityRunPhase;
}

/** A capability-run value, or an updater applied to the task's current value. */
export type CapabilityRunUpdate =
  | (ActiveCapabilityRunState | null)
  | ((current: ActiveCapabilityRunState | null) => ActiveCapabilityRunState | null);

/**
 * Attribution token captured at request start. `sessionId` is the owning task;
 * `generation` is that task's generation at capture time.
 */
export interface CapabilityRouting {
  readonly sessionId: string;
  readonly generation: number;
}

export interface CapabilitySessionStore {
  readonly runs: ReadonlyMap<string, ActiveCapabilityRunState | null>;
  readonly generations: ReadonlyMap<string, number>;
}

export const emptyCapabilitySessionStore: CapabilitySessionStore = {
  runs: new Map(),
  generations: new Map(),
};

export function runForSession(
  store: CapabilitySessionStore,
  sessionId: string,
): ActiveCapabilityRunState | null {
  return store.runs.get(sessionId) ?? null;
}

export function generationForSession(
  store: CapabilitySessionStore,
  sessionId: string,
): number {
  return store.generations.get(sessionId) ?? 0;
}

/** Captures the owning task's current generation without advancing it. */
export function captureRouting(
  store: CapabilitySessionStore,
  sessionId: string,
): CapabilityRouting {
  return { sessionId, generation: generationForSession(store, sessionId) };
}

function applyUpdate(
  current: ActiveCapabilityRunState | null,
  update: CapabilityRunUpdate,
): ActiveCapabilityRunState | null {
  return typeof update === "function" ? update(current) : update;
}

/**
 * Upper bound on retained per-task entries. Each created task adds one entry
 * that is otherwise never pruned; this caps unbounded growth over a long
 * session. The visible task list is far smaller (~20), so evicted entries are
 * long gone from view.
 */
export const MAX_LANE_SESSIONS = 64;

/**
 * Starts (or restarts) the run for a task, advancing its generation so any
 * still-in-flight request for the prior run drops on completion. Returns the
 * new store and the routing token the caller should capture for this run.
 */
export function openRun(
  store: CapabilitySessionStore,
  sessionId: string,
  run: ActiveCapabilityRunState,
): { store: CapabilitySessionStore; routing: CapabilityRouting } {
  const generation = generationForSession(store, sessionId) + 1;
  const generations = new Map(store.generations);
  generations.set(sessionId, generation);
  const runs = new Map(store.runs);
  runs.set(sessionId, run);
  // Bound growth: drop the oldest sessions (Map preserves insertion order)
  // beyond the cap, never the one just opened. A late result for an evicted
  // session drops naturally (its generation is gone).
  if (runs.size > MAX_LANE_SESSIONS) {
    for (const key of [...runs.keys()]) {
      if (runs.size <= MAX_LANE_SESSIONS) break;
      if (key === sessionId) continue;
      runs.delete(key);
      generations.delete(key);
    }
  }
  return { store: { runs, generations }, routing: { sessionId, generation } };
}

/** Sets a task's run without advancing its generation (same run, new phase). */
export function setRun(
  store: CapabilitySessionStore,
  sessionId: string,
  update: CapabilityRunUpdate,
): CapabilitySessionStore {
  const runs = new Map(store.runs);
  runs.set(sessionId, applyUpdate(runs.get(sessionId) ?? null, update));
  return { runs, generations: store.generations };
}

/**
 * Routes an async result back to its owning task. Applies the update only if the
 * task's generation still matches the captured token (not superseded) and the
 * task still exists (not closed); otherwise the result is dropped.
 */
export function routeRun(
  store: CapabilitySessionStore,
  token: CapabilityRouting,
  update: CapabilityRunUpdate,
): { store: CapabilitySessionStore; applied: boolean } {
  if (generationForSession(store, token.sessionId) !== token.generation) {
    return { store, applied: false };
  }
  if (!store.runs.has(token.sessionId) && !store.generations.has(token.sessionId)) {
    return { store, applied: false };
  }
  return { store: setRun(store, token.sessionId, update), applied: true };
}

/**
 * Clears every task's run and advances every known generation, so any in-flight
 * completion is dropped. Used on host rebind / workspace revocation.
 */
export function clearAllRuns(store: CapabilitySessionStore): CapabilitySessionStore {
  const generations = new Map(store.generations);
  const keys = new Set<string>([...store.runs.keys(), ...store.generations.keys()]);
  for (const key of keys) {
    generations.set(key, (generations.get(key) ?? 0) + 1);
  }
  return { runs: new Map(), generations };
}

/** Removes a task entirely, so its late results drop as "closed". */
export function closeSession(
  store: CapabilitySessionStore,
  sessionId: string,
): CapabilitySessionStore {
  const runs = new Map(store.runs);
  const generations = new Map(store.generations);
  runs.delete(sessionId);
  generations.delete(sessionId);
  return { runs, generations };
}

export interface CapabilitySessionsApi {
  /** The capability run for the currently active task (null if none). */
  readonly capabilityRun: ActiveCapabilityRunState | null;
  /** Starts a run on the active task, advancing its generation. */
  openActiveCapabilityRun(run: ActiveCapabilityRunState): CapabilityRouting;
  /** Captures the active task's routing token without advancing it. */
  captureActive(): CapabilityRouting;
  /** Updates the active task's run in place (same run, new phase). */
  setActiveCapabilityRun(update: CapabilityRunUpdate): void;
  /** Routes an async result to its owning task; drops if superseded/closed. */
  routeCapabilityRun(token: CapabilityRouting, update: CapabilityRunUpdate): boolean;
  /** Clears every task's run (host rebind / workspace revocation). */
  clearAllCapabilityRuns(): void;
  /** Removes a task, so its late results drop as closed. */
  closeCapabilitySession(sessionId: string): void;
}

/**
 * React binding over the pure store. Holds the store in state for rendering and
 * a mirror ref so mutators always compose against the latest store even when
 * several fire before React re-renders (rapid phase transitions, races).
 */
export function useCapabilitySessions(activeSessionId: string): CapabilitySessionsApi {
  const [store, setStore] = useState<CapabilitySessionStore>(emptyCapabilitySessionStore);
  const storeRef = useRef(store);
  storeRef.current = store;

  const commit = useCallback((next: CapabilitySessionStore) => {
    storeRef.current = next;
    setStore(next);
  }, []);

  const openActiveCapabilityRun = useCallback(
    (run: ActiveCapabilityRunState): CapabilityRouting => {
      const { store: next, routing } = openRun(storeRef.current, activeSessionId, run);
      commit(next);
      return routing;
    },
    [activeSessionId, commit],
  );

  const captureActive = useCallback(
    (): CapabilityRouting => captureRouting(storeRef.current, activeSessionId),
    [activeSessionId],
  );

  const setActiveCapabilityRun = useCallback(
    (update: CapabilityRunUpdate) => {
      commit(setRun(storeRef.current, activeSessionId, update));
    },
    [activeSessionId, commit],
  );

  const routeCapabilityRun = useCallback(
    (token: CapabilityRouting, update: CapabilityRunUpdate): boolean => {
      const { store: next, applied } = routeRun(storeRef.current, token, update);
      if (applied) {
        commit(next);
      }
      return applied;
    },
    [commit],
  );

  const clearAllCapabilityRuns = useCallback(() => {
    commit(clearAllRuns(storeRef.current));
  }, [commit]);

  const closeCapabilitySession = useCallback(
    (sessionId: string) => {
      commit(closeSession(storeRef.current, sessionId));
    },
    [commit],
  );

  return {
    capabilityRun: runForSession(store, activeSessionId),
    openActiveCapabilityRun,
    captureActive,
    setActiveCapabilityRun,
    routeCapabilityRun,
    clearAllCapabilityRuns,
    closeCapabilitySession,
  };
}
