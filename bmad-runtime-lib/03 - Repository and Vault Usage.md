---
title: "Repository and Vault Usage"
aliases:
  - "03 - Repository and Vault Usage"
tags:
  - bmad-runtime
  - vault/foundation
section: "Foundation"
order: 3
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: implementation-plan-library
status: v6-modernized-validated-implementation-guide
generated_on: 2026-07-09
review_pass: v6-modernization-and-platform-validation
architecture_rule: governed-chat-first-agentic-runtime
---



# Repository and Vault Usage

## 0. Dual-product repository boundary (V6.17)

The target repository separates delivery-specific authorities and shares only schemas, fixtures, BMAD semantics, and presentation components:

```text
sapphirus-bmad-runtime/
  apps/
    web/                         # React browser client
    desktop-ui/                  # React UI rendered by Tauri/WebView2
  services/
    runtime-api/                 # ASP.NET web-managed authority
    desktop-support-api/         # identity/licensing/model/packages/sync/remote jobs
  crates/
    desktop-host/                # Tauri command/event boundary
    desktop-domain/              # local lifecycle and Airlock
    desktop-workspace/           # folder capabilities, context, patch/checkpoint
    desktop-runner/              # structured Win32 process execution
    desktop-store/               # SQLite, encrypted CAS, recovery
  workers/                       # fixed web/explicit-remote worker images
  packages/
    contracts/                   # canonical JSON Schema/OpenAPI/IPC schemas
    ui/                          # delivery-neutral React components
    bmad-fixtures/               # signed packages and golden semantics
  tests/
    web/ desktop/ conformance/ security/ replay/
```

Code sharing may not introduce a common filesystem adapter, state database, approval token, or executor implementation. See [[99 - Dual-Delivery Contract and Conformance Specification]].

## 1. Repository Shape

The V6.17 tree in section 0 is canonical. The older `src/*` map below is retained only to explain migration from prior planning passes; implementation agents must not create `src/web`. Browser routes belong in `apps/web`, desktop React routes in `apps/desktop-ui`, and shared visual components/tokens in `packages/ui`.

```text
sapphirus-bmad-runtime/
  docs/
    plan-library/                  # these Markdown files
    adr/                           # accepted architecture decision records
    api/                           # OpenAPI and schema docs
  src/
    web/                           # DEPRECATED legacy path; do not scaffold
    runtime-api/                   # ASP.NET Core modular monolith
    workers/                       # Python worker images
    shared-contracts/              # generated schemas/clients, no authority or business logic
  infra/
    bicep/                         # Azure resources
    containerapps/                 # ACA app/job config
    pipelines/                     # CI/CD definitions
  tests/
    contract/                      # OpenAPI/schema compatibility
    integration/                   # API + SQL + Blob + worker harness
    policy/                        # Airlock tests
    replay/                        # workflow replay fixtures
    e2e/                           # Playwright chat-to-evidence flows
  fixtures/
    repos/                         # tiny and medium source repos
    bmad-packages/                 # valid/invalid package fixtures
    presentation-workflow/         # golden inputs/outputs
```

## 2. Source Ownership

| Area | Primary Language | Owner Module |
|---|---|---|
| Web Chat UI | TypeScript/React | `apps/web` |
| Desktop Chat UI | TypeScript/React | `apps/desktop-ui` |
| Shared design system and delivery-neutral views | TypeScript/React | `packages/ui` |
| Runtime control plane | C#/ASP.NET Core | `src/runtime-api` |
| Workers | Python | `src/workers` |
| Contracts | OpenAPI/JSON Schema | `src/shared-contracts` and `docs/api` |
| Infra | Bicep/YAML | `infra` |
| Tests | C#, TS, Python | `tests` |

## 3. Runtime API Internal Module Layout

```text
src/runtime-api/
  Sapphirus.Runtime.Api/           # HTTP endpoints, auth, middleware
  Sapphirus.Runtime.Application/   # orchestration services and ports
  Sapphirus.Runtime.Domain/        # domain objects and state machines
  Sapphirus.Runtime.Infrastructure/# SQL/Blob/KeyVault/SignalR adapters
  Sapphirus.Runtime.Bmad/          # BMAD parser/kernel/help advisor
  Sapphirus.Runtime.Airlock/       # pure policy kernel
  Sapphirus.Runtime.Workspaces/    # snapshots/checkouts/preimages/checkpoints
  Sapphirus.Runtime.ModelGateway/  # model calls and structured output validation
  Sapphirus.Runtime.Observability/ # OpenTelemetry, audit, trace writer
```

## 4. Worker Layout

```text
src/workers/
  base-worker/                     # common manifest protocol, redaction, logging
  patch-worker/                    # apply patch and produce diff result
  command-worker/                  # argv command execution
  package-validation-worker/       # BMAD package validation
  artifact-export-worker/          # document/presentation exports
  scan-worker/                     # async workspace scanning/indexing
```

