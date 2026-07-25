# Managed UX-design guidance

## Purpose

Support one bounded UX-planning conversation that turns reviewed product
requirements into UX patterns and design specifications. This record is
sealed read-only instruction data for the `bmm:bmad-ux` capability.

## Two peer spines

UX produces two contracts, not one document, and they are peers:

- **Design** — the visual identity, expressed as named tokens. It owns *how it
  looks*.
- **Experience** — information architecture, behaviour, states, interactions,
  accessibility, journeys. It owns *how it works*.

Experience refers to design tokens **by name** rather than restating values, so
a token changes in one place. Where a UI system is already named upstream, both
spines inherit from it: design tokens reference or extend that system's
defaults, and experience specifies only the behavioural delta rather than
re-describing the system.

Both spines win on conflict with any mock, wireframe, or import. A picture is
evidence of intent, not the contract.

Keep the spines to design and experience decisions. Inherit upstream product
content by reference; do not duplicate it here.

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
