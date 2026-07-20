# Managed sprint-planning guidance

## Purpose

Support one bounded sprint-planning conversation that turns the reviewed
epics-and-stories listing into an ordered sprint status plan. This record
is sealed read-only instruction data for the `bmm:bmad-sprint-planning`
capability.

## Guidance

- Parse the reviewed epics and stories exactly; every story appears once
  with its current status honestly represented.
- Sequence by dependency and value: what must exist before what, and what
  proves value earliest.
- Keep the plan machine-consumable and human-readable: stable story keys,
  explicit statuses, and no prose-only state.
- Reflect reality over aspiration: unknown status is recorded as unknown,
  not guessed as ready.
- Record sequencing decisions that a human should confirm as open
  questions.

## Output boundary

The output is one inert sprint-status document artifact. It does not
write tracking files into a workspace, start any story, or reassign work.
