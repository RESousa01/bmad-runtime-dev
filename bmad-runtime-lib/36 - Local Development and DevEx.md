---
title: "Local Development and DevEx"
aliases:
  - "36 - Local Development and DevEx"
tags:
  - bmad-runtime
  - vault/build-references
section: "Build References"
order: 36
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: implementation-plan-library
status: v6-modernized-implementation-guide
generated_on: 2026-07-09
review_pass: v6-modernization-and-platform-validation
---



# Local Development and DevEx

## V6.17 development meanings and toolchains

Use `developer_workstation` for build/test machines, `sealed_test_fake` for deterministic non-isolating adapters, and `windows_local` only for the installed product. Web development retains the no-local-container path: .NET/Node tests with fakes plus hosted Azure integration environments and remote image builds. Desktop development adds Node/pnpm, stable Rust, Tauri CLI, Windows SDK/MSVC build tools, WebView2, SQLite tooling, signing test certificates, and Windows integration runners.

Neither end-user product requires Docker, Kubernetes, a local server/model/GPU, or desktop access to Azure SQL/Blob. CI has distinct `shared-conformance`, `web`, and `windows-desktop` lanes; only signed release jobs may produce distributable installers.

> This file is part of the V6 implementation library, generated from the project context, review corrections, and the decomposed architecture library.



---

## Implementation-depth contract

This file is part of the V6 implementation library. It is written as an implementation guide, not as a strategy memo. Every component must be built against the same system-wide constraints:

1. **The first executable slice comes before breadth.** The first demonstrable product must prove authenticated chat, workspace context, typed plan output, proposal creation, Airlock validation, approval, isolated execution, validation, checkpoint, and evidence.
2. **The delivery-specific authority owns lifecycle state.** The web Runtime API imports remote-worker facts into SQL; the signed desktop Rust host imports local-executor facts into SQLite. Workers, child processes, renderers, models, sync services, and support APIs do not advance authoritative lifecycle state.
3. **Airlock creates the only side-effect token.** Workspace writes, command runs, exports, package imports, dependency restores, and policy-sensitive actions require an `ApprovedExecutionSpec` issued by Airlock.
4. **The model does not own proposals.** Model Gateway returns typed model outputs. Run Orchestrator creates normalized `Proposal` records. Airlock validates proposals.
5. **No raw shell by default.** Commands are represented as `argv[]` plus policy metadata; `sh -c`, shell expansion, broad environment access, and open network access are blocked unless explicitly operator-approved.
6. **Every side effect is reconstructable.** Diffs, preimages, spec hashes, policy hashes, approvals, job image digests, result manifests, logs, artifacts, and rollback metadata must be traceable.
7. **Each module has ports.** Even inside a modular monolith, use explicit interfaces and contracts to avoid creating a god control plane.


## 1. Component identity

| Field | Value |
|---|---|
| Component | `Local Development and DevEx` |
| Area | `developer experience` |
| Primary implementation package | `repo root tooling` |
| Runtime/technology | `.NET + Node + Python` |
| First-slice priority | `after-core or supporting` |


## 2. Purpose

Define how developers run, test, seed, debug, and validate the platform locally without weakening production security.

The implementation must be narrow enough to fit the corrected first vertical slice, but designed so BMAD package execution, the existing presentation adapter, Builder Studio, SkillOps, replay, and operator controls can plug into the same contracts later.


## 3. Owns / does not own

### Owns
- Detailed implementation guidance
- Cross-reference to related component files
- Acceptance criteria
- Test expectations

### Does not own
- Replacing source context
- Implicit architecture changes without ADR


## 4. Public/API surface and internal ports

### Required API/routes or callable operations
- `See route catalog and block-specific files`


### Internal contract rules

- Every boundary uses typed, schema-versioned values. C# uses `Runtime.Contracts` / `Runtime.Domain`, Rust uses generated contract types plus `desktop-domain`, and TypeScript uses generated web or desktop facade types; no generated DTO grants runtime authority.
- External payloads must be schema-versioned. Internal objects may evolve faster but must not leak into OpenAPI without a contract version.
- Every state mutation must be idempotent or protected by optimistic concurrency.
- Every side-effect operation must receive an `ApprovedExecutionSpec` or be provably read-only.
- Every error response must use the standard error envelope with `code`, `message`, `correlationId`, `retryable`, and optional `detailsRef`.


