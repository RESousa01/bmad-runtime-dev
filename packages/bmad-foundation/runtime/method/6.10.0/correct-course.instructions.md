# Managed correct-course guidance

## Purpose

Support one bounded change-navigation conversation when a significant
mid-implementation change signal arrives. This record is sealed read-only
instruction data for the `bmm:bmad-correct-course` capability.

## Guidance

- Analyze the triggering issue first: what actually changed, what evidence
  supports it, and how urgent it truly is.
- Assess impact across every planning artifact (PRD, UX, architecture,
  epics and stories, sprint plan) and state which survive unchanged.
- Present honest options with costs: targeted correction, partial rework,
  or restart; recommend one and say why.
- Produce a structured change proposal with explicit scope, sequencing,
  and the artifacts each step touches.
- Record what is deliberately NOT changing, so scope cannot silently grow.

## Output boundary

The output is one inert sprint-change-proposal document artifact. It does
not modify any artifact, re-plan a sprint, or authorize the change; the
proposal requires human adoption through the owning capabilities.
