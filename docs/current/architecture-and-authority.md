---
title: "Forge architecture and authority"
doc_type: current
status: current
owner_role: architecture-owner
last_verified: 2026-07-31
review_due: 2026-08-14
source_commit_from: "docs/provenance/knowledge-migration-baseline.json#/implementationWorktree/currentHead"
source_paths:
  - crates/desktop-app
  - crates/desktop-runtime
  - crates/desktop-store
  - apps/desktop-ui
  - docs/adr
supersedes:
  - docs/archive/legacy-v1-knowledge/knowledge-base/current/01-architecture-and-ownership.md
---

# Forge architecture and authority

## Runtime authority

`desktop-app` is the sole native composition root. It owns:

- workspace grants and sensitive-path policy;
- durable state and evidence;
- context review and egress decisions;
- provider credentials and transports;
- governed edit decisions, checkpoints, effects, rollback, and recovery;
- capability preparation and host-side identity binding.

The renderer receives typed projections and submits typed intents. It does not receive generic
filesystem, process, credential, database, or updater authority.

## Execution direction

[ADR-0009](../adr/ADR-0009-forge-as-a-general-coding-agent.md) accepts Forge as a general coding
agent whose tools are host-governed and whose organizing interface is BMAD.
[ADR-0010](../adr/ADR-0010-native-agent-runtime-and-turn-scoped-egress.md) is still proposed and
defines the intended native Rust tool loop and turn-scoped egress accounting. Proposed behavior
must not be described as fully implemented until source and lifecycle gates prove it.

## Data and effects

Workspace content is data, never instruction. Reads are bounded and sensitive paths fail closed.
Model output grants no authority. Proposed writes return through the same exact review,
single-use decision, checkpoint, atomic apply, post-validation, evidence, and rollback boundary as
human-originated changes.

## Documentation authority

The precedence model is:

```text
source/tests/gates > accepted ADR > docs/current > runbook > active plan
> dated review > source analysis > archive
```

See [ADR-0011](../adr/ADR-0011-documentation-authority-and-reference-boundary.md).

## Stable compatibility identifiers

Some schemas, packages, environment variables, binaries, and persisted values retain legacy
internal names. They are compatibility IDs, not the product name, and require a separate tested
migration before they can change. See [ADR-0013](../adr/ADR-0013-forge-name-and-legacy-identifier-compatibility.md).
