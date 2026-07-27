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
//
// `historyStatus` is what makes that sentence true rather than aspirational.
// Nothing used to re-read: the store was the only copy the UI had, so eviction
// below, a reload, or a workspace switch made a transcript that is still on
// disk disappear from the screen. A lane starts `unread`, the shell reads
// `chat.history` for it once, and the answer replaces the empty lane. Because
// the status lives *in* the lane, it is destroyed exactly when the lane is:
// an evicted lane reads back as `unread` and is re-read on its next
// activation, which is what turns eviction from data loss into a cache miss.

import { useCallback, useRef, useState } from "react";

import type { ChatHistoryProjection, ChatMessage } from "../lib/hostClient";

/// Matches `MAX_LANE_SESSIONS` in the capability store: the same tab-count
/// pressure applies, and the same eviction rule keeps it bounded.
const MAX_LANE_SESSIONS = 20;

/// How many messages the host retains per conversation. Mirrors
/// `MAX_SESSION_MESSAGES` in `desktop-runtime`'s `chat` module and the parser's
/// `MAX_TRANSCRIPT_MESSAGES`: past it the host drops whole oldest turns, so a
/// transcript this long is a window onto the conversation rather than all of
/// it, and the UI says so instead of presenting it as complete.
export const RETAINED_TRANSCRIPT_MESSAGES = 200;

/// Where this lane stands in reading the host's copy of its transcript.
///
///   - `unread`: never asked. The lane may be genuinely new, or it may have
///     been evicted, cleared, or lost to a reload — the renderer cannot tell
///     the difference, and does not need to: it asks.
///   - `loading`: a read is in flight. Also the lock that stops a second read
///     being started for the same lane.
///   - `loaded`: the host answered, or a live turn made the answer moot.
///   - `unavailable`: the read failed. The lane carries the reason in `error`
///     and offers a retry; what it must never do is sit there empty and silent,
///     which reads as "you have said nothing here".
export type ChatHistoryStatus = "unread" | "loading" | "loaded" | "unavailable";

export interface ChatLaneState {
  readonly messages: readonly ChatMessage[];
  /// The BMAD agent bound to this conversation, or `null` for plain chat.
  readonly agentCode: string | null;
  /// A send is in flight; the composer is disabled and the pending question is
  /// shown optimistically.
  readonly sending: boolean;
  /// The question that has not been answered — either still in flight, or left
  /// standing because the send failed.
  ///
  /// It used to be cleared on failure, which destroyed what the user had typed.
  /// A failed turn is exactly when they most need it back: the host now
  /// classifies provider failures, and a rate limit or a provider outage is
  /// answered by sending the same thing again. Telling someone to retry while
  /// having thrown away the thing to retry is worse than not telling them.
  ///
  /// Keeping it is safe because `composerDisabled` is gated on `sending`, never
  /// on this — see `TaskWorkspace.tsx`.
  readonly pending: string | null;
  readonly error: string | null;
  /// Whether `error` is one that sending the same message again could clear.
  ///
  /// Comes from the host's `LocalError.retryable`, which is now derived per
  /// provider-failure class rather than hard-coded `false`.
  ///
  /// **Recorded, not yet consumed.** Nothing reads this today: the only retry
  /// control in the transcript is gated on `historyStatus === "unavailable"`,
  /// which is the *history* retry, not a send retry. An earlier version of this
  /// comment said the send affordance followed this field, which was not true of
  /// any code. It is kept because the composer already restores the failed
  /// draft, so a "send again" control has the text it needs and only wants this
  /// flag to decide whether offering it is honest — a retry on a rejected key
  /// would be a loop that cannot succeed.
  readonly retryable: boolean;
  readonly historyStatus: ChatHistoryStatus;
}

export const emptyChatLane: ChatLaneState = {
  messages: [],
  agentCode: null,
  sending: false,
  pending: null,
  error: null,
  retryable: false,
  historyStatus: "unread",
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
    // These two messages exist only here — no `chat.send` was issued, so the
    // host has no record of them. A later restore must therefore not overwrite
    // them, and marking the lane read is what stops one from trying.
    historyStatus: "loaded",
  };
}

