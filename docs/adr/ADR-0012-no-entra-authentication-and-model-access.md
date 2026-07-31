---
title: "ADR-0012: No-Entra authentication and model access"
doc_type: adr
status: accepted
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2026-09-30
source_paths:
  - crates/desktop-cloud/src
  - crates/desktop-app/src
  - docs/operations/model-profile-format.md
  - docs/guides/forge-local-poc-demo.md
supersedes: []
---

# ADR-0012: No-Entra authentication and model access

- Status: accepted (2026-07-31)

## Context

Forge has a development-only direct-provider path and incomplete support-plane/Foundry work.
Historical designs gave Entra and WAM more authority than the current product direction permits.
Placeholder identity configuration would hide the missing production trust design rather than
solve it.

## Decision

Current Forge development and intended deployment must work without Entra.

The direct-provider development prototype may accept an OpenAI or Azure OpenAI API key through
typed IPC. The Rust host holds it only in zeroizing process memory, and every model request remains
bound to exact-context review and a single-use decision. This is prototype transport evidence, not
production authentication or release evidence.

The production no-Entra path is a separate governed design. It must provide credential custody,
rotation, revocation, service attribution, tenant isolation, bounded egress, audit evidence, and
operator recovery without fabricating user or managed-identity values. That path is incomplete.

Entra/WAM material is historical, rejected, or deferred reference material. It is not a current
developer prerequisite, a deployment prerequisite, or a substitute for the missing production
design.

## Endpoint boundaries

- OpenAI uses the Responses API host and model identity selected by the development provider.
- Azure OpenAI direct prototype traffic uses an Azure OpenAI resource endpoint and the
  `/openai/v1/` Responses route.
- Foundry project endpoints and deployment discovery are a different contract and remain subject
  to their own prerequisite and qualification evidence.
- The support-plane target is not proven merely because a direct API-key request succeeds.

## Consequences

- Current docs separate direct-provider PoC, deterministic local behavior, and intended managed
  production composition.
- Fake identity settings fail closed and cannot be cited as no-Entra qualification.
- Completion requires deployed, exercised, and reviewed production evidence; this ADR does not
  claim that evidence exists.

## Evidence

- [ADR-0010](ADR-0010-native-agent-runtime-and-turn-scoped-egress.md)
- [Forge local PoC guide](../guides/forge-local-poc-demo.md)
- [Foundry prerequisites](../operations/foundry-prerequisites.md)

