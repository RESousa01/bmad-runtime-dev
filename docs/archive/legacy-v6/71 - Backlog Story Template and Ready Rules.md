---
title: "Backlog Story Template and Ready Rules"
aliases:
  - "71 - Backlog Story Template and Ready Rules"
tags:
  - bmad-runtime
  - vault/delivery-plan
section: "Delivery Plan"
order: 71
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: backlog-template
status: implementation-guide
---



# Backlog Story Template and Ready Rules

## V6.17 required story fields

Every story declares: `deliveryModel`, authority owner, allowed stores, workspace target, executor audience, effect class, exact candidate/spec/result contracts, privacy/egress, threat cases, failure/recovery, rollback/disable path, observability/evidence, fixtures, and forbidden cross-authority edges. Shared stories must state that they contain no filesystem, persistence-transaction, approval-token, or executor implementation.

Desktop stories are not Ready without the applicable DESK-01 containment tier, selected-folder/path constraints, IPC surface, offline behavior, signing/update impact, and crash-boundary tests. Remote-handoff stories require explicit selection/consent, separate `web_managed` identity, `cannotApplyDirectly`, and fresh local approval acceptance criteria.

## Story template

```md
# Story S-NNN: Short Story Title

## Goal

## User/system value

## Scope

## Non-scope

## Dependencies

## Story size

Choose one from [[90 - LLM-Tailored Development Plan and Agent Workflow]]:

- XS:
- S:
- M:
- L:
- XL:

L and XL work must be split or preceded by a spike unless the senior reviewer explicitly accepts the exception.

## LLM development mode

Choose one from [[90 - LLM-Tailored Development Plan and Agent Workflow]]:

- Contract-first
- TDD
- Security-test-first
- Characterization/golden
- Replay-driven
- Spike-first
- Migration-first
- UI state-machine
- Refactor-with-shim

## Contracts touched

- APIs:
- Events:
- Tables:
- Blob prefixes:
- JSON Schemas:

## Authority and effect class

- Ordinary authenticated CRUD:
- Governed mutation requiring Airlock policy:
- Exact human approval required:
- Source/build-time intake decision:
- `ExecutionSpecCandidate` fields/hash impact:

## Execution and build lane

- Local deterministic fake only:
- Hosted CI / ACR Tasks remote build:
- Fixed ACA Job real execution:
- Why no local Docker/model/emulator is required:

## Source, license, and supply-chain impact

- Source snapshot/ref/hash:
- Component license/notice decision:
- Lock/SBOM/provenance/digest impact:

## Model and context impact

- ModelProfile/capability/schema projection:
- Evaluation/canary/fallback/rollback:
- Retention (`store=false`) and hosted-tool impact:
- Context trust/redaction/budget/memory-promotion impact:

## Implementation steps

1.
2.
3.

## Security/policy impact

## Failure modes

## Stop conditions

- Stop if:
- Ask for review if:
- Defer if:

## Rollback / disable path

## Observability impact

## Evidence and recovery impact

- `EvidenceLedgerEvent` / EvidenceBundle:
- Attempt/lease/completion/outbox:
- Replay/cursor/upcaster/projection:

## Data ownership and retention

## Tests

- Tests to write first:
- Unit:
- Contract:
- Integration:
- E2E:
- Replay:

## Acceptance criteria

## Documentation updates

## ADR impact

## Exception request

- Needed:
- Owner:
- Expiry:
- Compensating control:
```

## Ready rules

A story is not ready unless:

- It names the component file to follow.
- It links to [[90 - LLM-Tailored Development Plan and Agent Workflow]] when an AI coding agent will implement it.
- It names the LLM development mode.
- It names story size and splits L/XL work unless an explicit exception exists.
- It states whether it is in first vertical slice, v1 extension, v1.5, or v2.
- It lists contract changes.
- It states security/Airlock impact.
- It classifies the effect as ordinary CRUD, governed mutation, high-risk exact approval, or source/build-time intake and names the resulting authority path.
- It selects local-fake, remote-build, and/or fixed-ACA execution lanes and does not require local Docker, Kubernetes, infrastructure emulators, or local model serving.
- It records source/component-license and supply-chain evidence when code, fixtures, packages, dependencies, or images are introduced.
- It records exact ModelProfile/capability/schema/evaluation/retention/fallback impact when the model path changes.
- It names stop conditions for ambiguity, ownership confusion, policy uncertainty, and schema-version uncertainty.
- It defines rollback or disable behavior for stateful, operational, provider, package, worker, or user-visible changes.
- It states observability impact for new success, failure, denial, retry, timeout, or degraded states.
- It states durable Evidence Ledger and attempt/lease/completion/outbox recovery impact separately from telemetry.
- It names data owner and retention class for new persisted data.
- It defines success and failure tests.
- It does not conflict with locked decisions.
- It states which tests must be written before risky implementation.
- It confirms the work does not exceed the WIP limits in [[90 - LLM-Tailored Development Plan and Agent Workflow]].

## Done rules

A story is not done unless:

- Code compiles and tests pass.
- Contracts are regenerated.
- Migration/Blob changes documented.
- Trace/event updates documented.
- Rollback/disable path is implemented, tested, or explicitly marked not applicable with a reason.
- New failure/degraded states are visible to users or operators without raw-log spelunking.
- Context ledger is updated with decisions, files touched, tests, skipped checks, and next safe step.
- Any exception has owner, expiry, compensating control, and exit plan.
- Evidence/replay updated when side effects change.
- ADR updated when a locked/temporary decision changes.
- LLM final report lists files changed, tests run, verification evidence, and remaining risk.
