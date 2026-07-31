---
title: "Reference vault migration record"
doc_type: archive
status: archived
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2027-07-31
source_paths:
  - docs/archive/README.md
  - docs/reference/manifests/legacy-v6-manifest.json
  - docs/reference/manifests/legacy-v1-knowledge-manifest.json
supersedes: []
---

# Reference vault migration record

This page records the retirement of the old reference-vault authority layer. It is historical
evidence, not current implementation guidance. Current documentation starts at
[`docs/README.md`](../README.md).

## Preserved material

- The 105 root vault notes are byte-preserved under `docs/archive/legacy-v6/` and
  `docs/archive/preserved/`, with identities in `legacy-v6-manifest.json`.
- The v1 current/evidence documents, validators, tests, and superseded root handoff are preserved
  as 24 hashed files under `docs/archive/legacy-v1-knowledge/` and `docs/archive/handoffs/`.
- Current product and engineering guidance moved to `docs/current/`, `docs/architecture/`, and
  `docs/development/`.
- Active documentation validation moved to `tools/knowledge/`.

The old `vault-validation.json` remains historical evidence only. It does not establish current
product truth.

## Remaining source boundary

Imported vendor snapshots remain temporary, non-authoritative research inputs because approved
immutable storage locators and complete source identities have not been supplied. They must not be
deleted or promoted to product authority until the fail-closed intake gate in
[`docs/reference/source-intake-policy.md`](../reference/source-intake-policy.md) passes.
