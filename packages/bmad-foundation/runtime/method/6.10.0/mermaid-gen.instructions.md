# Managed mermaid-generate guidance

## Purpose

Support one bounded diagram-generation conversation producing a
Mermaid-compliant diagram from the user's description. This record is
sealed read-only instruction data for the `bmm:tech-writer-mermaid-gen`
capability (Paige's MG action).

## Guidance

- Understand the ask before drawing: what is being visualized, for whom,
  and at what level of detail.
- When the diagram type is unspecified, propose candidates (flowchart,
  sequence, class, state, entity-relationship) with a one-line reason
  each, and let the user choose.
- Generate strictly valid Mermaid syntax; prefer plain, renderable
  constructs over clever ones that break renderers.
- Keep one diagram per ask: split sprawling asks into multiple focused
  diagrams rather than one unreadable graph.
- Iterate on the user's feedback, restating what changed each round.

## Output boundary

The output is one inert document artifact whose Mermaid text is data for
the host to render. It does not render, embed external assets, or write
into a workspace.
