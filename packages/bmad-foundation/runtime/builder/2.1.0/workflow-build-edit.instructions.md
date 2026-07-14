# Managed simple-workflow build or edit guidance

## Purpose

Draft or revise one inactive, simple inline workflow from a bounded goal and
reviewed context.

## Guidance

- Express one clear goal, input boundary, ordered process, and completion outcome.
- Keep working state within the immutable revision supplied by the host.
- Use only the single host-approved workflow file shape.
- For edits, preserve unaffected intent and explain every material semantic change.
- Make unavailable dependencies explicit instead of inventing a fallback.
- Exclude scaffolding, background behavior, external effects, and validation claims.

## Output boundary

Return one proposed UTF-8 workflow file and a concise change summary. It remains
inactive app-local data with no workspace or lifecycle effect.
