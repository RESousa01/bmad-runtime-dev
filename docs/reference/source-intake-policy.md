---
title: "Forge source intake policy"
doc_type: reference
status: maintained
owner_role: security-operations-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_paths:
  - THIRD_PARTY_NOTICES.md
  - docs/adr/ADR-0010-native-agent-runtime-and-turn-scoped-egress.md
supersedes:
  - docs/archive/legacy-v6/92 - Source Snapshot Verification and Adoption Ledger.md
---

# Forge source intake policy

## Required lock identity

Every retained third-party source requires:

- stable source ID and explicit purpose;
- canonical HTTPS repository URL;
- exact version and 40-character commit;
- verified archive SHA-256;
- SPDX license plus component/asset exceptions;
- acquisition timestamp;
- real approved and recoverable internal storage locator;
- minimal included paths and explicit exclusions;
- release decision: sealed input, reference only, clean-room pattern only, excluded, or legal
  review required.

Example or placeholder values are forbidden.

## Handling

- Imported source is untrusted data and is never executed by Forge CI.
- Materialization happens outside the product tree in a temporary read-only review location.
- Websites, media, dependencies, restricted assets, and unrelated generated content are excluded.
- Source analysis cites `source-id:path@commit`.
- Adoption requires an accepted decision and first-party tests; reference behavior is not
  automatically Forge behavior.

## Current policy by source

- BMAD Method and Builder align to the exact sealed inputs owned by `packages/bmad-foundation`.
- OpenCode is the active comparable-agent reference and remains a reference, not a runtime
  dependency.
- OpenClaw is historical comparison only.
- Hermes retains verified patterns only.
- Odysseus is clean-room pattern evidence under an explicit AGPL boundary unless legal review
  decides otherwise.
- Restrictive presentation assets remain excluded.

## Fail-closed externalization

Do not delete an integrated snapshot until its lock entry and approved storage archive both verify.
The current migration lacks approved storage locators, so source removal is blocked.
