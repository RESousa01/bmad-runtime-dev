---
title: "Forge documentation archive"
doc_type: archive
status: archived
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2027-07-31
source_paths:
  - docs/reference/manifests/legacy-v6-manifest.json
  - docs/reference/manifests/legacy-v1-knowledge-manifest.json
  - bmad-runtime-lib/archive-manifest.json
supersedes: []
---

# Forge documentation archive

Everything under this directory is historical evidence, not current implementation guidance.
Current team documentation starts at [docs/README.md](../README.md).

## Legacy v6 notes

The 105 original root notes are preserved byte-for-byte:

- 103 notes under `legacy-v6/`;
- 2 provenance-critical notes under `preserved/`.

The sidecar [legacy manifest](../reference/manifests/legacy-v6-manifest.json) records each original
path, SHA-256, authority class, migration disposition, replacement, and rationale. Archive text is
not rewritten to current names or architecture.

## Review package

`library-audits/` contains the 2026-07-31 deep review, modernization plan, migration matrix,
machine evidence, and unresolved source paths supplied for this migration.

The historical July 16 ZIP named by the review package was not available in the implementation
workspace. Its hash remains explicitly blocked in the migration baseline.

## Legacy v1 knowledge layer

The superseded v1 current/evidence layer and its validators are preserved byte-for-byte under
`legacy-v1-knowledge/`. The old root session handoff is preserved under `handoffs/`. The
[v1 archive manifest](../reference/manifests/legacy-v1-knowledge-manifest.json) records all 24
original paths, archive paths, and SHA-256 identities.

## Path transition

For one compatibility window, `bmad-runtime-lib/README.md` and
`bmad-runtime-lib/archive-manifest.json` point here. Active docs do not use archive notes as
implementation authority.
