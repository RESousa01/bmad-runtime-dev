---
title: "Capability Matrix"
authority: current
repository_commit: f18ef124a6e61754391b793a1b37a8a0f67491ab
research_cutoff: 2026-07-16
claim_ids: [KB-CAP-001, KB-CAP-002, KB-CAP-003, KB-CLOUD-001, KB-RELEASE-001]
---

# Capability Matrix

| Capability | Status | Evidence claim |
|---|---|---|
| Selected-workspace reads and projections | `implemented` | KB-CAP-001 |
| Deterministic BMAD Help materialization, retention, and UI | `implemented` | KB-CAP-002 |
| Governed local edits and undo flow | `implemented` | KB-CAP-003 |
| Production consent and managed model broker | `scaffolded` | KB-CLOUD-001 |
| Signed, clean-machine internal installer | `blocked` | KB-RELEASE-001 |

These statuses describe the evidence commit. They are not percentage estimates
and do not imply pilot readiness.

## Readiness assessment at the evidence commit

| Readiness dimension | Assessment |
|---|---:|
| BMAD-06B capability | 98% +/-1 |
| Offline developer-checkout prototype | 98% +/-2 |
| Reproducible/installable offline prototype | 74% +/-6 |
| Complete installable EXE | 38% +/-8 |
| Integrated local/demo D2+D3 desktop | 93% +/-3 |
| User-facing deterministic Help prototype | 87% +/-5 |
| Production model-backed Help vertical | 70% +/-5 |
| User-facing governed-edits prototype | 84% +/-5 |
| First honest AI desktop prototype | 79% +/-5 |
| Internal AI/edits pilot readiness | 51% +/-6 |

`Complete installable EXE` means a current-commit NSIS artifact that is
Authenticode-signed and timestamped, with clean-machine install, launch,
upgrade, uninstall, bundled-resource, and recovery evidence. An older unsigned
developer artifact does not satisfy that definition.
