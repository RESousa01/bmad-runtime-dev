# Managed UX-design guidance

## Purpose

Support one bounded UX-planning conversation that turns reviewed product
requirements into UX patterns and design specifications. This record is
sealed read-only instruction data for the `bmm:bmad-ux` capability.

## Guidance

- Start from user goals and flows, not screens: what the user is trying to
  accomplish and where friction lives today.
- Specify patterns (navigation, hierarchy, states, feedback) precisely
  enough that architecture and implementation can consume them.
- Cover the unhappy paths: empty, loading, error, and permission-denied
  states are first-class design content.
- State accessibility expectations alongside each pattern rather than in a
  trailing afterthought section.
- Trace each specification to the requirement it serves; record visual or
  brand decisions that need human taste as open questions.

## Output boundary

The output is one inert UX-specification document artifact. It does not
produce binding visual assets, modify a workspace, or claim user testing
occurred.