### Starter interface/type sketch

```python
@dataclass(frozen=True)
class WorkerInvocation:
    job_id: str
    approved_spec_path: Path
    checkout_path: Path
    output_dir: Path
    log_dir: Path
```


## 5. State model

### Component states
- `draft`
- `reviewed`
- `accepted`
- `implemented`
- `verified`


### Generic side-effect lifecycle


```mermaid
stateDiagram-v2
  [*] --> Draft
  Draft --> Validated: schema/contract valid
  Validated --> AwaitingApproval: side effect required
  Validated --> Executable: policy-approved safe action
  AwaitingApproval --> Executable: approved before expiry
  AwaitingApproval --> Rejected: user/operator rejects
  AwaitingApproval --> Expired: approval/spec expiry
  Executable --> Running: dispatched
  Running --> Succeeded: result manifest accepted
  Running --> Failed: manifest failure imported
  Running --> TimedOut: timeout/cancel
  Failed --> RepairPending: repair allowed
  Failed --> UserDecisionRequired: partial success or unsafe retry
  Succeeded --> EvidenceReady: evidence materialized
  EvidenceReady --> [*]
```


## 6. Persistence responsibilities

### SQL tables or domain records touched
- `See data model and DDL starter where applicable`

### Blob/object storage paths touched
- `See blob layout reference where applicable`


### Persistence rules

- In `web_managed`, SQL stores lifecycle state, compact indexes, ownership metadata, and references. In `windows_local`, SQLite stores the corresponding local authority records.
- In `web_managed`, Blob stores large immutable payloads: snapshots, logs, diffs, manifests, artifacts, exports, packages, traces, and validation reports. In `windows_local`, encrypted local content-addressed storage holds authority-owned payloads; cloud upload is explicit and purpose-scoped.
- Any Blob payload referenced from SQL must include content hash, schema version, created timestamp, and retention class.
- No raw secrets, broad credentials, or unredacted prompt/context payloads are stored by default.
- Migrations must be forward-safe and testable against fixture data.


## 7. Detailed implementation steps


### Phase 0 — Contract and spike

1. Create or update the relevant ADR before implementation when the decision affects hosting, policy, security, data ownership, or external dependencies.

2. Define public DTOs and durable JSON schemas first. Do not let implementation classes silently become external contracts.

3. Create a minimal fixture that exercises the component without requiring the whole platform.

4. Add negative tests for the most dangerous bypass or failure case before adding the happy path.

5. Record assumptions in the component file and in the ADR index if they are not final.

6. For `Local Development and DevEx`, implement only the smallest behavior that proves its contract in the first executable slice, then add extended BMAD/Builder/artifact behavior after gate approval.


### Phase 1 — Skeleton implementation

1. Create the package/module/folder with explicit ports/interfaces and dependency direction rules.

2. Add dependency injection registration with narrow interfaces rather than passing broad services everywhere.

3. Implement persistence only through repository/store abstractions that expose business operations, not raw table access.

4. Emit structured events for every important state transition even if the UI does not yet render them.

5. Add unit tests for object creation, invalid input, authorization/policy denial, and idempotency where relevant.

6. For `Local Development and DevEx`, implement only the smallest behavior that proves its contract in the first executable slice, then add extended BMAD/Builder/artifact behavior after gate approval.


### Phase 2 — First vertical integration

1. Connect the component to the first executable slice only. Avoid adding full future scope before the vertical path works.

2. Use fake/stub adapters for expensive external systems until the contract is proven.

3. Make all side effects flow through Proposal → AirlockDecision → Approval/Grant → ApprovedExecutionSpec → Dispatch.

4. Persist large payloads to Blob and store only compact references in SQL.

5. Return UI-consumable run events so the Chat Workbench can render progress without polling raw tables.

6. For `Local Development and DevEx`, implement only the smallest behavior that proves its contract in the first executable slice, then add extended BMAD/Builder/artifact behavior after gate approval.


