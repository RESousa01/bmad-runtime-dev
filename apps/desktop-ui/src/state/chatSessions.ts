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

/// Records the exchange when a message dispatches an agent menu item instead
/// of sending a chat turn.
///
/// A dispatch is still a turn the user took. Routing straight to the capability
/// run without this dropped the message: the composer cleared, a run opened
/// elsewhere, and the conversation held no record of the sentence that caused
/// it. The assistant line is deliberately explicit that nothing has been sent —
/// opening a run is not a model request.
export function appendDispatchExchange(
  lane: ChatLaneState,
  intent: string,
  label: string,
  at: number,
): ChatLaneState {
  return {
    ...lane,
    messages: [
      ...lane.messages,
      { role: "user", text: intent, createdAt: at },
      {
        role: "assistant",
        text: `Opening the reviewed run for “${label}”. Nothing has been sent yet — review the context and approve it to run.`,
        createdAt: at,
      },
    ],
    pending: null,
    error: null,
  };
}

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

/// Discards one task's conversation, for closing or deleting that task.
///
/// The generation is advanced rather than deleted. Dropping the entry outright
/// would reset it to 0, and a reply captured under generation 0 — the very
/// first send in a task — would then still pass `routingIsCurrent` and
/// resurrect the transcript the user just discarded. Keeping a bumped
/// generation is what makes a late reply for a removed task drop.
export function dropLane(store: ChatSessionStore, sessionId: string): ChatSessionStore {
  const lanes = new Map(store.lanes);
  lanes.delete(sessionId);
  const generations = new Map(store.generations);
  generations.set(sessionId, generationForSession(store, sessionId) + 1);
  return { lanes, generations };
}

/// Abandons the active task's turn: the composer stops waiting, and the reply
/// already computed for that turn is dropped instead of appended.
///
/// The lane is cleared before the generation advances, because
/// `applyToLane` would refuse the write afterwards and the composer would stay
/// disabled with a pending question that can never resolve.
export function supersedeLane(
  store: ChatSessionStore,
  sessionId: string,
): ChatSessionStore {
  const lane = laneForSession(store, sessionId);
  const cleared = lane.sending
    ? setLane(store, sessionId, { ...lane, sending: false, pending: null })
    : store;
  return bumpGeneration(cleared, sessionId);
}

/// Drops every transcript, for a host rebind or a workspace revocation.
///
/// Generations advance rather than resetting to the empty map. Returning
/// `emptyChatSessionStore` looked equivalent — the lanes are gone either way —
/// but it reset every generation to 0, so a routing token captured at
/// generation 0 (the first send in a task, the common case) still satisfied
/// `routingIsCurrent` afterwards. A reply computed against the *revoked*
/// workspace could then be appended to the transcript of the workspace that
/// replaced it. Keeping a bumped generation per known task is what makes those
/// in-flight replies drop. Mirrors `clearAllRuns` in `taskSessions`.
export function clearAllLanes(store: ChatSessionStore): ChatSessionStore {
  const generations = new Map(store.generations);
  const known = new Set<string>([...store.lanes.keys(), ...store.generations.keys()]);
  for (const sessionId of known) {
    generations.set(sessionId, (generations.get(sessionId) ?? 0) + 1);
  }
  return { lanes: new Map(), generations };
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
  /// Abandons the active task's in-flight turn. The host still finishes the
  /// request it was given — this is a client-side abandon — but the reply is
  /// dropped rather than appended, and the composer is released.
  readonly supersedeActive: () => void;
  /// Discards one task's conversation and supersedes any reply in flight for
  /// it. Used when that task is closed or deleted.
  readonly dropSession: (sessionId: string) => void;
  /// How many messages one task's transcript holds. Used to tell the user what
  /// a delete would actually discard.
  readonly messageCountFor: (sessionId: string) => number;
  readonly clearAll: () => void;
}

export function useChatSessions(activeSessionId: string): ChatSessionsApi {
  const [store, setStore] = useState<ChatSessionStore>(emptyChatSessionStore);
  // The ref is the authoritative store; `store` is its render mirror. Callbacks
  // read the ref so several mutations in one tick compose against the freshest
  // value instead of the last rendered one, and so none of them has to take the
  // store as a dependency.
  //
  // The ref is written *only* here, in `commit`. It used to be written in two
  // other places, both illegal:
  //
  //   - `storeRef.current = store` in the component body, i.e. during render. A
  //     render React discards still ran that assignment.
  //   - inside the `setStore` updater, which must be a pure function of its
  //     argument. React double-invokes updaters under StrictMode, so that write
  //     ran twice per commit against a function the caller supplies.
  //
  // Deriving the next store from the ref and passing a plain value to
  // `setStore` keeps the same "freshest value in this tick" behaviour with no
  // write during render and a pure state update. `setStore` is not called
  // anywhere else, so the two cannot diverge.
  const storeRef = useRef(store);

  const commit = useCallback((next: (current: ChatSessionStore) => ChatSessionStore) => {
    const updated = next(storeRef.current);
    storeRef.current = updated;
    setStore(updated);
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
    commit((current) => supersedeLane(current, activeSessionId));
  }, [activeSessionId, commit]);

  const dropSession = useCallback(
    (sessionId: string) => {
      commit((current) => dropLane(current, sessionId));
    },
    [commit],
  );

  const clearAll = useCallback(() => {
    commit((current) => clearAllLanes(current));
  }, [commit]);

  const messageCountFor = useCallback(
    (sessionId: string) => laneForSession(store, sessionId).messages.length,
    [store],
  );

  return {
    lane: laneForSession(store, activeSessionId),
    captureRouting,
    routingFor,
    applyToLane,
    setActiveLane,
    supersedeActive,
    dropSession,
    messageCountFor,
    clearAll,
  };
}
