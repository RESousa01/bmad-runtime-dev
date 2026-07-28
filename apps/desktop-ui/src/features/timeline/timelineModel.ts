/**
 * A transcript, as a structure rather than a flat list.
 *
 * FE-19. The host hands back `ChatMessage[]` with four roles in it, two of
 * which -- `tool_call` and `tool_result` -- are halves of one thing. The
 * transcript paired them inline with `messages[index + 1]?.role !==
 * "tool_result"`, which is a lookahead standing in for a data structure, and it
 * is wrong in two ways that only show up on real transcripts:
 *
 *   - a retained transcript is truncated to its last N messages, so a slice can
 *     begin with a result whose call was cut off. The lookahead never considers
 *     that case, and the orphaned result renders as a call awaiting a result --
 *     the app claiming work is in flight that finished long ago;
 *   - two calls issued back to back are both told they are awaiting a result,
 *     because neither is immediately followed by one.
 *
 * Pairing them once, here, means every consumer sees the same structure and the
 * question "does this call have its result" has one answer.
 */

import type { ChatMessage } from "../../lib/hostClient";

/**
 * One item in the timeline.
 *
 * A tool call and its result are one entry, because they are one event to the
 * user: the app went and did something, and this is what came back.
 */
export type TimelineEntry =
  | { readonly kind: "user"; readonly message: ChatMessage }
  | { readonly kind: "assistant"; readonly message: ChatMessage }
  | {
    readonly kind: "tool";
    readonly call: ChatMessage;
    /** Null while the call is still outstanding. */
    readonly result: ChatMessage | null;
  }
  /**
   * A result whose call is not in this slice.
   *
   * Kept rather than dropped. The call was truncated away, not cancelled, and
   * silently discarding what came back would hide work the app actually did.
   */
  | { readonly kind: "orphan_result"; readonly result: ChatMessage };

/**
 * Pairs each tool call with the result that answers it.
 *
 * The result is matched by *order among tool messages*, not by adjacency: the
 * host emits results in the order the calls were made, and an assistant message
 * can land between a call and its result.
 */
export function buildTimeline(
  messages: readonly ChatMessage[],
): readonly TimelineEntry[] {
  // Queue of calls still waiting, oldest first. A result answers the oldest
  // outstanding call, which is what "in the order the calls were made" means.
  const outstanding: number[] = [];
  const entries: TimelineEntry[] = [];

  for (const message of messages) {
    switch (message.role) {
      case "user":
        entries.push({ kind: "user", message });
        break;
      case "assistant":
        entries.push({ kind: "assistant", message });
        break;
      case "tool_call":
        outstanding.push(entries.length);
        entries.push({ call: message, kind: "tool", result: null });
        break;
      case "tool_result": {
        const index = outstanding.shift();
        if (index === undefined) {
          entries.push({ kind: "orphan_result", result: message });
          break;
        }
        const pending = entries[index];
        // Narrowing, not a possibility: the index was recorded when the entry
        // was pushed. The check is here because the alternative is a cast.
        if (pending?.kind === "tool") {
          entries[index] = { call: pending.call, kind: "tool", result: message };
        }
        break;
      }
    }
  }

  return entries;
}

/** Whether anything in this timeline is still waiting on the host. */
export function hasOutstandingToolCall(entries: readonly TimelineEntry[]): boolean {
  return entries.some((entry) => entry.kind === "tool" && entry.result === null);
}

/**
 * How much of a tool result the collapsed summary shows.
 *
 * A result can be 8 KiB, so the body stays behind a disclosure and the
 * transcript stays readable.
 */
const TOOL_RESULT_PREVIEW_CHARS = 96;

/**
 * The first line of a result, bounded.
 *
 * First line rather than first N characters: tool output is overwhelmingly
 * line-oriented, and a preview cut mid-word across a line break reads as
 * corruption rather than as truncation.
 */
export function toolResultPreview(
  text: string,
  limit: number = TOOL_RESULT_PREVIEW_CHARS,
): string {
  const firstLine = (text.split("\n", 1)[0] ?? "").trim();
  return firstLine.length > limit ? `${firstLine.slice(0, limit)}…` : firstLine;
}

export const toolResultPreviewChars = TOOL_RESULT_PREVIEW_CHARS;
