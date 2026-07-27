import { describe, expect, it } from "vitest";

import {
  appendDispatchExchange,
  applyRestoredHistory,
  beginHistoryRestore,
  bumpGeneration,
  clearAllLanes,
  dropLane,
  emptyChatLane,
  emptyChatSessionStore,
  failHistoryRestore,
  generationForSession,
  laneForSession,
  retryHistoryRestore,
  routingIsCurrent,
  setLane,
} from "./chatSessions";
import type { ChatHistoryProjection, ChatMessage } from "../lib/hostClient";

const message = (text: string, role: "user" | "assistant" = "user"): ChatMessage => ({
  role,
  text,
  createdAt: 1_725_000_000_000,
});

describe("chat session store", () => {
  it("keeps each task's transcript separate", () => {
    let store = emptyChatSessionStore;
    store = setLane(store, "task-a", { ...emptyChatLane, messages: [message("from A")] });
    store = setLane(store, "task-b", { ...emptyChatLane, messages: [message("from B")] });

    expect(laneForSession(store, "task-a").messages[0]?.text).toBe("from A");
    expect(laneForSession(store, "task-b").messages[0]?.text).toBe("from B");
    expect(laneForSession(store, "task-c")).toEqual(emptyChatLane);
  });

  it("drops a reply whose task was superseded", () => {
    let store = emptyChatSessionStore;
    const routing = { sessionId: "task-a", generation: generationForSession(store, "task-a") };
    expect(routingIsCurrent(store, routing)).toBe(true);

    store = bumpGeneration(store, "task-a");

    expect(routingIsCurrent(store, routing)).toBe(false);
    // A different task is unaffected by task-a being superseded.
    expect(routingIsCurrent(store, { sessionId: "task-b", generation: 0 })).toBe(true);
  });

  it("binds an agent per conversation, independently of other tasks", () => {
    let store = emptyChatSessionStore;
    store = setLane(store, "task-a", (lane) => ({ ...lane, agentCode: "bmad-agent-architect" }));

    expect(laneForSession(store, "task-a").agentCode).toBe("bmad-agent-architect");
    // Plain chat stays the default everywhere else — BMAD is never imposed.
    expect(laneForSession(store, "task-b").agentCode).toBeNull();
  });

  it("discards one task's transcript when that task is deleted", () => {
    let store = emptyChatSessionStore;
    store = setLane(store, "task-a", { ...emptyChatLane, messages: [message("from A")] });
    store = setLane(store, "task-b", { ...emptyChatLane, messages: [message("from B")] });

    store = dropLane(store, "task-a");

    expect(laneForSession(store, "task-a")).toEqual(emptyChatLane);
    // Deleting one task never touches another's conversation.
    expect(laneForSession(store, "task-b").messages[0]?.text).toBe("from B");
  });

  it("drops a reply that lands after its task was deleted, rather than resurrecting it", () => {
    // The subtle case: the very first send in a task captures generation 0.
    // If deleting merely removed the generation entry it would read back as 0
    // again, the late reply would pass the routing check, and the transcript
    // the user just destroyed would reappear.
    let store = emptyChatSessionStore;
    const routing = { sessionId: "task-a", generation: generationForSession(store, "task-a") };
    expect(routing.generation).toBe(0);

    store = dropLane(store, "task-a");

    expect(routingIsCurrent(store, routing)).toBe(false);
  });

  it("drops replies still in flight when every transcript is cleared", () => {
    // The cross-workspace leak this guards. `clearAllLanes` runs on a workspace
    // revocation or a host rebind, and a task's *first* send captures
    // generation 0. Returning an empty store looked equivalent — the lanes are
    // gone either way — but it reset every generation to 0 as well, so a token
    // captured before the revocation still passed `routingIsCurrent`, and a
    // reply computed against the revoked workspace was appended to the
    // transcript of the workspace that replaced it.
    let store = emptyChatSessionStore;
    store = setLane(store, "task-a", {
      ...emptyChatLane,
      sending: true,
      pending: "against the old workspace",
    });
    const firstSend = { sessionId: "task-a", generation: generationForSession(store, "task-a") };
    expect(firstSend.generation).toBe(0);

    store = clearAllLanes(store);

    expect(store.lanes.size).toBe(0);
    expect(routingIsCurrent(store, firstSend)).toBe(false);
  });

  it("keeps supersededing a task whose lane was already evicted", () => {
    // A task can be known only by its generation: `setLane` evicts old lanes
    // while their sends are still in flight. Clearing must advance those too,
    // or the eviction itself would re-open the routing hole.
    let store = emptyChatSessionStore;
    store = bumpGeneration(store, "evicted-task");
    const routing = { sessionId: "evicted-task", generation: 1 };
    expect(store.lanes.has("evicted-task")).toBe(false);
    expect(routingIsCurrent(store, routing)).toBe(true);

    store = clearAllLanes(store);

    expect(routingIsCurrent(store, routing)).toBe(false);
  });

  it("bounds retained lanes and never evicts the one just written", () => {
    let store = emptyChatSessionStore;
    for (let index = 0; index < 40; index += 1) {
      store = setLane(store, `task-${index}`, {
        ...emptyChatLane,
        messages: [message(`m${index}`)],
      });
    }

    expect(store.lanes.size).toBeLessThanOrEqual(20);
    expect(laneForSession(store, "task-39").messages[0]?.text).toBe("m39");
  });
});

