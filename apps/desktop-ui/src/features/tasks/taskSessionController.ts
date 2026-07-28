/**
 * The state a task owns, keyed by task.
 *
 * FE-13. Drafts, attachments and the model-context snapshot were three
 * different shapes of the same mistake:
 *
 *   - drafts lived in one `Map` in the composition root, which was task-keyed
 *     but owned by nobody;
 *   - attachments lived in a single app-global slot, so switching tasks
 *     destroyed them -- `selectSession` called `clearContext()` on every switch
 *     precisely because the alternative was sending one task's files with
 *     another task's message;
 *   - there was no model-context snapshot at all, so the Context panel showed
 *     the attachment list and called it usage.
 *
 * One store, one key, one generation per task. A task keeps what it has while
 * the user works elsewhere, and nothing another task does can reach it.
 *
 * Why a generation
 * ----------------
 * The same reason `capabilitySessions` has one: an auto-selection is a host
 * round trip that outlives the user's attention. A suggestion started in task A
 * and resolving after the user has moved on must land on A or nowhere -- never
 * on whatever is visible. Callers capture a token at request start and hand it
 * back; a token whose generation has moved is dropped.
 *
 * Closing a task drops its session outright. A closed task keeps its transcript
 * -- that is the host's -- but not its unsent attachments: they were selected
 * against a workspace grant for a message that was never sent, and silently
 * re-arming them on reopen would attach files the user last thought about in a
 * different context.
 */

import { useCallback, useRef, useState } from "react";

import {
  emptyTaskAttachments,
  type TaskAttachments,
} from "../attachments/attachmentTypes";
import {
  reconcile,
  unmeasuredModelContext,
  type ModelContextKey,
  type ModelContextState,
} from "../model-context/modelContextTypes";

/**
 * Bumped when the shape changes.
 *
 * Nothing persists a session yet, so no migration exists to get wrong. It is
 * declared because FE-13 asks for a versioned schema and the version has to
 * start somewhere honest: at 1, with the note that adding persistence is what
 * makes it load-bearing.
 */
export const TASK_SESSION_SCHEMA_VERSION = 1;

/** Everything one task owns. */
export interface TaskSession {
  /** Unsent composer text. Survives a failed send; see `chatSessions.failSend`. */
  readonly draft: string;
  readonly attachments: TaskAttachments;
  readonly modelContext: ModelContextState;
}

export const emptyTaskSession: TaskSession = {
  attachments: emptyTaskAttachments,
  draft: "",
  modelContext: unmeasuredModelContext,
};

export interface TaskSessionStore {
  readonly sessions: ReadonlyMap<string, TaskSession>;
  readonly generations: ReadonlyMap<string, number>;
}

export const emptyTaskSessionStore: TaskSessionStore = {
  generations: new Map(),
  sessions: new Map(),
};

/** Attribution token captured at request start. */
export interface TaskSessionRouting {
  readonly taskId: string;
  readonly generation: number;
}

export function sessionFor(store: TaskSessionStore, taskId: string): TaskSession {
  return store.sessions.get(taskId) ?? emptyTaskSession;
}

export function generationFor(store: TaskSessionStore, taskId: string): number {
  return store.generations.get(taskId) ?? 0;
}

export function captureRouting(store: TaskSessionStore, taskId: string): TaskSessionRouting {
  return { generation: generationFor(store, taskId), taskId };
}

/**
 * Whether a token still describes the task it was captured from.
 *
 * False once the task was closed, or once its generation moved -- a newer
 * request superseded this one, and applying the older answer would show the
 * user a selection they have already replaced.
 */
export function routingIsCurrent(store: TaskSessionStore, routing: TaskSessionRouting): boolean {
  return store.sessions.has(routing.taskId)
    && generationFor(store, routing.taskId) === routing.generation;
}

export type TaskSessionUpdate = (session: TaskSession) => TaskSession;

export function updateSession(
  store: TaskSessionStore,
  taskId: string,
  update: TaskSessionUpdate,
): TaskSessionStore {
  const sessions = new Map(store.sessions);
  sessions.set(taskId, update(sessionFor(store, taskId)));
  return { generations: store.generations, sessions };
}

/** Applies an update only if the token is still current. */
export function routeUpdate(
  store: TaskSessionStore,
  routing: TaskSessionRouting,
  update: TaskSessionUpdate,
): TaskSessionStore {
  if (!routingIsCurrent(store, routing)) return store;
  return updateSession(store, routing.taskId, update);
}

/** Supersedes anything in flight for a task, without touching what it holds. */
export function supersede(store: TaskSessionStore, taskId: string): TaskSessionStore {
  const generations = new Map(store.generations);
  generations.set(taskId, generationFor(store, taskId) + 1);
  // The session is materialised so `routingIsCurrent` can tell "superseded"
  // from "closed" -- a token for a task with no entry is the latter.
  const sessions = store.sessions.has(taskId)
    ? store.sessions
    : new Map(store.sessions).set(taskId, emptyTaskSession);
  return { generations, sessions };
}

