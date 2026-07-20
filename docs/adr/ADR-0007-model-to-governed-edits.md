# ADR-0007: Verified model output enters D3 as an ordinary candidate

- Status: accepted (2026-07-20)
- Depends on: ADR-0002 (independent D2/D3 epochs), ADR-0005/0006
  (capability archetypes), the governed-changes authority (P2/P4).

## Context

Change-set capabilities (`bmm:bmad-dev-story`, `bmm:bmad-quick-dev`,
`bmm:bmad-qa-generate-e2e-tests`) produce verified
`governed_change_set` results: bounded relative-path create/replace/delete
candidates with model-declared preimage hashes, stored as encrypted
capability-run data. Something must let a user turn that stored result
into applied files — without the model output ever gaining authority.

## Decision

1. **The only path from model output to files is the existing D3
   proposal flow.** A stored change-set result is converted into ordinary
   `ProposedFileChange` values and submitted through the same
   `changes.propose` review, approval, checkpoint, apply, undo, and
   recovery machinery as renderer-originated edits. No parallel apply
   path exists; nothing about the result being model-produced changes any
   D3 rule.
2. **D2 consent authorized only egress.** The consent that produced the
   result is spent evidence. D3 approval is separate, local, fresh,
   single-use, and bound to preimages the host observes at proposal time
   (ADR-0002's independent epochs stand).
3. **Model-declared preimages are a staleness tripwire, not authority.**
   The host adapter re-reads every replace/delete target through governed
   workspace I/O and compares the model's declared preimage hash to the
   freshly observed bytes. A mismatch — the workspace moved after the
   model saw it — fails closed before any proposal exists. The D3 review
   then re-observes again through its own preimage machinery.
4. **The receipt is displayable evidence.** The renderer may show the
   run's origin and receipt status beside the ordinary diff review; it
   participates in no authority decision.

## Consequences

- Sign-out (D2 withdrawal) does not invalidate an already-reviewed D3
  proposal; a governed-edit epoch advance does — exactly the ADR-0002
  posture, now spanning model-originated changes.
- External edits between model completion and proposal surface as
  preimage mismatches and fail closed instead of producing surprise
  diffs.
- The change-set adapter is small and auditable: parse the sealed result,
  verify staleness, emit `ProposedFileChange` — every downstream property
  is inherited from the already-qualified D3 matrix.
