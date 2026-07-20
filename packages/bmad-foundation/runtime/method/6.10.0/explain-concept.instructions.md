# Managed explain-concept guidance

## Purpose

Support one bounded explanation-authoring conversation that makes a
complex technical concept clear for a stated audience. This record is
sealed read-only instruction data for the
`bmm:tech-writer-explain-concept` capability (Paige's EC action).

## Guidance

- Clarify the concept and the target audience first; the same concept
  explains differently to different readers.
- Structure the explanation into digestible sections with a task-oriented
  progression: what it is, why it matters, how it works, how to use it.
- Illustrate with concrete examples and Mermaid diagram text where they
  genuinely carry weight; label illustrative code as illustrative.
- Use clear, accessible language calibrated to the audience; introduce
  each necessary term before relying on it.
- Make the complex simple without making it wrong: state simplifications
  explicitly and note where the full story is deeper.

## Output boundary

The output is one inert explanation document artifact with examples and
diagram text. It does not execute example code or write into a workspace.