### Phase 3 — Production hardening

1. Add telemetry attributes, correlation IDs, redaction, and audit events.

2. Add retry, timeout, cancellation, and stale-state handling.

3. Add migration scripts and seed data for dev/test.

4. Add operator visibility for status, errors, budget/policy impact, and cleanup status.

5. Document runbooks for the top failure modes.

6. For `Local Development and DevEx`, implement only the smallest behavior that proves its contract in the first executable slice, then add extended BMAD/Builder/artifact behavior after gate approval.


### Phase 4 — Regression and release gate

1. Add contract tests against OpenAPI/JSON Schema.

2. Add replay fixtures or golden outputs where deterministic behavior is expected.

3. Add security tests for prompt injection, secret leakage, excessive agency, insecure output handling, and supply-chain drift where relevant.

4. Update release gate evidence with screenshots/log excerpts/manifests rather than informal claims.

5. Mark open risks and deferred v1.5/v2 items explicitly.

6. For `Local Development and DevEx`, implement only the smallest behavior that proves its contract in the first executable slice, then add extended BMAD/Builder/artifact behavior after gate approval.


## 8. Validation and test plan

### Required tests
- guide completeness review
- cross-reference check
- acceptance criteria check


### Minimum test layers

| Layer | What to test | Required before merge |
|---|---|---|
| Unit | object validation, state transitions, parsing, policy predicates | yes |
| Contract | OpenAPI/JSON Schema compatibility, generated clients, worker manifests | yes for public/durable payloads |
| Integration | SQL + Blob references, dispatch/import, authz, Airlock boundary | yes for side-effect paths |
| E2E | chat → proposal → approval → execution → evidence | yes for first slice files |
| Replay/golden | BMAD package fixtures, presentation adapter, evidence bundle | yes before v1 beta |
| Security negative | prompt injection, secret leak, policy bypass, path traversal, raw shell | yes for all side-effect components |


## 9. Failure modes and recovery

| Failure | Detection | Required behavior | User/operator visibility |
|---|---|---|---|
| Invalid schema | contract validation | reject before persistence or dispatch | show actionable error with correlation ID |
| Stale proposal/preimage | hash mismatch | void proposal or require rebase/new proposal | show stale context warning |
| Approval expired | expiry check | reject dispatch | show re-approve option |
| Policy mismatch | policy hash mismatch | reject spec | operator audit event |
| Worker timeout | job monitor | mark job timed out; preserve partial logs | timeline event + retry option if safe |
| Manifest missing/invalid | manifest import validation | do not advance success state | incident/failure card |
| Partial success | checkpoint/validation state | enter `user_decision_required` or `kept_for_repair` | explicit decision card |
| Secret detected | scanner/redactor | redact and block if high confidence | security finding card/operator event |


## 10. Security and policy requirements

- Treat workspace files, package files, generated artifacts, model outputs, and logs as untrusted input.
- Never let untrusted content override system instructions, Airlock policy, command allowlists, network policy, or secret handling.
- Enforce project-level authorization on every read and write.
- Log security-relevant denials as audit events, but do not include raw secret values.
- Prefer fail-closed behavior when policy, identity, schema, or storage checks are ambiguous.
- Add negative tests for the most likely bypass path before writing happy-path code.


## 11. Observability

Minimum telemetry fields for this component:

- `correlation.id`
- `project.id`
- `run.id` when available
- `component.name`
- `operation.name`
- `operation.outcome`
- `policy.version` when applicable
- `spec.id` when applicable
- `job.id` when applicable
- `artifact.id` when applicable
- redaction counters, not raw secrets

Metrics to consider: request latency, state-transition count, policy denials, approval wait time, job duration, manifest import failures, schema validation failures, retry count, budget blocks, and evidence materialization time.


## 12. Acceptance criteria

