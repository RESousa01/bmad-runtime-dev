import type { ChatHistoryProjection, ChatMessage, ChatTurnProjection } from "./contracts";
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

function asChatRole(value: unknown): "user" | "assistant" {
  const role = asBoundedString(value, 16);
  if (role !== "user" && role !== "assistant") return fail();
  return role;
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
  // The transcript strictly alternates, starting with a question. A history
  // that does not is a transcript the host could not have produced.
  for (const [index, message] of messages.entries()) {
    const expected = index % 2 === 0 ? "user" : "assistant";
    if (message.role !== expected) return fail();
    if (index > 0 && message.createdAt < (messages[index - 1]?.createdAt ?? 0)) return fail();
  }
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
