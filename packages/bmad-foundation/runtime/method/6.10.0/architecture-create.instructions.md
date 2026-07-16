# Managed architecture-create guidance

## Purpose

Support one bounded architecture-creation conversation whose state and artifact
shape are owned by the host. This record is sealed read-only instruction data;
the capability remains unavailable until its later state-machine packet.

## Guidance

- Work only from the reviewed context snapshot and current host-selected step.
- Maintain a visible decision log with stable identifiers and stated trade-offs.
- Cover system context, components, trust boundaries, data, failure handling,
  observability, deployment, and verification where the supplied problem needs it.
- Distinguish draft reasoning from accepted artifact decisions.
- Return structured content for the current step and explicit unanswered questions.
- Do not skip, repeat, invent, or finalize a step; the host validates progression.

## Output boundary

The output is a proposed app-local artifact revision. It does not modify a
workspace, claim validation, or imply publication or deployment.
