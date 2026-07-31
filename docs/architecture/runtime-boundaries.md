---
title: "Forge runtime boundaries"
doc_type: reference
status: maintained
owner_role: architecture-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - crates/desktop-app/src
  - crates/desktop-app/capabilities
  - crates/desktop-runtime/src
  - apps/desktop-ui/src
supersedes:
  - docs/archive/legacy-v6/94 - Windows Desktop Native Host and IPC.md
---

# Forge runtime boundaries

## Composition root

`desktop-app` owns live composition. It creates the workspace broker, local store, chat/context
coordinator, provider transport, BMAD services, governed-edit service, and Tauri command surface.
Lower crates provide types and bounded mechanisms; they do not independently grant authority.

## Renderer boundary

The WebView receives typed projections and emits typed requests. Tauri capabilities remain narrow.
Secrets are submitted once and are never projected back. Capability IDs and workspace IDs are
rebound host-side rather than trusted from rendered state.

## Model boundary

A model result is untrusted content. It may propose text, a capability result, or a governed
change, but it cannot directly call filesystem/effect internals. Egress consumes exact reviewed
context; writes consume a separate exact proposal/preimage decision.

## Agent execution boundary

ADR-0009 permits host-governed execution. Current source includes bounded read tools and
permission evaluation through `WorkspaceBroker`. Product-wide write, shell, Git, tool-loop,
timeout/output, and recovery qualification is not complete merely because individual primitives
exist. ADR-0010 remains proposed.

## Boundary checks

`node tools/check-boundaries.mjs` checks architecture restrictions from source, Cargo locks,
capabilities, and Tauri configuration. It does not prove deployed service configuration.