- [ ] The component has a clear owner package and does not leak responsibilities into unrelated modules.
- [ ] Public routes/payloads are represented in OpenAPI/JSON Schema where applicable.
- [ ] Side-effect paths cannot execute without Airlock evaluation and `ApprovedExecutionSpec`.
- [ ] SQL lifecycle state is mutated only by the Runtime API/Application layer.
- [ ] Blob payloads have content hashes and schema versions.
- [ ] Tests include at least one negative/bypass case.
- [ ] Events and evidence are emitted for user-visible actions.
- [ ] The component is represented in the release gate matrix.
- [ ] The implementation does not introduce Cortex as a runtime namespace.
- [ ] Documentation includes deferred v1.5/v2 scope explicitly rather than silently omitting it.


## 13. Integration checklist

- [ ] Update `32 - Integration Contract Map.md` with any new caller/callee relationship.
- [ ] Update `25 - OpenAPI, Schemas, and Generated Clients.md` for public route or schema changes.
- [ ] Update `22 - Data Model - SQL and Blob.md`, `47 - Database DDL Starter.md`, or `48 - Blob Storage Layout.md` for persistence changes.
- [ ] Update `27 - Testing, Validation, and Replay.md` for new fixtures or replay needs.
- [ ] Update `33 - Release Gates and Acceptance Matrix.md` if the change affects release readiness.
- [ ] Add or update ADR in `31 - Architecture Decision Records.md` if the change alters architecture, hosting, policy, or security posture.


---

## Historical Revision Notes (V3 -> V4)
## Review finding

`36 - Local Development and DevEx.md` is part of the implementation library support layer. In v3, support files were useful but not always testable. In v4, every support file must provide either a decision, reference contract, release gate, mapping, runbook, or checklist that can be executed by a developer or coding agent.

## Required usage

1. Read this file before changing the related implementation area.
2. Cross-check it against `07 - Source Coverage Matrix.md` and `50 - V4 Full Library Audit.md`.
3. When implementing a task, copy the relevant checklist items into the issue/story.
4. When a decision changes, update this file and `31 - Architecture Decision Records.md` in the same PR.
5. When a contract changes, update `25 - OpenAPI, Schemas, and Generated Clients.md`, `46 - API Route Catalog.md`, and generated clients.

## V4 quality rules for this file

- It must not contradict locked architecture decisions.
- It must not reintroduce a broad v1 scope that competes with the executable vertical slice.
- It must preserve BMAD source contracts and the existing presentation workflow adapter decision.
- It must reflect the Runtime API as lifecycle state owner and the worker as manifest/log producer only.
- It must identify whether guidance is `LOCKED`, `TEMPORARY`, `PHASE-0 SPIKE`, `V1`, `V1.5`, or `V2`.

## Implementation checklist linkages

| Related guide | What to cross-check |
|---|---|
| `01 - First Build - Executable Vertical Slice.md` | Does this file support or distract from the first slice? |
| `29 - Concurrency, Transactions, and Failures.md` | Are state and partial failure semantics compatible? |
| `32 - Integration Contract Map.md` | Are producer/consumer boundaries clear? |
| `33 - Release Gates and Acceptance Matrix.md` | Is there a release gate for this guidance? |
| `49 - Detailed Component Build Checklists.md` | Are implementation tasks represented as checklist items? |


## V6 modern local toolchain baseline

Local development is deliberately lightweight. It must validate contracts and deterministic domain behavior without requiring a local container engine, Kubernetes, infrastructure emulators, or local model serving. Hosted CI and the Azure development environment are the first real integration environments.

| Tool | Required baseline | Local enforcement | CI enforcement |
|---|---|---|---|
| .NET SDK | .NET 10.x LTS | `global.json` with exact supported feature band and roll-forward policy | `dotnet --info` gate before restore/build/test |
| Node.js | 24.x LTS | `.nvmrc`/Volta or direct install | `node --version` gate and `engines` check |
| pnpm | 11.x | `packageManager` field and `corepack`/pinned install | `pnpm install --frozen-lockfile` |
| TypeScript | 7.x application compiler | workspace dev dependency; pinned TS 6 sidecar only for compiler-API consumers | `pnpm typecheck`, generated-client type tests, and explicit sidecar isolation gate |
| Python | Per-worker locked profile; 3.14 preferred after dependency gate | `uv python pin` and `uv sync --locked` | hosted worker tests and cloud-built image smoke |
| Azure tooling | Azure CLI + Bicep; Azure Developer CLI only if adopted | direct install or setup script | IaC validation, environment checks, Bicep build/what-if, and remote image build |

