# Managed product-brief guidance

## Purpose

Support one bounded guided product-brief conversation whose state and
artifact shape are owned by the host. This record is sealed read-only
instruction data for the `bmm:bmad-product-brief` capability.

## Length, and where the rest goes

Aim for one to two pages. A brief that runs longer has stopped being a brief:
the extra detail belongs in an **addendum** alongside it, not in the brief
itself. Downstream consumers read the brief to orient, so its shape carries
meaning — keep it coherent and let the addendum hold the depth.

When closing, account explicitly for everything of substance the conversation
produced: what landed in the brief, what belongs in the addendum, and what was
process noise set aside deliberately. An unaccounted-for decision is a decision
lost. Audit and override information stays in the run's decision record, never
in the addendum.

## Guidance

- Nail the idea down through an expert-guided sequence: problem, audience,
  value, differentiation, scope edges, and success signals.
- Ask one sharp question at a time; reflect the user's answers back in
  tightened language they can accept or correct.
- Keep the brief honest: capture stated uncertainty as uncertainty, and
  scope exclusions as first-class content.
- Trace each element of the brief to the user statement or reviewed
  evidence that motivated it.
- Stay a brief: resist expanding into requirements, architecture, or
  planning content that later capabilities own.

## Output boundary

The output is one inert product-brief document artifact with sections,
evidence references, and open questions. It does not create a PRD, start
another capability, or modify a workspace.

## Continuity across runs

When this run completes, the host appends its decisions, its open
questions, and any heading it revised or withdrew to the append-only
memory log for `docs/product-brief`. A later run against that folder is
given the log as reviewed context, so a revision starts from what earlier
runs decided rather than from the finished brief alone. This is where the
closing account of what landed, what moved to the addendum, and what was
set aside actually survives.

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
records what the brief concluded. And nothing in it is ever edited or
removed; a superseded decision is answered by a later entry, never by
rewriting the earlier one.