/// The store is a bounded cache in front of a durable transcript the host owns.
/// Everything here is about the moment those two copies meet: exactly once per
/// lane, and never on top of something live.
describe("chat history restore", () => {
  const projection = (
    messages: readonly ChatMessage[],
    agentCode: string | null = null,
  ): ChatHistoryProjection => ({
    sessionId: "chat_01ARZ3NDEKTSV4RRFFQ69G5FAV",
    agentCode,
    messages: [...messages],
  });

  it("claims the read once, so a re-running effect cannot amplify into many", () => {
    let store = emptyChatSessionStore;
    const claimed = beginHistoryRestore(store, "task-a");

    expect(claimed).not.toBeNull();
    expect(claimed!.routing).toEqual({ sessionId: "task-a", generation: 0 });
    store = claimed!.store;
    expect(laneForSession(store, "task-a").historyStatus).toBe("loading");

    // Same store value, second caller: the claim is already taken.
    expect(beginHistoryRestore(store, "task-a")).toBeNull();

    // And once the answer has landed the lane is not owed another read, or
    // every re-render of a genuinely empty conversation would issue one.
    store = setLane(store, "task-a", (lane) => ({ ...lane, historyStatus: "loaded" }));
    expect(beginHistoryRestore(store, "task-a")).toBeNull();
  });

  it("shows the host's transcript in a lane the renderer had nothing for", () => {
    const restored = applyRestoredHistory(
      { ...emptyChatLane, historyStatus: "loading" },
      projection([message("from the durable store"), message("and its reply", "assistant")]),
    );

    expect(restored.messages.map(({ text }) => text)).toEqual([
      "from the durable store",
      "and its reply",
    ]);
    expect(restored.historyStatus).toBe("loaded");
  });

  it("adopts the persona on record but never overwrites one just chosen", () => {
    const adopted = applyRestoredHistory(
      { ...emptyChatLane, historyStatus: "loading" },
      projection([message("hello")], "bmad-agent-architect"),
    );
    expect(adopted.agentCode).toBe("bmad-agent-architect");

    // The user bound an agent while the read was out. Their choice is newer.
    const chosen = applyRestoredHistory(
      { ...emptyChatLane, agentCode: "bmad-agent-analyst", historyStatus: "loading" },
      projection([message("hello")], null),
    );
    expect(chosen.agentCode).toBe("bmad-agent-analyst");
  });

  it("leaves a turn in flight strictly alone", () => {
    // The read was dispatched when the lane was empty and came back after the
    // user asked something. The host's answer predates that question: writing
    // it would drop the pending state, and the reply landing next would append
    // a turn the restored transcript may already hold.
    const inFlight = {
      ...emptyChatLane,
      sending: true,
      pending: "what changed here?",
      historyStatus: "loading" as const,
    };

    const applied = applyRestoredHistory(inFlight, projection([message("older turn")]));

    expect(applied.messages).toEqual([]);
    expect(applied.sending).toBe(true);
    expect(applied.pending).toBe("what changed here?");
    // Still marked read: the question was asked and answered, and asking again
    // on every render would be a loop.
    expect(applied.historyStatus).toBe("loaded");
  });

  it("never replaces messages the lane acquired while the read was out", () => {
    const live = {
      ...emptyChatLane,
      messages: [message("just sent"), message("just answered", "assistant")],
      historyStatus: "loading" as const,
    };

    const applied = applyRestoredHistory(live, projection([message("older turn")]));

    expect(applied.messages.map(({ text }) => text)).toEqual(["just sent", "just answered"]);
    expect(applied.historyStatus).toBe("loaded");
  });

  it("does not erase a dispatch exchange, which exists only in the renderer", () => {
    // `appendDispatchExchange` writes two messages the host has no record of,
    // so a restore on top of them would simply delete them.
    const dispatched = appendDispatchExchange(emptyChatLane, "run CA", "Architecture", 1_700);
    expect(dispatched.historyStatus).toBe("loaded");

    expect(beginHistoryRestore(setLane(emptyChatSessionStore, "task-a", dispatched), "task-a"))
      .toBeNull();
  });

  it("says a failed read failed instead of leaving the lane silently empty", () => {
    const failed = failHistoryRestore(
      { ...emptyChatLane, historyStatus: "loading" },
      "The stored conversation could not be read.",
    );

    expect(failed.error).toBe("The stored conversation could not be read.");
    expect(failed.historyStatus).toBe("unavailable");

    let store = setLane(emptyChatSessionStore, "task-a", failed);
    // A failed lane is not retried on its own — that would be an unbounded
    // loop against a failing host — but the user can ask for another attempt.
    expect(beginHistoryRestore(store, "task-a")).toBeNull();

    store = retryHistoryRestore(store, "task-a");

    expect(laneForSession(store, "task-a").error).toBeNull();
    expect(beginHistoryRestore(store, "task-a")).not.toBeNull();
  });

  it("does not overwrite a live lane's error when its late read fails", () => {
    const live = { ...emptyChatLane, messages: [message("kept")], historyStatus: "loading" as const };

    const failed = failHistoryRestore(live, "The stored conversation could not be read.");

    expect(failed.messages.map(({ text }) => text)).toEqual(["kept"]);
    expect(failed.error).toBeNull();
  });

  it("makes eviction a cache miss rather than data loss", () => {
    // The reported defect. A lane pushed out by the twenty-lane bound used to
    // be gone from the UI while the host still held the conversation; now it
    // reads back as never-asked, so activating that tab re-reads it.
    let store = emptyChatSessionStore;
    store = setLane(store, "task-evicted", {
      ...emptyChatLane,
      messages: [message("said out loud")],
      historyStatus: "loaded",
    });
    for (let index = 0; index < 25; index += 1) {
      store = setLane(store, `task-${index}`, { ...emptyChatLane, historyStatus: "loaded" });
    }

    expect(store.lanes.has("task-evicted")).toBe(false);
    expect(laneForSession(store, "task-evicted").historyStatus).toBe("unread");
    expect(beginHistoryRestore(store, "task-evicted")).not.toBeNull();
  });

  it("makes a workspace switch a cache miss too, and drops the read it raced", () => {
    let store = setLane(emptyChatSessionStore, "task-a", {
      ...emptyChatLane,
      messages: [message("in the old workspace")],
      historyStatus: "loaded",
    });
    const claimed = beginHistoryRestore(
      setLane(store, "task-b", emptyChatLane),
      "task-b",
    )!;
    store = claimed.store;

    store = clearAllLanes(store);

    expect(laneForSession(store, "task-a").historyStatus).toBe("unread");
    // The read issued against the revoked grant must not land in the lane that
    // replaced it.
    expect(routingIsCurrent(store, claimed.routing)).toBe(false);
  });
});

