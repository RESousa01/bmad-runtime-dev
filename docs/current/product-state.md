---
title: "Forge product state"
doc_type: current
status: current
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - README.md
  - crates/desktop-app
  - crates/desktop-runtime
  - apps/desktop-ui
  - packages/bmad-foundation/capability-catalog.json
supersedes:
  - docs/archive/legacy-v1-knowledge/knowledge-base/current/00-current-product-state.md
---

# Forge product state

Verified against commit `7f8182fa3ba3461ca6c2d387aedc91663399e2f7` on 2026-07-31.

## What Forge is

Forge is a Windows desktop workspace for governed AI-assisted development. The native Rust host
owns state, workspace access, approvals, provider credentials, file effects, checkpoints,
rollback, and evidence. The React/WebView2 renderer is a typed presentation client.

## Proven in current source

- Native folder selection and opaque workspace grants.
- Bounded tree, read, and search projections with sensitive-path handling.
- Exact context review and single-use decisions before model egress.
- Governed edit review, checkpointed application, postimage verification, rollback, and
  interrupted-edit recovery.
- Deterministic local BMAD Help.
- Development-only direct OpenAI and Azure OpenAI provider transport with host-held in-memory
  credentials.
- A canonical BMAD catalog generated from pinned Method `6.10.0` and Builder `2.1.0` sources.
- One qualified primary BMAD capability, `bmm:bmad-prd`; the other 52 primary Method/Builder
  capabilities remain implementation-pending.

Evidence: [root product overview](../../README.md),
[capability catalog](../../packages/bmad-foundation/capability-catalog.json), and
[baseline gates](../reviews/forge-knowledge-baseline-2026-07-31.md).

## What the current PoC demonstrates

The local Forge launcher can run the active renderer and support API seam while the Tauri host
runs natively. A user can configure a direct provider, review exact context, and exercise the
development request path. Docker proves local configuration and liveness contracts only.

See [Forge local PoC](../guides/forge-local-poc-demo.md).

## Disconnected, partial, or disabled

- The native general-agent tool loop described by ADR-0010 remains proposed/partial rather than a
  completed product-wide runtime.
- Fifty-two primary BMAD capabilities remain implementation-pending; Builder install, activate,
  and execute lifecycles are not qualified.
- No-Entra production credential custody, rotation, revocation, service attribution, and support
  composition remain incomplete.
- Foundry/Azure prerequisites, deployed support-plane qualification, telemetry, signing,
  clean-machine lifecycle, and pilot evidence are separate unfinished gates.
- `.bmad100/run-state.json` is stale relative to the current commit and must be reconciled before
  its task count is treated as current campaign authority.

## Evidence rule

Source checks do not prove installer, Azure, signing, clean-machine, or pilot readiness. See
[release readiness](release-readiness.md) for separate gates.
