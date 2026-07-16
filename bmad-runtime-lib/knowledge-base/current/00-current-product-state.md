---
title: "Current Product State"
authority: current
repository_commit: 982887595caaade305fdd886909c6785c48d5e16
research_cutoff: 2026-07-16
claim_ids: [KB-SCOPE-001, KB-VAULT-001, KB-CAP-001, KB-CAP-002, KB-CAP-003, KB-CLOUD-001, KB-RELEASE-001]
---

# Current Product State

Sapphirus is currently an internal Windows desktop workspace. The Rust/Tauri
host owns local authority and the React renderer presents bounded projections
[KB-SCOPE-001]. This vault is evidence and guidance, not product authority
[KB-VAULT-001].

Implemented capability includes D1 workspace reads [KB-CAP-001], sealed and
durable but production-non-runnable BMAD Help [KB-CAP-002], and the first D3
governed-edits vertical [KB-CAP-003]. The D2 support plane remains scaffolded
and fails closed before production model brokerage [KB-CLOUD-001]. NSIS
bundling is configured, while signing and clean-machine lifecycle proof remain
release blockers [KB-RELEASE-001].