describe("appendDispatchExchange", () => {
  /// Regression guard: dispatching an agent menu item from the composer used to
  /// return without touching the lane, so the composer cleared and the message
  /// that opened the run vanished from the conversation.
  it("records the user's turn and says nothing has been sent", () => {
    const lane = appendDispatchExchange(
      emptyChatLane,
      "let's architect this",
      "Architecture",
      1_700,
    );

    expect(lane.messages).toHaveLength(2);
    expect(lane.messages[0]).toEqual({
      role: "user",
      text: "let's architect this",
      createdAt: 1_700,
    });
    expect(lane.messages[1]?.role).toBe("assistant");
    expect(lane.messages[1]?.text).toContain("Architecture");
    // The run is opened, not sent: the reply must not imply a model answered.
    expect(lane.messages[1]?.text).toContain("Nothing has been sent yet");
    expect(lane.pending).toBeNull();
    expect(lane.error).toBeNull();
  });

  it("appends after existing messages rather than replacing the transcript", () => {
    const seeded = {
      ...emptyChatLane,
      messages: [{ role: "user" as const, text: "hello", createdAt: 1 }],
      error: "stale",
    };
    const lane = appendDispatchExchange(seeded, "run CA", "Architecture", 2_000);

    expect(lane.messages).toHaveLength(3);
    expect(lane.messages[0]?.text).toBe("hello");
    expect(lane.error).toBeNull();
  });
});
