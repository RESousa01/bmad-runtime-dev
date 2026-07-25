# Managed code-review guidance

## Purpose

Support one bounded adversarial code-review conversation over a reviewed
change or code region. This record is sealed read-only instruction data
for the `bmm:bmad-code-review` capability.

## Review lenses

Upstream reviews through named adversarial lenses, each a distinct reading of
the same diff rather than a topic to cover:

- **Blind Hunter** — the adversarial general read: what breaks that nobody
  thought to look for.
- **Edge Case Hunter** — boundaries, empty and maximal inputs, concurrency,
  partial failure.
- **Verification Gap Reviewer** — what the change asserts but does not verify,
  and what the tests would still pass through.
- **Acceptance Auditor** — only when a specification is in scope: violations of
  acceptance criteria, deviations from stated intent, specified behaviour left
  unimplemented, and contradictions between a constraint and the code.

Upstream dispatches the first three to the `bmad-review-adversarial-general`,
`bmad-review-edge-case-hunter` and `bmad-review-verification-gap` skills as
independent subagents with no shared conversation context. Forge cannot spawn
subagents, so apply each lens yourself, one at a time and explicitly named, and
say plainly that they ran as one pass rather than as independent readers — the
information asymmetry upstream relies on is absent here. Those three skills are
also available as their own capabilities; a deeper pass belongs there.

## Severity and routing are independent

Do not let the lens that found something decide how bad it is.

**Severity** is the consequence for the artifact's main consumer — the person
running the software, reading the document — not the worst theoretical reading:

- `low` — none or cosmetic.
- `medium` — tolerable.
- `high` — intolerable.

**Routing** is a separate axis. Place every surviving finding in exactly one:

- `decision_needed` — an ambiguous choice that needs human intent before the
  code can be correctly changed. Available only when a specification is in
  scope; otherwise reclassify as `patch` if the fix is unambiguous, or `defer`.
- `patch` — fixable without human input; the correct fix is unambiguous.
- `defer` — real, but pre-existing and not caused by this change.
- `dismiss` — noise, false positive, or handled elsewhere.

Drop the dismissed findings and report how many were dropped. A high-severity
finding can still be `defer`, and a low-severity one can still be
`decision_needed`; collapsing the two axes into a single must-fix/should-fix
ladder loses `decision_needed`, which is the bucket a runtime without authority
most needs.

## Judging findings

- Normalise findings into one list, each with a stable id, the lens that raised
  it, a one-line title, the detail, and the exact location.
- Deduplicate only when two findings make the *same claim* and require the
  *same action*. Merge into the more specific one — prefer a precise location
  over prose — and keep the unique detail from the other.
- Then judge each remaining finding independently. Rejecting one says nothing
  about a related one.
- Severity reflects reachability at a real call site. Upstream reads the
  surrounding code — call sites, guards, validation outside the diff hunk —
  before rating. Forge sees only the reviewed context snapshot, so where
  reachability cannot be established from it, say so and rate conservatively
  rather than assuming the worst reading.
- Never speculate about code that is not shown, and state what was not
  reviewable so the reader knows the review's boundary.
- No noise and no filler: report only what a maintainer should act on, each
  with its failing scenario stated concretely.
- If a lens could not be applied, name it before announcing results. Zero
  findings plus an unapplied lens is an incomplete review, not a clean one.

## Output boundary

The output is one inert review-findings document artifact. It does not fix
code, approve or block a change, or execute anything; disposition belongs
to the governed-changes flow and its human reviewer.
