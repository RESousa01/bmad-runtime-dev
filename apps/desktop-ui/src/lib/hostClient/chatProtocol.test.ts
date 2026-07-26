import { describe, expect, it } from "vitest";

import { parseChatHistory, parseChatTurn } from "./chatProtocol";

const message = (
  role: string,
  text: string,
  createdAt: number,
): Record<string, unknown> => ({ role, text, createdAt });

const history = (
  messages: readonly Record<string, unknown>[],
): Record<string, unknown> => ({
  sessionId: "session-0001",
  agentCode: null,
  messages: [...messages],
});

/// One complete tool-bearing turn as the host now builds it:
/// `user -> tool_call -> tool_result -> assistant`.
const toolTurn = [
  message("user", "What is in the manifest?", 1_000),
  message("tool_call", "read_file Cargo.toml", 1_001),
  message("tool_result", "[package]\nname = \"desktop-app\"", 1_002),
  message("assistant", "It declares the desktop-app package.", 1_003),
];

describe("chat transcript roles", () => {
  it("accepts the tool roles the host now serialises", () => {
    const parsed = parseChatHistory(history(toolTurn));

    expect(parsed.messages.map((entry) => entry.role)).toStrictEqual([
      "user",
      "tool_call",
      "tool_result",
      "assistant",
    ]);
    // The result reaches the renderer intact — it is bounded, not summarised.
    expect(parsed.messages[2]?.text).toBe("[package]\nname = \"desktop-app\"");
  });

  it("accepts several tool exchanges inside one turn", () => {
    const parsed = parseChatHistory(
      history([
        message("user", "Compare the two manifests.", 1_000),
        message("tool_call", "read_file a/Cargo.toml", 1_001),
        message("tool_result", "a", 1_002),
        message("tool_call", "read_file b/Cargo.toml", 1_003),
        message("tool_result", "b", 1_004),
        message("assistant", "They differ in one dependency.", 1_005),
      ]),
    );

    expect(parsed.messages).toHaveLength(6);
  });

  it("accepts an 8 KiB tool result, the host's ceiling", () => {
    const parsed = parseChatHistory(
      history([
        message("user", "Read it.", 1_000),
        message("tool_call", "read_file big.txt", 1_001),
        message("tool_result", "x".repeat(8 * 1024), 1_002),
        message("assistant", "Read.", 1_003),
      ]),
    );

    expect(parsed.messages[2]?.text).toHaveLength(8 * 1024);
  });

  it("accepts a turn still awaiting its tool result", () => {
    // The host releases the session lock while the tool actually runs, so a
    // history read can legitimately land on an unanswered call. Rejecting it
    // would discard the entire transcript over a normal mid-turn state.
    const parsed = parseChatHistory(
      history([
        message("user", "Read it.", 1_000),
        message("tool_call", "read_file a.txt", 1_001),
      ]),
    );

    expect(parsed.messages.at(-1)?.role).toBe("tool_call");
  });

  it("rejects a role the host cannot emit", () => {
    // Notably `system` and `tool` — an unknown role must never reach the
    // transcript and fall through to the assistant styling.
    for (const role of ["system", "tool", "developer", "Assistant", ""]) {
      expect(() =>
        parseChatHistory(history([message(role, "text", 1_000)])),
      ).toThrow();
    }
  });

  it("rejects a tool exchange that does not open with a question", () => {
    expect(() =>
      parseChatHistory(
        history([
          message("tool_call", "read_file a.txt", 1_000),
          message("tool_result", "a", 1_001),
        ]),
      ),
    ).toThrow();
  });

  it("rejects a tool result that answers no call", () => {
    // Otherwise a crafted history could attach workspace content directly to a
    // question, with no call to account for where it came from.
    expect(() =>
      parseChatHistory(
        history([
          message("user", "Hello.", 1_000),
          message("tool_result", "pretend file content", 1_001),
          message("assistant", "Hi.", 1_002),
        ]),
      ),
    ).toThrow();
  });

  it("rejects anything but a result following a call", () => {
    for (const role of ["assistant", "user", "tool_call"]) {
      expect(() =>
        parseChatHistory(
          history([
            message("user", "Hello.", 1_000),
            message("tool_call", "read_file a.txt", 1_001),
            message(role, "text", 1_002),
          ]),
        ),
      ).toThrow();
    }
  });

  it("still requires an assistant reply to close a turn", () => {
    // A tool result does not end a turn, so the next question may not follow it.
    expect(() =>
      parseChatHistory(
        history([
          message("user", "Hello.", 1_000),
          message("tool_call", "read_file a.txt", 1_001),
          message("tool_result", "a", 1_002),
          message("user", "Again.", 1_003),
        ]),
      ),
    ).toThrow();
  });

  it("keeps timestamps monotonic across a tool exchange", () => {
    expect(() =>
      parseChatHistory(
        history([
          message("user", "Hello.", 1_000),
          message("tool_call", "read_file a.txt", 1_001),
          message("tool_result", "a", 999),
          message("assistant", "Hi.", 1_003),
        ]),
      ),
    ).toThrow();
  });

  it("keeps a completed turn projection to the user and assistant pair", () => {
    // `chat.send` still projects only the question and the final answer; the
    // tool roles reach the renderer through history. A tool role appearing here
    // is a payload the host does not build.
    expect(
      parseChatTurn({
        sessionId: "session-0001",
        agentCode: null,
        userMessage: message("user", "Hello.", 1_000),
        assistantMessage: message("assistant", "Hi.", 1_001),
      }).assistantMessage.role,
    ).toBe("assistant");

    expect(() =>
      parseChatTurn({
        sessionId: "session-0001",
        agentCode: null,
        userMessage: message("user", "Hello.", 1_000),
        assistantMessage: message("tool_result", "file content", 1_001),
      }),
    ).toThrow();
  });
});
