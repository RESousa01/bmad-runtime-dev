---
title: "Targeted Corrections"
aliases:
  - "74 - Targeted Corrections"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 74
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-targeted-corrections
status: complete
---



# Targeted Corrections

## V6.17 targeted corrections

- Corrected a single cloud architecture being treated as universal: it is now explicitly `web_managed`.
- Added an independent `windows_local` Tauri/Rust authority instead of weakening the cloud boundary.
- Replaced ambiguous “local fake” language with `sealed_test_fake`.
- Corrected project/run discriminator wording: `Project.deliveryModel` is immutable and inherited by runs.
- Replaced multi-file “atomic patch” language with journaled crash-recoverable batch apply and per-file atomic replacement where supported.
- Corrected Job Object claims: process tree/resource control only, not filesystem/network isolation.
- Made desktop Azure functions a support/replica plane and forbade direct local state/file authority.
- Made remote jobs explicit cross-delivery handoffs whose outputs require fresh local policy, approval, checkpoint, and apply.

This file records concrete corrections made after reviewing V4. The goal was not to increase size. The goal was to remove ambiguity, stale labels, and unsupported assertions.

## 1. Corrections made

| Area | V4 issue | V6 correction |
|---|---|---|
| Start Here identity | Start Here still identified the library as V3 even though V4 had been produced. | Rewrote Start Here as V6 current implementation library. |
| External platform references | External facts were listed without validation status or source URLs. | Rewrote `60 - External Platform References and Verification Sources.md` with validated statements, decision impacts, and official sources. |
| Validation traceability | No single register separated project-source claims, external facts, architecture decisions, implementation specs, and spikes. | Added `73 - Verification Register.md`. |
| Future validation | No repeatable protocol for future edits. | Added `75 - Library Validation Protocol.md`. |
| Dynamic Sessions | Risk of reading Dynamic Sessions as a v1 execution baseline. | Marked as external capability plus Phase-0/v1.5 candidate only. |
| JSON mode / structured outputs | Risk of treating JSON mode as schema-safe. | Explicitly states JSON mode is not schema-conformant; structured outputs plus server validation are required. |
| SLSA | Risk of citing retired or stale SLSA versions. | Added current-version caution and source. |
| App Service auth | Risk of conflating authentication with project authorization. | Clarified that Entra/App Service auth is identity entry; Runtime API still owns project/resource RBAC. |
| SQL/Blob split | Risk of treating SQL as a generic event/log sink. | Reaffirmed SQL compact lifecycle state; Blob bulky payloads; streaming event channel for live logs. |
| Worker authority | Risk of workers writing authoritative state. | Reaffirmed worker manifests only; Runtime API imports manifests and mutates SQL state. |

## 2. Active guidance after V6

The current library should be read this way:

1. Source context and critical review are preserved records, not edited implementation files.
2. Active implementation files are V6 validated unless marked historical.
3. External platform claims must be checked against `73 - Verification Register.md`.
4. Product decisions must be checked against `02 - Locked Architecture Decisions.md` and `31 - Architecture Decision Records.md`.
5. Component implementation must be checked against `52 - API, Event, Table, and Blob Ownership.md`, `63 - Backend Port Interfaces.md`, and each component-specific guide.

## 3. Corrections deliberately not made

| Item | Reason |
|---|---|
| Did not delete preserved original context. | It is the audit baseline; deleting it would make coverage unverifiable. |
| Did not delete preserved critical review. | It anchors the scope and architecture corrections. |
| Did not remove the historical V4 audit. | It is useful version history; V6 adds a new validation layer instead. |
| Did not claim every project assumption is externally true. | Some are architecture choices and must remain ADR-controlled. |
| Did not promote Dynamic Sessions into v1. | Product fit requires latency/isolation/cost evidence. |
| Did not mark SignalR as permanently locked. | SSE may be enough for simple event streams; this remains ADR-021. |

## 4. Residual work before implementation

- Write ADR-001 through ADR-010 before stabilizing architecture.
- Run ACA Job latency benchmark before committing UX expectations for command loops.
- Run structured-output schema-limit spike against the exact Azure OpenAI model/profile selected.
- Build one fake-model vertical slice before integrating real models.
- Build worker-manifest import tests before any real command execution.
- Run policy bypass tests before allowing patch apply, command run, package import, or artifact export.
- Build source-alignment tests against real BMAD package fixtures.
