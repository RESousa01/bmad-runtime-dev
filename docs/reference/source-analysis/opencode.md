---
title: "OpenCode source analysis"
doc_type: reference
status: blocked
owner_role: architecture-owner
last_verified: 2026-07-31
review_due: 2026-08-31
source_paths:
  - docs/adr/ADR-0010-native-agent-runtime-and-turn-scoped-egress.md
  - docs/archive/library-audits/forge-bmad-runtime-lib-deep-review.md
supersedes: []
---

# OpenCode source analysis

## Review identity limitation

The dated Forge v2 audit compared OpenCode commit
`0a601cf334b9a83cc2854108a2b860f25e6e7e8e`. The current main worktree does not contain
`library/opencode`, and no approved source-lock/storage record is available. This page therefore
does not make path-level reproducibility claims.

## Adopted direction

- General coding-agent interaction patterns and task/session ownership as reference concepts.
- Skill/agent/command discovery and tool-loop structure as behavior to re-derive and test in Rust.
- Permission prompting and explicit tool descriptions as interaction reference.

## Rejected

- OpenCode is not a runtime dependency.
- Forge does not spawn or embed an OpenCode sidecar.
- Renderer or co-process direct IO is incompatible with Forge's host enforcement boundary.

## Current Forge boundary

ADR-0010 proposes a native Rust runtime and explains the sidecar rejection. Until a real immutable
source lock is supplied, this reference remains blocked for detailed citation and externalization
acceptance.