### Required repository files

```text
/.config/dotnet-tools.json
/.editorconfig
/.nvmrc
/global.json
/package.json                 # packageManager and engines
/pnpm-workspace.yaml
/pnpm-lock.yaml
/services/runtime-api/...     # ASP.NET Core web-managed authority
/apps/web/...                 # React browser workbench
/apps/desktop-ui/...          # React Tauri/WebView2 workbench
/packages/ui/...              # shared tokens, primitives, and delivery-neutral views
/workers/pyproject.toml
/workers/uv.lock
/infra/main.bicep
/azure.yaml                   # azd orchestration if adopted
/.devcontainer/devcontainer.json # optional convenience only; never required
```

### Modern DevEx rule

Do not make Aspire, Docker Desktop, WSL container integration, Kubernetes, Azurite, SQL Edge, or a local model server part of the baseline. Bicep/AVM plus explicit environment configuration is the deployment source of truth. A future convenience profile may be added only when it remains optional and the documented no-container workflow passes from a clean machine.

### Locked no-container development profile

The baseline developer loop is:

1. run unit, contract, schema, parser, policy, generated-client, and replay tests directly with pinned .NET, Node, and Python toolchains;
2. use deterministic in-process fakes and temporary test workspaces for model, worker, SQL/Blob, event-stream, and identity ports;
3. run the sealed BMAD foundation fixture only through trusted fake behavior—never execute imported package code, generated shell, dependency restores, or untrusted workspace commands locally;
4. build container images remotely with ACR Tasks (`az acr build`) or hosted CI and record the source revision, build definition, SBOM/provenance, and resulting digest;
5. run the first real isolated execution in a fixed-template Azure Container Apps Job in the minimal development environment.

The fake executor proves orchestration and evidence contracts. It is not described, tested, or trusted as a sandbox.

## Odysseus-Informed Local Smoke Tests

Source: [[88 - Odysseus Source Code Review - Self-Hosted AI Workspace]].

Local development needs a fresh-install path, not just incremental developer setup:

| Check | Requirement |
|---|---|
| Fresh clone smoke | One command validates API, web, generated clients, in-memory/fake stores, fake worker contract, and fake model provider without Docker or infrastructure emulators. |
| First admin/operator setup | Local setup proves privileged routes are inaccessible before setup and scoped after setup. |
| Optional dependency degradation | Search, vector memory, email, notification, and provider probes report degraded state clearly when disabled. |
| Provider-free startup | The application starts and runs replay fixtures without provider credentials or a local model server; real-provider tests are an explicit Azure lane. |
| Copyable diagnostics | Failed commands show redacted command, exit code, stderr/stdout summary, log ref, and next suggested check. |
| Offline assets | Self-host mode does not depend on public CDNs for required UI assets. |

## Consolidated Source-Review DevEx Gates

Source: [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]].

Local development must prove these before a PR is considered ready:

| Gate | Command or artifact expectation |
|---|---|
| Contract compile | Generated TypeScript client and .NET DTOs compile from the canonical OpenAPI/JSON Schema source. |
| TypeScript 7 compatibility | `tsc --build` and the web bundler pass on pinned TypeScript 7; compatibility fallback requires ADR note. |
| Fake vertical slice | Fake provider and fake worker produce proposal, approval requirement, approved spec, manifest, checkpoint, and evidence. |
| Egress fixtures | URL policy unit tests pass for private networks, redirects, metadata endpoints, and DNS failure. |
| Package rehearsal fixture | Local tests cover deterministic parse/static scan and denial paths; install and invocation rehearsal that executes package code runs only in the Azure isolated lane. |
| Degraded optional services | Disabled search/vector/email/notification/provider probes surface clear degraded status instead of hidden failures. |
| No-container clean-machine gate | The documented workflow passes on a machine without Docker, Kubernetes, infrastructure emulators, or local model serving. |
| Remote image-build gate | ACR Tasks or hosted CI emits a digest-pinned image plus build provenance/SBOM, and the ACA Job smoke imports a valid `WebWorkerResultManifest`. |
