import { describe, expect, it } from "vitest";

import {
  appendDispatchExchange,
  bumpGeneration,
  dropLane,
  emptyChatLane,
  emptyChatSessionStore,
  generationForSession,
  laneForSession,
  routingIsCurrent,
  setLane,
} from "./chatSessions";
import type { ChatMessage } from "../lib/hostClient";

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
