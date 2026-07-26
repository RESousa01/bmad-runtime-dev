import type {
  ChatHistoryProjection,
  ChatMessage,
  ChatRole,
  ChatTurnProjection,
} from "./contracts";
import {
  asBoundedString,
  asContractId,
  assertExactKeys,
  asRecord,
  asTextContent,
  asUnsignedInteger,
  fail,
} from "./validation";
import { parseDispatchReply } from "./workspaceProtocol";

/// Mirrors the host's per-message ceiling. A transcript message is multi-line
/// by nature, so it is bounded as text content rather than a single line.
const MAX_MESSAGE_BYTES = 32_768;
const MAX_TRANSCRIPT_MESSAGES = 200;
/// Only the six sealed Method agent codes may wear a persona. Anything else —
/// a Builder id, a path, a crafted string — is rejected rather than shown.
const AGENT_CODE_PATTERN = /^bmad-agent-[a-z][a-z0-9-]{0,48}$/u;

/// Mirrors the host's `ChatRole` exactly. A role the host cannot emit is a
/// payload the host did not produce, so it is rejected rather than coerced to
/// something renderable — an unknown role must never fall through to the
/// assistant styling.
const CHAT_ROLES: ReadonlySet<string> = new Set<ChatRole>([
  "user",
  "assistant",
  "tool_call",
  "tool_result",
]);

function asChatRole(value: unknown): ChatRole {
  const role = asBoundedString(value, 16);
  if (!CHAT_ROLES.has(role)) return fail();
  return role as ChatRole;
}

/// Which roles may follow `previous`, mirroring `ChatSession`'s append rules.
///
/// A turn is `user -> (tool_call -> tool_result)* -> assistant`, so this is no
/// longer simple alternation. Two invariants are load-bearing rather than
/// cosmetic: only an assistant reply closes a turn, and a `tool_call` may be
/// followed by nothing except its `tool_result`. Enforcing the second is what
/// stops a crafted history from showing a call whose answer is really the next
/// turn's content.
function successorsOf(previous: ChatRole): ReadonlySet<ChatRole> {
  switch (previous) {
    // A closed turn can only be followed by the next question.
    case "assistant":
      return new Set<ChatRole>(["user"]);
    // An outstanding call is answered before anything else may happen.
    case "tool_call":
      return new Set<ChatRole>(["tool_result"]);
    // Mid-turn: the model either asks for another tool or answers.
    case "user":
    case "tool_result":
      return new Set<ChatRole>(["assistant", "tool_call"]);
  }
}

export function asAgentCode(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  const code = asBoundedString(value, 64);
  if (!AGENT_CODE_PATTERN.test(code)) return fail();
  return code;
}

function asChatMessage(value: unknown): ChatMessage {
  const message = asRecord(value);
  assertExactKeys(message, ["role", "text", "createdAt"]);
  const text = asTextContent(message.text, MAX_MESSAGE_BYTES);
  if (text.trim().length === 0) return fail();
  return {
    role: asChatRole(message.role),
    text,
    createdAt: asUnsignedInteger(message.createdAt),
  };
}

export function parseChatTurn(value: unknown): ChatTurnProjection {
  const turn = asRecord(value);
  assertExactKeys(turn, ["sessionId", "agentCode", "userMessage", "assistantMessage"]);
  const userMessage = asChatMessage(turn.userMessage);
  const assistantMessage = asChatMessage(turn.assistantMessage);
  // A turn is one exchange: the question then its answer, in that order.
  if (userMessage.role !== "user" || assistantMessage.role !== "assistant") return fail();
  if (assistantMessage.createdAt < userMessage.createdAt) return fail();
  return {
    sessionId: asContractId(turn.sessionId),
    agentCode: asAgentCode(turn.agentCode),
    userMessage,
    assistantMessage,
  };
}

export function parseChatTurnReply(
  value: unknown,
  requestId: string,
): { projection: ChatTurnProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "chat_turn") return fail();
  return { projection: parseChatTurn(parsed.data.value), sequence: parsed.sequence };
}

export function parseChatHistory(value: unknown): ChatHistoryProjection {
  const history = asRecord(value);
  assertExactKeys(history, ["sessionId", "agentCode", "messages"]);
  if (!Array.isArray(history.messages) || history.messages.length > MAX_TRANSCRIPT_MESSAGES) {
    return fail();
  }
  const messages = history.messages.map(asChatMessage);
  // Every turn opens with a question and runs `user -> (tool_call ->
  // tool_result)* -> assistant`. A history that does not is a transcript the
  // host could not have produced.
  for (const [index, message] of messages.entries()) {
    const previous = messages[index - 1];
    if (previous === undefined) {
      // Only a question opens a transcript.
      if (message.role !== "user") return fail();
      continue;
    }
    if (!successorsOf(previous.role).has(message.role)) return fail();
    if (message.createdAt < previous.createdAt) return fail();
  }
  // A trailing `tool_call` is deliberately accepted. The host releases the
  // session lock while the tool actually runs, so a history read that lands in
  // that window legitimately ends on an unanswered call. Rejecting it would
  // throw away the whole transcript over a normal mid-turn state; the
  // transcript renders it as still awaiting its result instead.
  return {
    sessionId: asContractId(history.sessionId),
    agentCode: asAgentCode(history.agentCode),
    messages,
  };
}

export function parseChatHistoryReply(
  value: unknown,
  requestId: string,
): { projection: ChatHistoryProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "chat_history") return fail();
  return { projection: parseChatHistory(parsed.data.value), sequence: parsed.sequence };
}
