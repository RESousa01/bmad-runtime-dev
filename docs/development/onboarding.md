---
title: "Forge onboarding"
doc_type: runbook
status: maintained
owner_role: devex-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - docs/README.md
  - docs/current
  - package.json
supersedes:
  - docs/archive/legacy-v6/Start Here.md
---

# Forge onboarding

## 15-minute orientation

1. Read [Forge team documentation](../README.md).
2. Read [product state](../current/product-state.md).
3. Read [roadmap and known gaps](../current/roadmap-and-known-gaps.md).
4. Review [architecture and authority](../current/architecture-and-authority.md).
5. Open the [ADR index](../architecture/adr-index.md).
6. Locate the [BMAD status](../bmad/BMAD-100-IMPLEMENTATION-STATUS-2026-07-30.md) and verify its
   date/commit limitations before using it.

At the end, you should be able to explain what Forge does, what the Rust host owns, which model
modes exist, why only one primary BMAD capability is qualified, and which release gates remain
unproven.

## 60-minute developer setup

Follow [local setup](local-setup.md). Confirm the pinned toolchain, install dependencies, run the
fast documentation gate, run one narrow product gate, and start either deterministic or direct
development mode.

## First contribution

Use [contribution guide](contribution-guide.md):

1. Choose one bounded active task.
2. Identify authority, acceptance checks, and related ADRs.
3. Make the smallest coherent change.
4. Run narrow tests.
5. Update durable docs only if the verified behavior or decision changed.
6. Run `git diff --check` and the required named gates.

Do not consult the legacy archive to decide current behavior. Use it only through the explicit
history/reference route.
