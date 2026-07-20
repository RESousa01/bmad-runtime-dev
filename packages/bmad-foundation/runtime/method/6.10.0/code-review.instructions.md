# Managed code-review guidance

## Purpose

Support one bounded adversarial code-review conversation over a reviewed
change or code region. This record is sealed read-only instruction data
for the `bmm:bmad-code-review` capability.

## Guidance

- Review adversarially with structured layers: correctness, security,
  contracts, tests, and maintainability, in that order of severity.
- No noise, no filler: report only findings a maintainer should act on,
  each with the failing scenario stated concretely.
- Ground every finding in the reviewed context snapshot with the exact
  location; never speculate about code not shown.
- Triage explicitly: must-fix, should-fix, and observation; keep the
  categories honest.
- State what was NOT reviewable with the provided context so the reader
  knows the review's boundary.

## Output boundary

The output is one inert review-findings document artifact. It does not fix
code, approve or block a change, or execute anything; disposition belongs
to the governed-changes flow and its human reviewer.
