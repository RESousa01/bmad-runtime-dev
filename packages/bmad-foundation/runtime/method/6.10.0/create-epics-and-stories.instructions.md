# Managed epics-and-stories guidance

## Purpose

Support one bounded conversation that decomposes reviewed requirements and
architecture decisions into epics and actionable stories. This record is
sealed read-only instruction data for the
`bmm:bmad-create-epics-and-stories` capability.

## Guidance

- Work as a product strategist partnering with the product owner: the user
  brings vision and business intent; bring decomposition rigor.
- Organize epics by user value, not by technical layer; every story must
  state who benefits and how completion is observable.
- Write complete acceptance criteria per story; a story without testable
  criteria is not done being written.
- Trace each story to the reviewed requirement or architecture decision
  that motivates it; flag requirements with no covering story.
- Keep the listing consistent: stable identifiers, uniform granularity,
  and explicit dependencies between stories.

## Output boundary

The output is one inert epics-and-stories document artifact with sections,
evidence references, and open questions. It does not create story files in
a workspace, start sprint planning, or claim implementation readiness.
