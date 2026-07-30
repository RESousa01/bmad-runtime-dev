# bmad-architecture

- **rowId**: `method:src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md`
- **canonicalId**: `bmm:bmad-architecture`
- **pinned source**: `packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md`

## Implemented governed behavior

The adapter declares the complete seven-member pinned source subtree as
immutable, SHA-256-addressed data. The architecture creation engine consumes
reviewed document artifacts through its ordered, append-only progression and
distills a single inert architecture spine with a source-shaped memlog. Both
interactive and headless creation are tested against the same required spine
and memlog output; source prose grants no filesystem, process, network,
provider, scheduler, or write authority.

Cancellation, model/tool retry, validation repair, authority reauthorization,
and durable failure records are generic-runtime behavior. The task test runs
those typed recovery paths using the architecture source identity, so none is
implemented as a source-script escape hatch.

## Compatibility decision

**adapt / adapted.** The normalized projection remains a governed `create`
projection because the shared sealed verifier currently requires
`normalizedAction: "create"`. The upstream skill additionally detects
conversational `update` and `validate` intents, expects an existing memlog to
be restored on resume, and produces a validation report. The leased adapter
trait has no intent-to-session binding, persistent session decoder, or
validate-report artifact contract. These are explicit shared-interface gaps,
not silently omitted source behavior.

The semantic diff is owned by `MTH-12` and applies to Forge current with
BMAD Method 6.10.0. It may become `adopt / exact` only after the generic
runtime binds all three intents and both modes to a durable architecture
session, restores the session after restart/source upgrade, and exposes the
source validation report through typed artifacts and the catalog/Help ledger.

## Evidence

Source-derived cases are in
`artifacts/bmad100/method/bmad-architecture/golden/source-derived-cases.json`.
The receipt files beside them record source-closure, fixture conformance, and
authority commands. Cases marked `shared_interface_gap` are intentionally not
presented as implemented.
