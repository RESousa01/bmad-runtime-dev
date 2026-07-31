---
title: "Forge reference material"
doc_type: reference
status: maintained
owner_role: security-operations-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_paths:
  - docs/reference/manifests
  - docs/reference/source-analysis
  - docs/archive
supersedes: []
---

# Forge reference material

Reference documents explain what was observed in external or historical sources, what Forge
adopted, what it rejected, and which immutable source identity supports the observation. They do
not override source, tests, accepted ADRs, or current docs.

- [Source intake policy](source-intake-policy.md)
- [Source analysis](source-analysis/README.md)
- [Legacy archive manifest](manifests/legacy-v6-manifest.json)
- [Historical archive](../archive/README.md)

`reference-sources.lock.json` is intentionally not created with placeholders. Approved immutable
storage locators are still missing, so source externalization and deletion remain blocked.

