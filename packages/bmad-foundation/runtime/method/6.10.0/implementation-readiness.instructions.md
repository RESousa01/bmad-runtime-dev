# Managed implementation-readiness guidance

## Purpose

Support one bounded readiness review proving that PRD, UX, architecture,
and the epics-and-stories listing are complete and mutually aligned before
implementation. This record is sealed read-only instruction data for the
`bmm:bmad-check-implementation-readiness` capability.

## Guidance

- Review adversarially: success is measured by the gaps found, not by the
  approvals granted.
- Trace every requirement forward into stories and every story backward
  into requirements; report both dangling directions explicitly.
- Check alignment pairwise (PRD to UX, PRD to architecture, architecture
  to stories) and name each contradiction precisely.
- Classify findings by severity: blocks implementation, degrades quality,
  or cosmetic; never bury a blocker in a list of nits.
- Ground every finding in the reviewed context snapshot with a citation to
  the artifact section it concerns.

## The overall assessment

End with one overall reading, stated in upstream's terms rather than left
implicit in a list of findings:

- **Ready** — no finding blocks implementation.
- **Needs work** — implementation could start, but named gaps will cost more to
  fix later than now.
- **Not ready** — at least one finding blocks implementation outright.

Say which it is and why, naming the findings that drove it. Note that this is a
required gate in the method's own sequence, so a reader will act on it — which
is exactly why it must be stated plainly rather than softened. It remains a
reading and not an authorisation: this run approves nothing and advances
nothing.

## Output boundary

The output is one inert readiness-report document artifact. It does not
approve implementation, modify planning artifacts, or gate any workflow;
the decision belongs to the humans reading it.
