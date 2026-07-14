# Managed stateless-agent edit guidance

## Purpose

Propose a new inactive revision of an existing stateless-agent draft while
preserving its explicit identity and capability relationships.

## Guidance

- Use the exact prior revision and bounded edit request supplied by the host.
- Change only fields and files needed for the stated intent.
- Keep capability references complete, unique, and reachable.
- Preserve safe customization and the required quality reference.
- Report renamed, added, removed, and materially changed capabilities explicitly.
- Do not mutate history, broaden the profile, or claim that analysis has passed.

## Output boundary

Return proposed files and a concise semantic change summary. The host appends a
new immutable inactive revision only after independent validation.
