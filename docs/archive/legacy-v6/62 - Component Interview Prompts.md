---
title: "Component Interview Prompts"
aliases:
  - "62 - Component Interview Prompts"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 62
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: coding-agent-handoff
status: implementation-guide
---



# Component Interview Prompts

## V6.17 mandatory architecture questions

For every component ask: Which delivery model applies? Who owns lifecycle and durable evidence? Which store and executor audience are legal? What cross-authority edges are forbidden? What remains usable offline? What data leaves a desktop and under which consent/retention? What is the proven filesystem/network containment tier? How are crash recovery, rollback, signing/update, schema migration, and contract conformance verified?

Reject answers that use “local” without distinguishing `sealed_test_fake`, `developer_workstation`, and `windows_local`, or that describe Job Objects as a sandbox, cloud sync as authority, remote output as directly applicable, or multi-file patching as atomic.

Use these prompts when assigning work to an implementation agent. They force the agent to inspect contracts before coding.

## Runtime API prompt

```text
Implement the Runtime API slice for Sapphirus BMAD Runtime. First read 11 - Runtime API Control Plane.md, 25 - OpenAPI, Schemas, and Generated Clients.md, 29 - Concurrency, Transactions, and Failures.md, 52 - API, Event, Table, and Blob Ownership.md, and 54 - State Machine Reference.md. Do not add direct cross-module table writes. Implement state transitions through ports, append run events transactionally, and add tests for idempotency and invalid transitions.
```

## Airlock prompt

```text
Implement Airlock policy and approvals. First read 19 - Airlock Policy and Approvals.md and 55 - Airlock Policy Rulebook.md. Ensure every side-effect endpoint requires an ApprovedExecutionSpec created only by Airlock. Add bypass tests for missing spec, expired spec, mismatched proposal hash, mismatched policy hash, stale preimage, and unapproved command class.
```

## Worker prompt

```text
Implement the Python executor worker. First read 20 - Execution Lanes and Container App Jobs.md, 38 - Worker Images and Command DSL.md, and 56 - Worker Manifest Protocol.md. The worker must not write SQL. It must read an immutable spec, validate hashes, run only approved argv commands/actions, redact logs, and write an append-only manifest to Blob.
```

## BMAD package prompt

```text
Implement BMAD package loading. First read 13 - BMAD Kernel, Package Loader, and Help Advisor.md and 39 - BMAD Package Format.md. Parse SKILL.md, module.yaml, module-help.csv, bmad-modules.yaml, TOML config layers, generated skill directories, templates, scripts, evals, and fixtures. Invalid packages must not enter the active catalog.
```

## Presentation adapter prompt

```text
Implement the existing presentation workflow adapter. First read 15 - Artifact Creator and Presentation Adapter.md. Preserve original workflow prompts, stages, source handling, templates, review steps, and export behavior. Add BMAD wrapper metadata, approval checkpoints, provenance, and golden fixture comparison.
```
