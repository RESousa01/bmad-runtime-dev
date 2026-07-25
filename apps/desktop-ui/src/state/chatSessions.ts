// Per-task conversation state.
//
// Chat is Forge's default lane, so every task tab owns a transcript. The store
// follows the same rules as `taskSessions`: it is immutable, keyed by the
// durable renderer task id, and bounded, so a long session with many tabs
// cannot grow without limit.
//
// Routing matters here for the same reason it does for capability runs. A reply
// can land after the user has switched tabs, and it must be applied to the task
// that asked the question — never to whichever tab happens to be visible. The
// caller captures a token at send time and hands it back on completion; a
// superseded or closed task drops the reply.
//
// The host owns the authoritative transcript (it builds each request from it).
// This store is the renderer's view of it, so a reload or a workspace change
// re-reads history from the host rather than trusting what is cached here.

import { useCallback, useRef, useState } from "react";

import type { ChatMessage } from "../lib/hostClient";

/// Matches `MAX_LANE_SESSIONS` in the capability store: the same tab-count
/// pressure applies, and the same eviction rule keeps it bounded.
const MAX_LANE_SESSIONS = 20;

export interface ChatLaneState {
  readonly messages: readonly ChatMessage[];
  /// The BMAD agent bound to this conversation, or `null` for plain chat.
  readonly agentCode: string | null;
  /// A send is in flight; the composer is disabled and the pending question is
  /// shown optimistically.
  readonly sending: boolean;
  /// The question awaiting its reply, shown while `sending`.
  readonly pending: string | null;
  readonly error: string | null;
}

export const emptyChatLane: ChatLaneState = {
  messages: [],
  agentCode: null,
  sending: false,
  pending: null,
  error: null,
};

export interface ChatSessionStore {
  readonly lanes: ReadonlyMap<string, ChatLaneState>;
  readonly generations: ReadonlyMap<string, number>;
}

export const emptyChatSessionStore: ChatSessionStore = {
  lanes: new Map(),
  generations: new Map(),
};

export interface ChatRouting {
  readonly sessionId: string;
  readonly generation: number;
}

export function laneForSession(store: ChatSessionStore, sessionId: string): ChatLaneState {
  return store.lanes.get(sessionId) ?? emptyChatLane;
}

export function generationForSession(store: ChatSessionStore, sessionId: string): number {
  return store.generations.get(sessionId) ?? 0;
}

/// Bounds the map, evicting oldest-first and never the lane just written.
function evict(lanes: Map<string, ChatLaneState>, keep: string): Map<string, ChatLaneState> {
  while (lanes.size > MAX_LANE_SESSIONS) {
    const oldest = [...lanes.keys()].find((key) => key !== keep);
    if (oldest === undefined) break;
    lanes.delete(oldest);
  }
  return lanes;
}

export function setLane(
  store: ChatSessionStore,
  sessionId: string,
  update: ChatLaneState | ((current: ChatLaneState) => ChatLaneState),
): ChatSessionStore {
  const current = laneForSession(store, sessionId);
  const next = typeof update === "function" ? update(current) : update;
  const lanes = new Map(store.lanes);
  lanes.set(sessionId, next);
  return { lanes: evict(lanes, sessionId), generations: store.generations };
}

/// Supersedes any in-flight reply for one task, so a stale completion is
/// dropped rather than applied.
export function bumpGeneration(store: ChatSessionStore, sessionId: string): ChatSessionStore {
  const generations = new Map(store.generations);
  generations.set(sessionId, generationForSession(store, sessionId) + 1);
  return { lanes: store.lanes, generations };
}

/// Whether a reply captured under `routing` may still be applied.
export function routingIsCurrent(store: ChatSessionStore, routing: ChatRouting): boolean {
  return generationForSession(store, routing.sessionId) === routing.generation;
}

export function clearAllLanes(): ChatSessionStore {
  return emptyChatSessionStore;
}

export interface ChatSessionsApi {
  readonly lane: ChatLaneState;
  readonly captureRouting: () => ChatRouting;
  /// Routing for a task the caller names explicitly — used when a task was
  /// created in the same tick as the send, so the active-session state has not
  /// settled yet.
  readonly routingFor: (sessionId: string) => ChatRouting;
  readonly applyToLane: (
    routing: ChatRouting,
    update: ChatLaneState | ((current: ChatLaneState) => ChatLaneState),
  ) => void;
  readonly setActiveLane: (
    update: ChatLaneState | ((current: ChatLaneState) => ChatLaneState),
  ) => void;
  readonly supersedeActive: () => void;
  readonly clearAll: () => void;
}

export function useChatSessions(activeSessionId: string): ChatSessionsApi {
  const [store, setStore] = useState<ChatSessionStore>(emptyChatSessionStore);
  // Mirrors the store so callbacks read the freshest value without taking the
  // store as a dependency (StrictMode-safe; same pattern as taskSessions).
  const storeRef = useRef(store);
  storeRef.current = store;

  const commit = useCallback((next: (current: ChatSessionStore) => ChatSessionStore) => {
    setStore((current) => {
      const updated = next(current);
      storeRef.current = updated;
      return updated;
    });
  }, []);

  const captureRouting = useCallback(
    (): ChatRouting => ({
      sessionId: activeSessionId,
      generation: generationForSession(storeRef.current, activeSessionId),
    }),
    [activeSessionId],
  );

  const routingFor = useCallback(
    (sessionId: string): ChatRouting => ({
      sessionId,
      generation: generationForSession(storeRef.current, sessionId),
    }),
    [],
  );

  const applyToLane = useCallback(
    (
      routing: ChatRouting,
      update: ChatLaneState | ((current: ChatLaneState) => ChatLaneState),
    ) => {
      if (!routingIsCurrent(storeRef.current, routing)) return;
      commit((current) => setLane(current, routing.sessionId, update));
    },
    [commit],
  );

  const setActiveLane = useCallback(
    (update: ChatLaneState | ((current: ChatLaneState) => ChatLaneState)) => {
      commit((current) => setLane(current, activeSessionId, update));
    },
    [activeSessionId, commit],
  );

  const supersedeActive = useCallback(() => {
    commit((current) => bumpGeneration(current, activeSessionId));
  }, [activeSessionId, commit]);

  const clearAll = useCallback(() => {
    commit(clearAllLanes);
  }, [commit]);

  return {
    lane: laneForSession(store, activeSessionId),
    captureRouting,
    routingFor,
    applyToLane,
    setActiveLane,
    supersedeActive,
    clearAll,
  };
}
