import { describe, expect, it } from "vitest";

import type { ChatMessage, ChatRole } from "../../lib/hostClient";
import {
  buildTimeline,
  hasOutstandingToolCall,
  toolResultPreview,
  toolResultPreviewChars,
} from "./timelineModel";

let clock = 0;
function message(role: ChatRole, text: string): ChatMessage {
  clock += 1;
  return { createdAt: clock, role, text };
}

describe("buildTimeline", () => {
  it("keeps ordinary messages in order", () => {
    const entries = buildTimeline([
      message("user", "explain this"),
      message("assistant", "here you go"),
    ]);
    expect(entries.map((entry) => entry.kind)).toEqual(["user", "assistant"]);
  });

  it("folds a call and its result into one entry", () => {
    // One event to the user: the app went and did something, and this is what
    // came back. Two entries would read as two things happening.
    const entries = buildTimeline([
      message("tool_call", "read src/App.tsx"),
      message("tool_result", "export function App() {"),
    ]);
    expect(entries).toHaveLength(1);
    expect(entries[0]?.kind).toBe("tool");
    expect(entries[0]).toMatchObject({ result: { text: "export function App() {" } });
  });

  it("leaves a call outstanding until its result arrives", () => {
    const entries = buildTimeline([message("tool_call", "read src/App.tsx")]);
    expect(entries[0]).toMatchObject({ kind: "tool", result: null });
    expect(hasOutstandingToolCall(entries)).toBe(true);
  });

  /// The lookahead this replaces told both calls they were still waiting,
  /// because neither was immediately followed by a result.
  it("answers two back-to-back calls in the order they were made", () => {
    const entries = buildTimeline([
      message("tool_call", "read a.ts"),
      message("tool_call", "read b.ts"),
      message("tool_result", "contents of a"),
      message("tool_result", "contents of b"),
    ]);

    expect(entries).toHaveLength(2);
    expect(entries[0]).toMatchObject({
      call: { text: "read a.ts" },
      result: { text: "contents of a" },
    });
    expect(entries[1]).toMatchObject({
      call: { text: "read b.ts" },
      result: { text: "contents of b" },
    });
    expect(hasOutstandingToolCall(entries)).toBe(false);
  });

  it("pairs across an assistant message that lands between them", () => {
    // The host can narrate while a call is outstanding. Adjacency is not what
    // makes a result belong to a call.
    const entries = buildTimeline([
      message("tool_call", "read a.ts"),
      message("assistant", "looking at that now"),
      message("tool_result", "contents of a"),
    ]);
    expect(entries.map((entry) => entry.kind)).toEqual(["tool", "assistant"]);
    expect(entries[0]).toMatchObject({ result: { text: "contents of a" } });
    expect(hasOutstandingToolCall(entries)).toBe(false);
  });

  /**
   * The failure that motivated this module.
   *
   * A retained transcript is truncated to its last N messages, so a slice can
   * begin with a result whose call was cut off. The lookahead read that as a
   * call still awaiting a result -- the app claiming work is in flight that
   * finished before the slice even starts.
   */
  it("keeps a result whose call was truncated away", () => {
    const entries = buildTimeline([
      message("tool_result", "contents of a"),
      message("assistant", "here is what I found"),
    ]);

    expect(entries.map((entry) => entry.kind)).toEqual(["orphan_result", "assistant"]);
    // And nothing is claimed to be in flight.
    expect(hasOutstandingToolCall(entries)).toBe(false);
  });

  it("does not let an orphan result answer a later call", () => {
    // An orphan belongs to a call that is gone. Handing it to the next call
    // would attach output to a request that did not produce it.
    const entries = buildTimeline([
      message("tool_result", "from a call that was truncated"),
      message("tool_call", "read b.ts"),
    ]);

    expect(entries[0]?.kind).toBe("orphan_result");
    expect(entries[1]).toMatchObject({ kind: "tool", result: null });
  });

  it("handles an empty transcript", () => {
    expect(buildTimeline([])).toEqual([]);
  });

  it("keeps every message it was given", () => {
    // Nothing is dropped on the floor, whatever the ordering. A transcript
    // that silently loses a message is worse than one that renders it oddly.
    const messages = [
      message("tool_result", "orphan"),
      message("user", "hello"),
      message("tool_call", "read a"),
      message("assistant", "one moment"),
      message("tool_result", "contents"),
      message("tool_call", "read b"),
    ];
    const entries = buildTimeline(messages);
    const accounted = entries.flatMap((entry) => {
      switch (entry.kind) {
        case "tool": return entry.result === null ? [entry.call] : [entry.call, entry.result];
        case "orphan_result": return [entry.result];
        default: return [entry.message];
      }
    });
    expect(accounted).toHaveLength(messages.length);
    expect(new Set(accounted.map((m) => m.createdAt)).size).toBe(messages.length);
  });
});

describe("toolResultPreview", () => {
  it("takes the first line", () => {
    expect(toolResultPreview("first line\nsecond line")).toBe("first line");
  });

  it("trims it", () => {
    expect(toolResultPreview("   padded   \nrest")).toBe("padded");
  });

  it("bounds a long first line", () => {
    const long = "x".repeat(toolResultPreviewChars + 40);
    const preview = toolResultPreview(long);
    expect(preview).toHaveLength(toolResultPreviewChars + 1);
    expect(preview.endsWith("…")).toBe(true);
  });

  it("does not mark a line that fits", () => {
    expect(toolResultPreview("short").endsWith("…")).toBe(false);
  });

  it("handles an empty result", () => {
    expect(toolResultPreview("")).toBe("");
  });
});
