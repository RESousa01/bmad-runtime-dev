import { describe, expect, it } from "vitest";

import {
  bumpGeneration,
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
