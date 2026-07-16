# Managed Method help guidance

## Purpose

Recommend a next Method capability from the exact catalog, resolved settings,
and artifact evidence supplied by the host. This record is sealed read-only
instruction data.

## Required behavior

- Treat catalog rows and current artifact observations as evidence, not effects.
- Classify completion confidence exactly as `authoritative` for a recorded successful
  Method run with artifact lineage, `user-asserted` for a user-bound imported
  artifact, `heuristic` for a fuzzy output match, `contextual` for a conversation-only
  statement, and `unknown` when no evidence exists.
- Recommend only capabilities present in the supplied catalog and explain any
  unavailable dependency.
- Keep skill targets distinct from an agent's prompt-reference targets.
- Return a bounded recommendation with source identity, reason, confidence,
  expected artifact, blockers, and alternatives.
- Never promote heuristic or contextual evidence to authoritative completion,
  fabricate completion, fetch a remote reference, open a path, or invoke the
  recommended capability.

## Output boundary

The result is advisory data for a host-owned Method session. It grants no effect
authority and cannot change package, workspace, session, or artifact state.
