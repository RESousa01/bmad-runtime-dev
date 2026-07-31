---
title: "Forge authentication and model access"
doc_type: current
status: current
owner_role: security-operations-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - crates/desktop-cloud/src
  - crates/desktop-app/src
  - docs/operations/model-profile-format.md
  - docs/operations/foundry-prerequisites.md
supersedes:
  - docs/archive/legacy-v1-knowledge/knowledge-base/current/03-security-and-trust.md
---

# Forge authentication and model access

## Current direct-provider prototype

Forge can connect to OpenAI or Azure OpenAI in a development feature build. The user enters a
provider, model, API key, and provider-specific endpoint information. Typed IPC transfers the key
once to the Rust host; the frontend clears the field, and the host retains the secret only in
zeroizing process memory until disconnect or process exit.

Connecting does not send workspace content. Each model call still requires an exact context
review and a fresh single-use decision. Storage is disabled on Responses requests.

## Endpoint distinctions

| Path | Current meaning | Evidence boundary |
|---|---|---|
| OpenAI Responses | Direct development request to the configured OpenAI model | PoC only |
| Azure OpenAI `/openai/v1/` | Direct development request to a configured Azure OpenAI resource | PoC only |
| Foundry project/deployment routes | Project endpoint and deployment-discovery contract | Requires real prerequisite and deployed qualification evidence |
| Support-plane broker | Intended governed production composition | Incomplete |

A successful call on one path does not qualify another.

## No-Entra production direction

Current development and intended deployment must work without Entra. Historical WAM and
identity-based designs are not prerequisites. Fake tenant, user, or managed-identity values are
not a substitute for credential custody, rotation, revocation, service attribution, isolation,
bounded egress, audit evidence, and recovery.

The production no-Entra path is incomplete. See
[ADR-0012](../adr/ADR-0012-no-entra-authentication-and-model-access.md).

## Foundry status

Local fake-server contracts may validate deterministic request/response behavior, but real
Foundry readiness additionally requires an actual project endpoint, deployment inventory,
credential/RBAC arrangement, staging qualification, and deployed evidence. Mocks cannot satisfy
those gates.
