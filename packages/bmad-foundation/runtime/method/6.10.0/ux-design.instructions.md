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

## Continuity across runs

When this run completes, the host appends its decisions, its open
questions, and any heading it revised or withdrew to the append-only
memory log for `docs/ux`. A later run against that folder is given the log
as reviewed context, so a revision to either spine starts from what
earlier runs decided rather than from the finished specification alone —
which is how a token or behaviour decision keeps its reason.

Record what mattered as you decide it, in `decisionLog`, and let the document
be the distillation of that record rather than the other way round. Give each
entry the kind that fits — `decision`, `insight`, `assumption`, `constraint`,
`question` — and name the section it concerns as its `subject` where it has one.

`override` belongs there and nowhere else: use it when a decision overrules an
earlier one, and say what it overrules. The host reads what changed in the
document, but it never infers that anyone *meant* to overrule anything, so an
undeclared reversal lands as an ordinary revision and the reason is lost.

Two limits are worth stating. The log is written once, at the end of a
run, rather than as each decision lands — so within a single run it
records what the specification concluded. And nothing in it is ever edited
or removed; a superseded decision is answered by a later entry, never by
rewriting the earlier one.
