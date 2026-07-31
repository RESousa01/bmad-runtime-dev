---
title: "BMAD Method and Builder source analysis"
doc_type: reference
status: maintained
owner_role: bmad-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_paths:
  - packages/bmad-foundation/source-manifest.json
  - packages/bmad-foundation/source-accounting.json
  - docs/archive/legacy-v6/83 - BMAD Source Code Review - Method and Builder.md
  - docs/archive/legacy-v6/100 - BMAD Method and Builder Deep Comprehension Audit.md
supersedes:
  - docs/archive/legacy-v6/83 - BMAD Source Code Review - Method and Builder.md
  - docs/archive/legacy-v6/100 - BMAD Method and Builder Deep Comprehension Audit.md
---

# BMAD Method and Builder source analysis

## Verified identity

Forge's sealed foundation is the current reproducible identity for BMAD Method `6.10.0` and
Builder `2.1.0`. Exact archive hashes and per-member hashes are recorded in
`packages/bmad-foundation/source-manifest.json`.

## Adopted

- Method capabilities as the organizing product interface.
- Exact source/member accounting and generated capability projections.
- Explicit interactive/headless modes, declared actions, lifecycle expectations, and evidence
  requirements.
- Builder as a separate authoring lifecycle whose outputs remain inert until activated.

## Rejected or constrained

- Imported scripts do not execute in Forge CI.
- Source text does not grant filesystem, process, network, or install authority.
- Presence in the sealed archive does not imply runtime support.
- Builder output cannot self-install or self-register.

## Historical evidence

The original broad reviews are preserved byte-for-byte in the legacy archive. Their source-path
claims should not be reused directly; current implementation status comes from the sealed
foundation manifests and gates.

External storage lock creation remains blocked, but BMAD reproducibility is preserved by the
product-owned sealed archive hashes rather than the duplicate `_source_review` copies.
