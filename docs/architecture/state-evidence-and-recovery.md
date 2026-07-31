---
title: "Forge state, evidence, and recovery"
doc_type: reference
status: maintained
owner_role: architecture-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - crates/desktop-app/src/edits.rs
  - crates/desktop-app/src/recovery.rs
  - crates/desktop-execution
  - crates/desktop-store
supersedes:
  - docs/archive/legacy-v6/96 - Windows Local State, Evidence, Checkpoint, and Rollback.md
---

# Forge state, evidence, and recovery

## Durable state

The local host persists grants, decisions, effect journals, checkpoints, and evidence needed to
explain and recover governed operations. Durable records bind workspace identity/grant epoch,
proposal and manifest hashes, preimages/postimages, decision consumption, and checkpoint identity.

## Governed edit sequence

```text
observe preimages
-> prepare exact change manifest
-> review
-> consume fresh single-use decision
-> persist checkpoint and journal
-> verify preconditions
-> apply atomic UTF-8 changes
-> verify postimages
-> finalize evidence
```

Stale preimages, changed grant epochs, hash drift, malformed journal data, and unsupported recovery
states fail closed.

## Restart recovery

Boot does not mutate workspace files. An interrupted effect remains `recovery_required` and blocks
new updates. After the exact workspace is re-enabled, the user can review a bounded restore and
issue a fresh decision. Successful restore verifies checkpoint bytes and finalizes once.
Interrupted restore becomes `manual_review` rather than retrying automatically.

## Rollback

Undo is a new reviewed proposal that restores checkpoint bytes; it is not an unreviewed reverse
command. The same scope, decision, effect, and evidence boundaries apply.
