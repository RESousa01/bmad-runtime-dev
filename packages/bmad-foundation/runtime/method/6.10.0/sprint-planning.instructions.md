# Managed sprint-planning guidance

## Purpose

Support one bounded sprint-planning conversation that turns the reviewed
epics-and-stories listing into an ordered sprint status plan. This record
is sealed read-only instruction data for the `bmm:bmad-sprint-planning`
capability.

## The status vocabulary is closed

This plan is the machine contract joining sprint planning to story creation,
implementation, and review. Downstream skills read these exact values, so use
them and no others — never invent a status such as "unknown" to cover a gap.

- **Epic** — `backlog` (not yet started), `in-progress` (actively worked on),
  `done` (all its stories completed).
- **Story** — `backlog` (exists only in the epic file), `ready-for-dev` (story
  file created), `in-progress` (implementation under way), `review`
  (implementation complete, awaiting review), `done`.
- **Retrospective** — `optional` (may be completed, not required), `done`.
- **Action item** — `open` (committed during a retrospective, not yet
  addressed), `in-progress`, `done`.

Keys are equally part of the contract: `epic-N` for an epic, `N-M-slug` for a
story, `epic-N-retrospective` for a retrospective.

If a status genuinely cannot be determined from the reviewed context, do not
coin a value for it. Leave the entry at the most conservative status the
evidence supports — `backlog` for a story you cannot show exists as a file —
and raise the ambiguity as an explicit open question for a human to settle.

## Sequencing

- Parse the reviewed epics and stories exactly; every story appears once, with
  its status honestly represented.
- Sequence by dependency and value: what must exist before what, and what
  proves value earliest.
- An epic becomes `in-progress` when work starts on its first story.
- The next story is normally created only after the previous one is `done`, so
  its learnings can be carried in.
- A story moves to `review` after implementation, and review runs in fresh
  context rather than continuing the implementing conversation.
- Keep the plan machine-consumable and human-readable: stable keys, explicit
  statuses, no prose-only state.
- Record sequencing decisions a human should confirm as open questions.

## Output boundary

The output is one inert sprint-status document artifact. It does not
write tracking files into a workspace, start any story, or reassign work.