/// Folds the host's copy of a transcript into a lane that was waiting for it.
///
/// Replacement, not merge: the host is the authority and the lane was empty, so
/// there is nothing to reconcile and nothing that can be duplicated. The two
/// refusals below are the whole safety argument, because a read is slow enough
/// for the lane to have come alive underneath it:
///
///   - a turn in flight is left strictly alone. The host's answer predates the
///     question now being asked, so writing it would drop the optimistic
///     pending state, and the reply landing afterwards would append a turn the
///     restored transcript may already contain.
///   - a lane that has acquired messages by any other route is left alone for
///     the same reason. That covers a send that completed while the read was
///     out, and the dispatch exchange, which is renderer-only and would simply
///     be erased.
///
/// Both refusals still mark the lane read. The question "does the host have
/// something we are missing?" has been asked and answered; asking again on
/// every render would be a loop.
export function applyRestoredHistory(
  lane: ChatLaneState,
  projection: ChatHistoryProjection,
): ChatLaneState {
  if (lane.sending || lane.messages.length > 0) {
    return { ...lane, historyStatus: "loaded" };
  }
  return {
    ...lane,
    messages: projection.messages,
    // Only adopt the persona the host has on record if the user has not bound
    // one while the read was in flight. Their selection is newer than this
    // answer, and clearing it would silently return the composer to plain chat.
    agentCode: lane.agentCode ?? projection.agentCode,
    error: null,
    historyStatus: "loaded",
  };
}

/// Records a failed read. The lane stays empty, but it stops being *silently*
/// empty: the message renders as an alert and the retry below is offered.
export function failHistoryRestore(lane: ChatLaneState, message: string): ChatLaneState {
  if (lane.sending || lane.messages.length > 0) {
    return { ...lane, historyStatus: "unavailable" };
  }
  return { ...lane, error: message, historyStatus: "unavailable" };
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

/// Claims the one history read a lane is owed, returning the store to commit
/// and the routing token to hand back on completion — or `null` when this lane
/// is not waiting for one.
///
/// Claim and mark are one step on purpose. The decision "should I read?" and
/// the write that records "I am reading" have to be indivisible against the
/// same store value, or two callers in the same tick — a re-rendering effect,
/// React's development double-invoke — each see `unread` and each dispatch a
/// read. The routing token is the same one a send captures, so a read that
/// outlives its task, its workspace grant, or an explicit supersede is dropped
/// by exactly the mechanism that already drops a late reply.
export function beginHistoryRestore(
  store: ChatSessionStore,
  sessionId: string,
): { readonly store: ChatSessionStore; readonly routing: ChatRouting } | null {
  const lane = laneForSession(store, sessionId);
  if (lane.historyStatus !== "unread") return null;
  return {
    store: setLane(store, sessionId, { ...lane, historyStatus: "loading" }),
    routing: { sessionId, generation: generationForSession(store, sessionId) },
  };
}

/// Puts a failed lane back in line for a read.
export function retryHistoryRestore(store: ChatSessionStore, sessionId: string): ChatSessionStore {
  const lane = laneForSession(store, sessionId);
  if (lane.historyStatus !== "unavailable") return store;
  return setLane(store, sessionId, { ...lane, error: null, historyStatus: "unread" });
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
  /// Claims the one history read this lane is owed. Returns the routing token
  /// to hand back to `applyToLane`, or `null` when the lane has already been
  /// read, is being read, or gave up. Callers must treat `null` as "do not
  /// dispatch": it is the only thing standing between an effect that re-runs
  /// and an unbounded stream of reads.
  readonly beginHistoryRestore: (sessionId: string) => ChatRouting | null;
  /// Puts a lane whose read failed back in line for another attempt.
  readonly retryHistoryRestore: (sessionId: string) => void;
  /// How many messages one task's transcript holds. Used to tell the user what
  /// a delete would actually discard.
  readonly messageCountFor: (sessionId: string) => number;
  readonly historyStatusFor: (sessionId: string) => ChatHistoryStatus;
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

  // Reads and writes `storeRef` directly rather than going through `commit`,
  // because the caller needs the claim's *outcome* synchronously — `commit`
  // returns nothing, and deciding from the rendered `store` would let two runs
  // in one tick both see `unread`.
  const claimHistoryRestore = useCallback((sessionId: string): ChatRouting | null => {
    const claimed = beginHistoryRestore(storeRef.current, sessionId);
    if (claimed === null) return null;
    storeRef.current = claimed.store;
    setStore(claimed.store);
    return claimed.routing;
  }, []);

  const requestHistoryRetry = useCallback(
    (sessionId: string) => {
      commit((current) => retryHistoryRestore(current, sessionId));
    },
    [commit],
  );

  const messageCountFor = useCallback(
    (sessionId: string) => laneForSession(store, sessionId).messages.length,
    [store],
  );

  /// Whether `messageCountFor` is answering from a transcript that was actually
  /// read, or from a lane that has simply never been filled.
  ///
  /// Both return `0` messages, and the difference decides whether the delete
  /// confirmation may tell the user there is nothing to lose.
  const historyStatusFor = useCallback(
    (sessionId: string) => laneForSession(store, sessionId).historyStatus,
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
    beginHistoryRestore: claimHistoryRestore,
    retryHistoryRestore: requestHistoryRetry,
    messageCountFor,
    historyStatusFor,
    clearAll,
  };
}
