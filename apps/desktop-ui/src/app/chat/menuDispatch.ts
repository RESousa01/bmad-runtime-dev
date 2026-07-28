/**
 * Deciding whether a message dispatches an agent menu item instead of being
 * sent as an ordinary turn.
 *
 * FE-10 step 3. With an agent bound, a message that unambiguously names one of
 * that agent's menu items opens the corresponding capability run — the
 * behaviour the method's own activation describes. Anything else stays an
 * ordinary conversation.
 *
 * Three conditions, all required, and each of them is a way to get this wrong:
 *
 *   - an agent must be bound. Plain chat has no menus, so nothing to match.
 *   - the library must have loaded. Matching against a catalogue that has not
 *     arrived silently turns every message into ordinary chat, which looks
 *     identical to the feature not existing.
 *   - the matched menu item must carry a `capabilityId`. A menu entry without
 *     one is a label with nothing behind it, and dispatching it would open a
 *     run for a capability the host never declared.
 *
 * `matchMenuIntent` itself is deliberately conservative and returns null on
 * ambiguity; this module only decides whether to consult it and what a match
 * means.
 */

import { matchMenuIntent } from "../../lib/menuIntent";
import type { BmadLibraryUiState } from "../../lib/bmadProjection";

export interface MenuDispatch {
  readonly capabilityId: string;
  /** The words shown to the user, which the transcript records. */
  readonly label: string;
}

export interface MenuDispatchInput {
  readonly intent: string;
  /** The agent bound to this conversation, or null for plain chat. */
  readonly boundAgent: string | null;
  readonly library: BmadLibraryUiState;
}

/**
 * The menu item this message dispatches, or null for an ordinary turn.
 *
 * Null is the common answer and the safe one: a message that does not clearly
 * name a menu item is a message, and treating it as a command would take the
 * user somewhere they did not ask to go.
 */
export function resolveMenuDispatch({
  boundAgent,
  intent,
  library,
}: MenuDispatchInput): MenuDispatch | null {
  if (boundAgent === null || library.kind !== "ready") return null;

  const agent = library.projection.methodAgents.find(
    (candidate) => candidate.agentCode === boundAgent,
  );
  if (agent === undefined) return null;

  const matched = matchMenuIntent(intent, agent.menus);
  if (matched === null) return null;

  // `== null` covers both null and undefined: the projection may omit the
  // field entirely for a menu entry that only prints text.
  const capabilityId = matched.menu.capabilityId;
  if (capabilityId == null) return null;

  return { capabilityId, label: matched.menu.displayLabel };
}
