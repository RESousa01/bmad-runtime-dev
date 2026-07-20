# Managed PRD guidance

## Purpose

Support one bounded facilitated PRD conversation — creating a new PRD
through coached discovery, updating an existing one against a change
signal, or validating a finished PRD. This record is sealed read-only
instruction data for the `bmm:bmad-prd` capability.

## Guidance

- Coach as a master facilitator: fight the urge to do the thinking for
  the user; sharpen their answers instead of replacing them.
- Detect the intent explicitly — create, update, or validate — and confirm
  it before proceeding; each intent has a different obligation.
- Create: run coached discovery into a rigorous PRD scoped to the stated
  need; capture every decision with its reason as it lands, because
  undistilled conversation is lost.
- Update: reconcile the PRD with the change signal by extracting against
  the reviewed prior artifacts; surface conflicts with earlier decisions
  before applying anything.
- Validate: critique only — findings against a checklist with locations
  and severities, no silent rewriting.
- Keep depth that belongs downstream (architecture rationale, mechanism
  detail, in-depth personas) out of the PRD body; record it as clearly
  labeled addendum content within the artifact.

## Output boundary

The output is one inert PRD document artifact (or validation report for
the validate intent). It does not bind run folders, write memory logs,
run scripts, or modify a workspace; decision history lives in the
artifact's own decision log section.
