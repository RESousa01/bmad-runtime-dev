// Dispatching an agent's menu item straight from the conversation.
//
// The method's own source describes this as the agent's normal behaviour
// (`bmad-agent-architect/SKILL.md`):
//
//   "If the user's initial message already names an intent that clearly maps
//    to a menu item (e.g. 'hey Winston, let's architect this'), skip the menu
//    and dispatch that item directly after greeting."
//
// and, immediately after:
//
//   "When nothing on the menu fits, just continue the conversation."
//
// So this matcher is deliberately *conservative*: it dispatches only on an
// unambiguous match and otherwise returns nothing, letting the message be an
// ordinary chat turn. Guessing wrong here would hijack a conversation, which
// is exactly the failure the rework exists to remove.
//
// It grants no authority: it can only select a menu the host already projected
// with a host-validated `capabilityId`.

import type { BmadAgentMenuProjection } from "./bmadProjection";

/// Words that carry no intent and would otherwise make weak matches look
/// strong.
const STOP_WORDS = new Set([
  "a",
  "an",
  "and",
  "the",
  "to",
  "for",
  "of",
  "my",
  "our",
  "this",
  "that",
  "it",
  "lets",
  "let",
  "please",
  "can",
  "you",
  "we",
  "i",
  "do",
  "make",
  "create",
  "new",
]);

function tokens(value: string): string[] {
  return value
    .toLowerCase()
    .split(/[^a-z0-9]+/u)
    .filter((token) => token.length > 0);
}

function significantTokens(value: string): string[] {
  return tokens(value).filter((token) => token.length >= 3 && !STOP_WORDS.has(token));
}

export interface MenuIntentMatch {
  readonly menu: BmadAgentMenuProjection;
  /// Why it matched, for the confirmation the UI shows before running.
  readonly reason: "menu_code" | "label";
}

/// Finds the single menu item a message unambiguously names.
///
/// Returns `null` when nothing matches *or* when more than one item matches:
/// an ambiguous intent continues the conversation rather than picking one.
export function matchMenuIntent(
  message: string,
  menus: readonly BmadAgentMenuProjection[],
): MenuIntentMatch | null {
  const runnable = menus.filter(
    (menu) => menu.capabilityId !== null && menu.availability === "available",
  );
  if (runnable.length === 0) return null;
  const messageTokens = tokens(message);
  if (messageTokens.length === 0) return null;
  const messageSet = new Set(messageTokens);

  // An explicit menu code is the strongest signal, and the one upstream's
  // menus are addressed by. It must appear as a whole token, so "CA" does not
  // match inside "cascade".
  const byCode = runnable.filter((menu) => messageSet.has(menu.code.toLowerCase()));
  if (byCode.length === 1) return { menu: byCode[0]!, reason: "menu_code" };
  if (byCode.length > 1) return null;

  // Otherwise every significant word of the item's label must be present.
  const byLabel = runnable.filter((menu) => {
    const label = significantTokens(menu.displayLabel);
    return label.length > 0 && label.every((token) => messageSet.has(token));
  });
  return byLabel.length === 1 ? { menu: byLabel[0]!, reason: "label" } : null;
}
