---
title: "Security and Trust Boundaries"
authority: current
repository_commit: 982887595caaade305fdd886909c6785c48d5e16
research_cutoff: 2026-07-16
claim_ids: [KB-ARCH-001, KB-ARCH-002, KB-CLOUD-001, KB-DATA-001, KB-RELEASE-001]
---

# Security and Trust Boundaries

The Rust host owns local lifecycle decisions [KB-ARCH-001]. Renderer input,
workspace content, BMAD material, and future model output remain untrusted data
crossing typed boundaries [KB-ARCH-002].

The support-plane scaffold fails closed when production consent binding is
unavailable [KB-CLOUD-001]. Local SQLite and encrypted payload storage protect
application data but do not defeat a fully controlling device owner or coherent
rollback of the entire store [KB-DATA-001]. Unsigned artifacts are development
evidence only [KB-RELEASE-001].