/** Drops a task's session entirely. */
export function closeSession(store: TaskSessionStore, taskId: string): TaskSessionStore {
  if (!store.sessions.has(taskId) && !store.generations.has(taskId)) return store;
  const sessions = new Map(store.sessions);
  const generations = new Map(store.generations);
  sessions.delete(taskId);
  // The generation is *kept*. A token captured before the close still has to
  // read as stale, and deleting the counter would restart it at zero and make
  // an old token look current again if the id were ever reused.
  generations.set(taskId, generationFor(store, taskId) + 1);
  return { generations, sessions };
}

/**
 * Drops every session.
 *
 * For a workspace switch or a host rebind: attachments are resolved against a
 * workspace grant, and a draft is the only thing that could survive -- but a
 * draft written about files from another workspace is not obviously worth
 * keeping either, and guessing is what FE-13 forbids.
 */
export function clearAllSessions(store: TaskSessionStore): TaskSessionStore {
  const generations = new Map(store.generations);
  for (const taskId of store.sessions.keys()) {
    generations.set(taskId, generationFor(store, taskId) + 1);
  }
  return { generations, sessions: new Map() };
}

/** Re-evaluates every task's model-context snapshot against its current key. */
export function reconcileModelContext(
  store: TaskSessionStore,
  keyFor: (taskId: string) => ModelContextKey,
): TaskSessionStore {
  let changed = false;
  const sessions = new Map(store.sessions);
  for (const [taskId, session] of store.sessions) {
    const next = reconcile(session.modelContext, keyFor(taskId));
    if (next === session.modelContext) continue;
    sessions.set(taskId, { ...session, modelContext: next });
    changed = true;
  }
  return changed ? { generations: store.generations, sessions } : store;
}

export interface TaskSessionsApi {
  readonly store: TaskSessionStore;
  /** The active task's session. */
  readonly session: TaskSession;
  readonly sessionFor: (taskId: string) => TaskSession;
  readonly update: (taskId: string, update: TaskSessionUpdate) => void;
  readonly updateActive: (update: TaskSessionUpdate) => void;
  readonly captureRouting: (taskId?: string) => TaskSessionRouting;
  /**
   * Supersedes anything in flight and returns the token for the new request.
   *
   * One call, because the two steps cannot be composed by a caller: `setStore`
   * is asynchronous and the ref behind `captureRouting` only re-syncs on
   * render, so superseding and then capturing in the same handler returns the
   * *pre-bump* generation -- a token that is stale the moment the bump lands,
   * which silently drops every result. Capturing first and superseding after
   * invalidates it just as fast.
   */
  readonly beginRequest: (taskId?: string) => TaskSessionRouting;
  readonly route: (routing: TaskSessionRouting, update: TaskSessionUpdate) => void;
  /** Whether a captured token still describes the task it came from. */
  readonly isCurrent: (routing: TaskSessionRouting) => boolean;
  readonly supersede: (taskId?: string) => void;
  readonly close: (taskId: string) => void;
  readonly clearAll: () => void;
}

export function useTaskSessions(activeTaskId: string): TaskSessionsApi {
  const [store, setStore] = useState<TaskSessionStore>(emptyTaskSessionStore);
  /*
   * Written only by `commit`. Refreshing it with `storeRef.current = store` in
   * the component body is a write during render, which a render React discards
   * still performs -- the same mistake `useCapabilitySessions` documents. Every
   * mutation routes through `commit`, so the mirror stays correct without it.
   */
  const storeRef = useRef(store);

  const commit = useCallback((next: TaskSessionStore) => {
    storeRef.current = next;
    setStore(next);
  }, []);

  const update = useCallback((taskId: string, next: TaskSessionUpdate) => {
    commit(updateSession(storeRef.current, taskId, next));
  }, [commit]);

  return {
    beginRequest: (taskId?: string) => {
      const id = taskId ?? activeTaskId;
      const next = supersede(storeRef.current, id);
      commit(next);
      return { generation: generationFor(next, id), taskId: id };
    },
    captureRouting: (taskId?: string) =>
      captureRouting(storeRef.current, taskId ?? activeTaskId),
    // Stable: named as a dependency by the workspace and host invalidators, so
    // it must not change identity on a task switch.
    clearAll: useCallback(() => commit(clearAllSessions(storeRef.current)), [commit]),
    close: useCallback((taskId: string) => {
      commit(closeSession(storeRef.current, taskId));
    }, [commit]),
    isCurrent: (routing: TaskSessionRouting) => routingIsCurrent(storeRef.current, routing),
    route: useCallback((routing: TaskSessionRouting, next: TaskSessionUpdate) => {
      commit(routeUpdate(storeRef.current, routing, next));
    }, [commit]),
    session: sessionFor(store, activeTaskId),
    sessionFor: (taskId: string) => sessionFor(store, taskId),
    store,
    supersede: (taskId?: string) => {
      commit(supersede(storeRef.current, taskId ?? activeTaskId));
    },
    update,
    updateActive: (next: TaskSessionUpdate) => update(activeTaskId, next),
  };
}
