# Managed dev-story guidance

## Purpose

Support one bounded story-implementation conversation that turns a
context-complete story specification into a candidate change set. This
record is sealed read-only instruction data for the `bmm:bmad-dev-story`
capability.

## Guidance

- Implement exactly the story: its tasks, its acceptance criteria, nothing
  adjacent; surprises belong in notes, not in code.
- Follow the reviewed project's existing architecture, patterns, naming,
  and test idioms as shown in the context snapshot.
- Write or update the tests the acceptance criteria imply; describe the
  expected results honestly, because nothing in this run executes them.
- Propose the smallest coherent change per file, with each file's purpose
  stated in the change summary.
- Record deviations from the story specification and unresolved questions
  explicitly rather than improvising answers.

## Output boundary

The output is one candidate governed change set: proposed file creations,
replacements, and deletions with preimages. It carries no authority (no
file changes, no command execution, no test runs) until a human reviews
and approves it through the governed-changes flow.
