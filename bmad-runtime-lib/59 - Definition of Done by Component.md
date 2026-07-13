---
title: "Definition of Done by Component"
aliases:
  - "59 - Definition of Done by Component"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 59
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: definition-of-done
status: v6-modernized-implementation-guide
validated_on: 2026-07-09
---



# Definition of Done by Component

## V6.17 completion rule

Every component declares applicability (`shared`, `web_managed`, `windows_local`), authority owner, durable store, executor audience, security claims, offline/degraded behavior, migration/recovery, privacy, observability, fixtures, and release evidence. A shared schema is not completion for either delivery adapter.

Desktop components additionally require Rust/TypeScript IPC contract tests, no generic filesystem/shell capability, adversarial selected-folder tests, crash-at-every-journal-boundary recovery, local evidence verification/export, installer/update/signature tests, model-context consent tests, and the applicable DESK-01 gate. Web components retain Azure identity/network, SQL/Blob transaction, fixed worker template, manifest import, and browser E2E requirements.

## 1. Universal Definition of Done

A component is not done until all applicable checks pass:

- Public APIs or internal ports are documented.
- LLM development mode from [[90 - LLM-Tailored Development Plan and Agent Workflow]] was selected and followed when AI agents implemented the change.
- OpenAPI/JSON Schema examples validate.
- State transitions are tested, including invalid transitions.
- SQL/Blob/event ownership is explicit.
- Security failure modes are tested.
- Observability events and trace refs are emitted.
- Rollback or disable behavior is documented for stateful, provider, worker, package, migration, or user-visible changes.
- New degraded, denied, failed, retried, timed-out, or partially-successful states are visible to a user or operator.
- New persisted data has an owner, schema version, content hash where applicable, and retention class.
- Replay fixture or integration test covers success and at least one failure path.
- Evidence impact is documented.
- Authoritative lifecycle, Evidence Ledger, and outbox writes are atomic where required; telemetry loss cannot change domain truth.
- Source-derived content has immutable provenance where available and a component-level license/notice decision.
- Developer onboarding/tests do not require Docker, Kubernetes, infrastructure emulators, or local model serving; `sealed_test_fake` adapters are never claimed as isolation.
- ADR impact is checked.
- LLM context ledger is updated when an AI agent implemented or materially changed the component.
- CI gates run with the V6 toolchain baseline.

## 2. First-slice components

| Component | Done means | Required tests/evidence |
|---|---|---|
| Chat Workbench | User can complete full vertical slice without leaving app. | Playwright: plan → proposal → approval → execution → evidence. |
| Runtime API | All lifecycle state transitions are authoritative, transactional, and evented. | state-machine tests, idempotency tests, authorization tests. |
| Orchestrator | Typed model output becomes normalized proposals; invalid output cannot proceed. | fake model fixtures and schema-failure tests. |
| Workspace Service | Immutable snapshot, preimage, checkout, checkpoint, rollback metadata work. | preimage drift, stale proposal, rollback fixtures. |
| Context Packs | Context contains provenance, hashes, redactions, and invalidation. | secret redaction and stale context tests. |
| Model Gateway | Fake and Azure adapters return the same typed contract; exact capabilities/credential/retention, schema projection/canonical validation, evaluated role profiles, explicit fallback, and rollback are enforced. | refusal/incomplete/schema, lookalike URL, `store=false`, hosted-tools-off, four-lane eval, canary/fallback/rollback, budget tests. |
| Airlock | Policy evaluates the full `ExecutionSpecCandidate`; required human approval binds its exact hash; only an audience-bound, expiring, single-use `ApprovedExecutionSpec` authorizes a governed effect. | bypass, candidate mutation, reuse, audience, expiry, policy/mutable-input drift, denial fixtures. |
| Execution Jobs | Remotely built digest-pinned fixed ACA template executes; worker writes bounded `WebWorkerResultManifest`/logs/artifacts only; Runtime imports result. | no SQL creds, no runtime override, manifest/spec/image/attempt binding, crash/outbox recovery, redaction, no-local-Docker smoke. |
| Evidence | Atomic `EvidenceLedgerEvent` authority and materialized `EvidenceBundle` reconstruct candidate, policy, approval, attempt, manifest, state, outputs, and rollback; trace loss is tolerated. | crash-at-boundary, cursor gap/upcaster/projection replay, golden bundle, trace-drop fixtures. |

## 3. V1 extension components

| Component | Done means | Required tests/evidence |
|---|---|---|
| BMAD Kernel | Both Method/Builder install profiles normalize deterministically with source/config/skill/help/step/artifact lineage; valid packages activate only after component-license, trust, scan, Azure rehearsal, and approval gates. | golden valid/invalid/mixed profiles, source/license, prompt injection, exact-digest rehearsal, reversible activation. |
| Help Advisor | Suggests source-grounded next actions and blockers. | phase/capability fixtures. |
| Presentation Adapter | Existing workflow behavior preserved through BMAD wrapper. | golden source-to-output comparison. |
| Builder Studio | Draft remains inactive until static validation, Azure-isolated exact-digest install/invocation rehearsal, independent evaluation, approval where required, and reversible activation. | scanner/evaluator unavailable fail-closed, baseline/candidate/trigger/safety fixtures, rollback; no model self-promotion. |
| Operator Console | Admin actions are role-gated, audited, and separated from normal project UX. | RBAC and audit tests. |

## 4. Platform components

| Component | Done means | Required tests/evidence |
|---|---|---|
| Azure IaC | Cost-capped dev/stage environments and fixed job templates can be recreated early from code with least-privilege identities and teardown. | Bicep build/what-if, clean smoke/teardown, role-assignment separation, backup/restore. |
| SQL migrations | Migrations are ordered, reversible where possible, and indexed. | migration up/down or forward-only validation, performance checks. |
| Blob lifecycle | Prefixes, retention, versioning, and access policies match ownership map. | lifecycle policy test, scoped access test. |
| Observability | Dashboards show run, model, execution, policy, and artifact health. | synthetic run produces correlated traces/logs/metrics. |
| Supply chain | ACR Tasks/hosted CI—not a local daemon—builds immutable worker/release artifacts with lock, component-license, scan, SBOM, provenance/attestation, signature, and digest. | clean remote build, artifact verification, mutable-tag and job-override denial. |

## 5. Not done means not done

A feature is not done if it works only in the happy path, requires manual database/blob edits, bypasses candidate-bound Airlock policy, lacks durable replay/evidence, requires a developer to read raw logs, depends on a preview feature without an ADR/rollback, requires local containers/models, treats a fake as containment, or promotes a source/model/package/image without provenance and evaluation evidence.

For AI-authored changes, a feature is also not done if the agent did not report files changed, tests run, verification evidence, skipped checks, remaining risks, rollback/disable status, observability impact, and the next safe step.

If a component needs an exception to ship, the exception must have an owner, reason, compensating control, expiry date, and exit plan. Exceptions without expiry are not valid for v1 release gates.
