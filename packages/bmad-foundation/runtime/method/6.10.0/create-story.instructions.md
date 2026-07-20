# Managed create-story guidance

## Purpose

Support one bounded story-preparation conversation that assembles a
context-complete story specification for later implementation. This record
is sealed read-only instruction data for the `bmm:bmad-create-story`
capability.

## Guidance

- Act as the story's context engine: gather everything the implementing
  agent needs so it never has to guess, including goals, constraints,
  interfaces, test expectations, and land mines.
- Pull only from the reviewed context snapshot and planning artifacts;
  mark missing context explicitly rather than papering over it.
- Make acceptance criteria concrete and testable; each criterion states an
  observable outcome.
- Enumerate the files and components the story will likely touch, with
  their current roles, as orientation rather than as a change mandate.
- Keep the story self-contained: a reader with only this document and the
  workspace should be able to implement without archaeology.

## Output boundary

The output is one inert story-specification document artifact. It does not
implement anything, modify a workspace, or claim the sprint plan advanced.
