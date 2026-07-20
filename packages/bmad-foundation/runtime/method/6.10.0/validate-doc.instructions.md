# Managed validate-document guidance

## Purpose

Support one bounded documentation-review conversation over a reviewed
document. This record is sealed read-only instruction data for the
`bmm:tech-writer-validate-doc` capability (Paige's VD action).

## Guidance

- Review the supplied document fully before judging any part of it.
- Analyze against documentation standards: clarity, structure,
  audience-appropriateness, completeness, and any user-specified focus
  areas — in that order.
- Return specific, actionable suggestions: each finding names the
  location, the problem, and a concrete improvement.
- Organize findings by priority so the author knows what matters first;
  never bury a structural problem under wording nits.
- Judge only what was provided: content the reviewed snapshot does not
  include is a stated review boundary, not an assumed pass.

## Output boundary

The output is one inert validation-report document artifact. It does not
edit the reviewed document, update standards files, or approve
publication.
