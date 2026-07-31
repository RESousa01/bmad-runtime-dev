---
title: "Forge model connectivity"
doc_type: reference
status: maintained
owner_role: security-operations-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - crates/desktop-cloud/src/direct_provider.rs
  - services/desktop-support-api/Foundry
  - services/desktop-support-api/Model
  - docs/operations/foundry-prerequisites.md
supersedes:
  - docs/archive/legacy-v6/98 - Azure Support Plane for Windows Desktop.md
---

# Forge model connectivity

## Direct development providers

`desktop-cloud` implements:

- OpenAI Responses under the validated `https://api.openai.com/v1/` base;
- Azure OpenAI Responses under a validated resource base ending `/openai/v1/`.

Both append `responses`, disable storage, keep credentials in the Rust host, and require reviewed
authorized context. This is development-only transport.

## Foundry project surface

Foundry project endpoints use a different project-scoped contract:

- deployment inventory under `/api/projects/<project>/deployments` or qualified model inventory;
- Responses under the project endpoint's `/openai/v1/responses`.

Local fixtures validate parsing, paging, route restrictions, malformed/oversized responses,
streaming/tool/result behavior, cancellation, and auth shapes. Real readiness still requires an
actual endpoint/deployment inventory, credentials/RBAC arrangement, staging, and deployed
receipts.

## Support plane

The support API and infrastructure describe a separate intended production boundary. Local Docker
profiles prove configuration/liveness contracts only. They do not reproduce cloud database,
secret/configuration services, private networking, revisions/scaling, or monitoring.

## No-Entra direction

The intended production composition must work without Entra. Credential custody, rotation,
revocation, attribution, isolation, bounded egress, telemetry, and recovery are incomplete and
must not be replaced by fake identity settings.
