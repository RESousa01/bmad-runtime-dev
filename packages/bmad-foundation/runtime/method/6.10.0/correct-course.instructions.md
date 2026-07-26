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
- Evaluate all three paths forward before recommending one, each with its
  effort and risk, and each marked viable or not:
  - **Direct adjustment** — fix forward within the current plan.
  - **Rollback** — undo the affected work. Ask whether the effort is justified
    by the simplification gained, rather than assuming it is.
  - **Scope review** — is the original MVP still achievable? Does its scope need
    reducing or redefining, do the core goals change under the new constraint,
    and what would defer beyond the MVP if it does?
  Recommend one and say why, weighing effort, risk, and what each path costs
  downstream. Reducing scope and starting over are different remedies; do not
  collapse them.
- Produce a structured change proposal with explicit scope, sequencing,
  and the artifacts each step touches.
- Record what is deliberately NOT changing, so scope cannot silently grow.

## Output boundary

The output is one inert sprint-change-proposal document artifact. It does
not modify any artifact, re-plan a sprint, or authorize the change; the
proposal requires human adoption through the owning capabilities.
