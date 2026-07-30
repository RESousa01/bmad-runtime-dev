# Managed BMAD Review instruction projection

This projection adapts the pinned `src/core-skills/bmad-review` closure. It does
not grant filesystem, process, network, browser, subagent, model, or external
handoff authority. The host supplies reviewed content and resolves every effect
through the existing typed Method plan and Airlock boundaries.

## Inputs

- Review content is required, either as reviewed inline content or a reviewed
  file reference supplied by the host.
- `lenses` may name configured lenses. When omitted, use the applicable lenses
  from the pinned customization file.
- `also_consider` adds review focus without replacing a lens.
- Pre-resolved customization may be supplied only by the host.

## Ordered execution

1. Resolve the pinned customization. Reject unknown lenses, invalid
   configuration, or unavailable required resources.
2. Load the reviewed content through the host-provided bounded resource set.
   Never discover or read additional paths independently.
3. Select applicable lenses. Adversarial and edge-case lenses apply to any
   content; verification-gap applies to code; structure and prose apply to
   documents; prose runs after structure.
4. Announce the selected lenses and why each applies.
5. Run independent lenses without sharing findings between them.
6. Run dependent lenses only after their declared prerequisite.
7. Assemble and present the findings.

## Findings contract

The canonical review data is a JSON array. Each finding contains exactly:

- `lens`
- `category`
- `description`
- `evidence`
- `suggested_improvement`

A lens may add only fields declared by its pinned reference. Do not add
severity, priority, ranking, or a global verdict.

The desktop capability result is the governed
`sapphirus.bmad-document-artifact.v1` envelope. Put the complete canonical JSON
array, unchanged, in one `Findings` section body. Optional prose may summarize
scope and selected lenses, but it must not alter or rank the canonical findings.

## Lifecycle and recovery

Interactive and headless modes use the same seven-step Method table. Empty
findings are valid when every selected lens completed. Missing input,
configuration drift, unavailable resources, invalid lens output, user
cancellation, or denied effects fail closed. Durable checkpoints permit exact
retry, repair, restart, and resume only under the same sealed binding; source or
configuration drift requires rebinding and a fresh context review.
