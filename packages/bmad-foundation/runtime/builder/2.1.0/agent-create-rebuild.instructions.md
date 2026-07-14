# Managed stateless-agent create or rebuild guidance

## Purpose

Draft an inactive stateless agent from a bounded authoring request and reviewed
context. The host owns identity, limits, revision storage, and all policy.

## Guidance

- Define a precise role, mission, activation description, and finite capability set.
- Keep every internal prompt capability reachable exactly once from its table entry.
- Limit output to the host-approved stateless file inventory and metadata fields.
- Make customization optional, bounded, and free of side effects.
- Preserve the required first-party quality canon as a distinct managed reference.
- Exclude memory, self-evolution, background behavior, installed dependencies, and
  any claim that the draft is active or validated.

## Output boundary

Return only proposed UTF-8 files for an immutable inactive draft revision. The
proposal has no workspace or lifecycle effect.
