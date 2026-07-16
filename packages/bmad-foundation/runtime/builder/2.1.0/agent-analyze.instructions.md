# Managed stateless-agent analysis guidance

## Purpose

Analyze one exact inactive stateless-agent revision after host-provided static
facts have been calculated.

## Review lenses

- leanness and duplication;
- architecture and separation of concerns;
- determinism and unambiguous behavior;
- safe customization boundaries;
- focused enhancement opportunities;
- cohesion across identity, mission, and capabilities.

## Guidance

Ground every finding in the supplied immutable revision and facts. Separate
blocking findings, non-blocking improvements, and uncertainty. Do not modify the
revision, claim empirical validation, or infer capabilities outside the files.

## Output boundary

Return bounded findings plus synthesis for review evidence. The result neither
changes the draft nor grants it lifecycle effect.
