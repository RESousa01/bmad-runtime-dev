# ADR-0006: Builder authoring as governed inactive drafts

- Status: accepted (2026-07-20)
- Depends on: ADR-0005 (capability denominator), the generic capability
  lifecycle (readiness Tasks 5-7).

## Context

The Builder source family (agent analyze/create-rebuild/edit, workflow
analyze/build-edit) authors new agents and workflows. In the upstream
product these outputs install and register themselves. Sapphirus's locked
posture is that nothing model-produced acquires authority: since P5 the
five Builder instruction projections have been sealed as inactive data,
and the closure ledger carried their operations as `planned`.

## Decision

1. **Builder authoring runs through the same generic capability
   lifecycle** as every menu capability: reviewed exact context, one
   single-use consent, transport dispatch, wire verification, durable
   persistence in the capability-run store. No separate authoring engine
   exists.
2. **The only output archetype is `inactive_builder_draft`.** The wire
   verifier enforces the closed shape with exact-key checking: a draft
   carries `draftKind`, `title`, `revisionNote`, and bounded relative-path
   files — and nothing else. Activation, registration, installation,
   hook, command, and network fields are rejected as authority smuggling,
   never ignored.
3. **Drafts can never install, register, execute, or alter the capability
   catalog.** The reviewed capability table is a compile-time constant;
   draft content is stored as encrypted data and rendered inertly. Making
   a draft executable would require a new reviewed source-intake and
   catalog change through the ordinary human process.
4. **Persistence note.** Drafts persist as capability-run results
   (encrypted CAS payloads keyed by run), not through the legacy Builder
   revision repository; the P5 Builder aggregate remains the home of the
   pre-existing analysis/decision history. Unifying the two stores is
   deliberate future work and does not change the authority boundary.

## Consequences

- The five closure-ledger Builder records move `planned -> active` with
  the same evidence rule as menu capabilities: the table-driven lifecycle
  matrix proves each operation end-to-end (now 29/29).
- The renderer shows drafts with explicit inactive status and no
  install/activate affordance; the completed-run panel treats them like
  any other inert result.
- The ADR-0003 rule that Builder outputs stay outside the executable
  catalog remains fully in force; this ADR activates *authoring*, not
  execution.
