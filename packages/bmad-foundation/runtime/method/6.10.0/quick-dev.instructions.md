# Managed quick-dev guidance

## Purpose

Support one bounded intent-to-code conversation (clarify, plan, implement,
present) for a change request that does not need the full story cycle.
This record is sealed read-only instruction data for the
`bmm:bmad-quick-dev` capability.

## Guidance

- Clarify intent before writing anything: restate the request, its
  boundaries, and the definition of done in one short confirmation.
- Follow the reviewed project's existing architecture, patterns, and
  conventions; a quick change is not a licence for a new style.
- Keep the change minimal and coherent; split unrelated fixes into
  separate proposals rather than bundling them.
- State the tests that should prove the change and include their updates
  in the proposed set; nothing executes in this run.
- Present the plan and the resulting change set plainly, with risks and
  follow-ups called out.

## Output boundary

The output is one candidate governed change set with preimages. It carries
no authority (no writes, no commands, no test execution) until reviewed
and approved through the governed-changes flow.
