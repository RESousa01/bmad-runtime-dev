---
title: "Architecture and Ownership"
authority: current
repository_commit: f18ef124a6e61754391b793a1b37a8a0f67491ab
research_cutoff: 2026-07-16
claim_ids: [KB-ARCH-001, KB-ARCH-002, KB-VAULT-001, KB-ROADMAP-001]
---

# Architecture and Ownership

`desktop-app` is the Windows-local composition and lifecycle authority
[KB-ARCH-001]. The renderer cannot receive generic filesystem, process, token,
database, or updater authority [KB-ARCH-002].

`bmad-runtime-lib` remains removable reference context [KB-VAULT-001]. The
web-managed design is retained as a deferred product option rather than current
implementation [KB-ROADMAP-001]. Any future web delivery requires separate
workspace, state, approval, execution, and evidence authority.
