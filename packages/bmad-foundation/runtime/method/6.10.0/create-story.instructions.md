# Managed create-story guidance

## Purpose

Support one bounded story-preparation conversation that assembles a
context-complete story specification for later implementation. This record
is sealed read-only instruction data for the `bmm:bmad-create-story`
capability.

## Which story, and from where

The story comes from the **epic**: the next story still in `backlog`, taken in
the order the sprint plan and the epic's own sequence imply. Name which story
you are preparing and which epic it belongs to, using the `N-M-slug` key the
sprint plan uses, so the two artifacts stay joined.

Your purpose is **not to copy the epic**. The epic states the story, its
acceptance criteria in outline, and source hints. This run turns that into a
specification an implementer can work from without going back to the epic —
which means supplying what the epic left implicit, not restating what it said.

Once prepared, the story's place in the plan is `ready-for-dev`. This run does
not move it there; say so, and leave the transition to the sprint plan and the
human who owns it.

## Previous story intelligence

A story is rarely the first. Where the reviewed context includes the preceding
story, carry its learnings forward — that continuity is a large part of the
value here:

- development notes and anything discovered while implementing it;
- review feedback and corrections that were requested;
- the files it created or modified, and what role they now play;
- anything that would change how this story should be approached.

Say explicitly when no previous story was available, rather than leaving the
reader to assume you checked.

## The story itself

- Act as the story's context engine: gather everything the implementing
  agent needs so it never has to guess, including goals, constraints,
  interfaces, test expectations, and land mines.
- Pull only from the reviewed context snapshot and planning artifacts;
  mark missing context explicitly rather than papering over it.
- Make acceptance criteria concrete and testable; each criterion states an
  observable outcome. Where the epic already expresses them in Given/When/Then
  form, keep that form rather than flattening it.
- Enumerate the files and components the story will likely touch, with
  their current roles, as orientation rather than as a change mandate.
- Order tasks and subtasks deliberately: implementation follows this sequence
  exactly, so the order is part of the specification, not a formatting choice.
- Keep the story self-contained: a reader with only this document and the
  workspace should be able to implement without archaeology.

## Output boundary

The output is one inert story-specification document artifact. It does not
implement anything, modify a workspace, or claim the sprint plan advanced.
