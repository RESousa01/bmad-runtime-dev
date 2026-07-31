---
title: "Forge workspace and execution"
doc_type: reference
status: maintained
owner_role: architecture-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - crates/desktop-workspace
  - crates/desktop-app/src/chat_tools.rs
  - crates/desktop-app/src/edits.rs
  - docs/adr/ADR-0009-forge-as-a-general-coding-agent.md
supersedes:
  - docs/archive/legacy-v6/95 - Windows Local Workspace and Execution.md
---

# Forge workspace and execution

## Workspace identity

Folder selection occurs natively. The host returns an opaque workspace identity and grant epoch;
the renderer does not receive a reusable filesystem capability. Revocation invalidates later use.

`WorkspaceBroker` pins operations inside the granted root, normalizes relative paths, bounds
listing/read/search results, and rejects sensitive paths structurally.

## Current read tools

The native chat tool surface currently exposes bounded read-oriented operations through the
permission gate and workspace broker. Refusal occurs before the broker. Workspace output is data
and cannot grant permission or alter policy.

## Planned/partial execution breadth

ADR-0009 accepts read, write, shell, and Git capability under a last-match-wins rule set with
unmatched calls defaulting to `ask`. The complete native tool loop, durable session ownership,
write/shell/Git surface, timeout/output accounting, and recovery behavior remain incomplete.

Do not advertise an individual primitive or candidate adapter as product-wide execution
qualification.

## Writes

File mutations use governed edits: observe preimages, build an exact proposal, review, consume a
fresh decision, checkpoint, apply atomically, verify postimages, journal evidence, and make a
reviewed rollback available. Model origin does not bypass this boundary.
