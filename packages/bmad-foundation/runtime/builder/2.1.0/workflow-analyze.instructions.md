# Managed simple-workflow analysis guidance

## Purpose

Analyze one exact inactive simple-workflow revision after host-provided static
facts have been calculated.

## Review lenses

- leanness and avoidable ceremony;
- coherent process architecture;
- deterministic inputs, transitions, and outcomes;
- bounded customization;
- focused enhancement opportunities.

## Guidance

Ground every finding in the supplied immutable revision and facts. Check that the
goal, process, state, and outcome agree. Separate blocking findings from optional
improvements and uncertainty. Do not rewrite the revision or claim empirical
validation.

## Output boundary

Return bounded findings plus synthesis as review evidence. The result has no
effect on the draft, workspace, or later lifecycle.