## 5. Documentation Maintenance Rules

- Update the relevant architecture block file when implementation changes a boundary or contract.
- Add an ADR when a `LOCKED` decision changes.
- Do not bury architectural changes inside PR descriptions only.
- Keep diagrams executable Mermaid where possible.
- Keep examples schema-valid or mark them as pseudocode.
- Every file should include failure semantics, not just happy path.

## 6. File-to-Code Traceability

| Markdown file | Code areas |
|---|---|
| `10 - Chat Workbench.md` | `apps/web`, `apps/desktop-ui`, `packages/ui`, generated clients, E2E tests |
| `11 - Runtime API Control Plane.md` | API host, Application ports, Domain state |
| `12 - Run Orchestrator and Agent Kernel.md` | Orchestrator service, proposal creation, run state |
| `13 - BMAD Kernel, Package Loader, and Help Advisor.md` | `Sapphirus.Runtime.Bmad` |
| `16 - Workspace Service.md` | `Sapphirus.Runtime.Workspaces`, workers mount protocol |
| `19 - Airlock Policy and Approvals.md` | `Sapphirus.Runtime.Airlock`, policy tests |
| `20 - Execution Lanes and Container App Jobs.md` | worker images, ACA Jobs, dispatcher |
| `21 - Trace, Evidence, and Observability.md` | trace writer, evidence bundle, OpenTelemetry |
| `25 - OpenAPI, Schemas, and Generated Clients.md` | OpenAPI, JSON Schema, generated SDKs |

## 7. Pull Request Checklist

Every PR touching runtime behavior must answer:

1. Which plan-library file changed?
2. Which API/schema changed?
3. Which state machine changed?
4. Which Airlock rule is involved?
5. Which trace/evidence event is emitted?
6. Which replay fixture proves the behavior?
7. What is the rollback behavior?

---

## v2 Review Improvements

### 1. Proposed Monorepo Shape

```text
sapphirus-bmad-runtime/
  docs/
    implementation-library/
    adr/
    threat-model/
    api/
  src/
    web/                         # React + TypeScript
    runtime-api/                 # ASP.NET Core modular monolith
      Modules/
        Threads/
        Runs/
        Proposals/
        Airlock/
        Workspace/
        Execution/
        Bmad/
        ModelGateway/
        Evidence/
        Operator/
      Contracts/
      Infrastructure/
      Tests/
    workers/
      common/
      patch-worker/
      command-worker/
      bmad-validation-worker/
      artifact-export-worker/
    contracts/
      openapi/
      json-schema/
      generated/
    infra/
      bicep/
      environments/
      scripts/
    fixtures/
      sample-react-app/
      bmad-packages/
      prompt-injection/
      secret-redaction/
```

### 2. Code Ownership Map

| Area | Owner Role | Required Review |
|---|---|---|
| Airlock policy | Security/architecture | Mandatory security review. |
| Execution workers | Runtime/security | Worker image + policy review. |
| Workspace Service | Runtime | Concurrency/rollback review. |
| Model Gateway | AI platform | Cost/privacy/schema review. |
| BMAD Kernel | Method/runtime | BMAD contract validation. |
| Frontend workbench | Product/frontend | Accessibility and event-contract review. |
| Operator Console | Ops/security | RBAC and audit review. |
| IaC | Platform | Environment and secret review. |

### 3. Documentation Synchronization Rule

Every implementation PR must update exactly one of these:

- block file in `docs/implementation-library/`;
- ADR in `docs/adr/`;
- OpenAPI/JSON Schema in `src/contracts/`;
- test fixture in `fixtures/`.

A PR that changes runtime behavior but updates none of these is incomplete.

### 4. Generated Artifacts Policy

Generated files must be either:

- committed with generator version and input hash; or
- generated in CI and validated against a checked-in golden copy.

For API clients:

```text
OpenAPI source → generated TypeScript client → generated C# client/DTOs → compile/test gate
```

Manual edits to generated clients are prohibited.

### 5. Local Development Commands

Initial scripts should expose one command per validation class:

```bash
pnpm --dir apps/web lint
pnpm --dir apps/web test
pnpm --dir apps/web build
pnpm --dir packages/ui test
dotnet test services/runtime-api/Tests
uv run --directory workers/patch-worker pytest
uv run --directory workers/command-worker pytest
./infra/scripts/validate-bicep.sh
./packages/contracts/scripts/validate-openapi.sh
```

These commands become candidates for Airlock allowlists only after they are deterministic in CI.


---

## Historical Revision Notes (V3 -> V4)
## Review finding

`03 - Repository and Vault Usage.md` is part of the implementation library support layer. In v3, support files were useful but not always testable. In v4, every support file must provide either a decision, reference contract, release gate, mapping, runbook, or checklist that can be executed by a developer or coding agent.

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
