---
title: "Risk Register and Mitigation Plan"
aliases:
  - "58 - Risk Register and Mitigation Plan"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 58
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: risk-register
status: v6-modernized-implementation-guide
validated_on: 2026-07-09
---



# Risk Register and Mitigation Plan

## V6.17 architecture risks

| Risk | Delivery | Severity | Mitigation / gate |
|---|---|---|---|
| One “shared runtime” blurs authority | shared | critical | Immutable discriminator, forbidden-edge tests, separate stores/spec audiences/executors |
| Child tool reads outside selected root | desktop | critical | DESK-01 enforcement tier; disable arbitrary commands or narrow claims until proven |
| Reparse/hardlink/path race | desktop | critical | Handle/file-ID validation, root identity, adversarial NTFS suite, fail closed |
| Partial multi-file apply/crash | desktop | high | Preimages, durable journal, per-file atomic replace where supported, startup recovery |
| Source/context exfiltration | desktop | high | Classification/redaction, previewable egress manifest, explicit consent, no source telemetry |
| Azure becomes silent local authority | desktop | critical | Replica-only sync, no local paths/tokens/commands, API negative tests |
| Remote output directly overwrites local work | shared | critical | Separate web record, `cannotApplyDirectly`, local proposal/reapproval/checkpoint |
| Contract drift across C#/Rust/TS | shared | high | Golden vectors, codegen boundaries, version matrix, release gate |
| Cloud executor isolation regression | web | critical | Fixed template/digest/network identity, manifest validation, canary/rollback |
| Desktop signing/update compromise | desktop | critical | Signed provenance/SBOM, pinned update keys, staged rings, rollback/incident runbook |

## 1. Risk scoring

| Severity | Meaning |
|---|---|
| Critical | Can cause unauthorized side effects, data leakage, unrecoverable workspace damage, or release-blocking architecture failure. |
| High | Can break core product promise or create major rework. |
| Medium | Can degrade UX, reliability, cost, or operability but has contained impact. |
| Low | Localized implementation issue. |

## 2. Product and scope risks

| Risk | Severity | Trigger | Mitigation | Owner | Release gate |
|---|---:|---|---|---|---|
| MVP becomes three products again | High | Builder/Artifact/Operator work blocks vertical slice | Enforce master sequence: chat → plan → proposal → approval → job → evidence first | Product/Architecture | First release board contains only vertical-slice stories until gate passes |
| BMAD package breadth distracts from execution substrate | Medium | Parser work expands before patch/test loop works | Only import minimal golden package until substrate works | BMAD Kernel | Package loader cannot block execution-slice milestone |
| Presentation adapter gets rewritten instead of wrapped | High | New PPT workflow appears instead of adapter mapping | Use adapter workbook and golden comparison fixtures | Artifact Creator | Existing-workflow fixture must pass |
| Operator console expands into admin platform too early | Medium | Operator features precede audit/RBAC basics | Limit v1 operator to policy/budget/trace essentials | Product/Security | Non-operator route denial test passes |

## 3. Architecture and state risks

| Risk | Severity | Trigger | Mitigation | Owner | Release gate |
|---|---:|---|---|---|---|
| Runtime API becomes god object | High | Direct table/module access spreads | Ports, ownership map, architecture tests | Backend | Static dependency test passes |
| State transitions become inconsistent | Critical | Worker/API both mutate lifecycle state | API-only lifecycle transition service; workers write manifests only | Backend/Platform | Worker has no SQL credentials; manifest import test passes |
| Airlock bypass | Critical | Side-effect endpoint accepts raw proposal | `ApprovedExecutionSpec`-only side-effect APIs | Security/Backend | Bypass test suite passes |
| Idempotency failure duplicates jobs | High | Retry creates duplicate execution | Idempotency keys, unique constraints, dispatch state machine | Backend | duplicate-dispatch replay fixture passes |
| Stale proposals execute | Critical | New checkpoint after proposal but before approval | preimage/checkpoint validation before spec minting and before apply | Workspace/Airlock | stale proposal test passes |
| Rollback is incomplete | High | Patch applies but validation fails | checkpoint manifest and rollback metadata before/after side effects | Workspace | rollback fixture passes |
| Approval is detached from executable meaning | Critical | Generic/prior approval is reused for a regenerated command/spec | Hash complete `ExecutionSpecCandidate`; approve exact hash; Airlock mints expiring, audience-bound, single-use spec; revalidate mutable inputs/policy | Airlock/Security | candidate A cannot authorize candidate B; reuse/audience/TOCTOU fixtures pass |
| Evidence or work recovery is lossy | Critical | In-memory queue/audit/trace/provider state is treated as authority or crash loses completion | Atomic lifecycle + `EvidenceLedgerEvent` + outbox; durable attempt/lease/completion; idempotent import; replay never executes | Backend/Platform | crash-at-every-boundary and cursor-gap replay pass |

## 4. Security and privacy risks

| Risk | Severity | Trigger | Mitigation | Owner | Release gate |
|---|---:|---|---|---|---|
| Prompt injection from repo content | High | Workspace file instructs model/tool changes | untrusted content wrapper, prompt separation, policy immutability | Model Gateway/Security | OWASP LLM prompt-injection fixtures pass |
| Secret leakage into prompts/traces | Critical | Context/logs include credentials | secret scanner, redaction, privileged raw trace separation | Security | redaction fixtures pass |
| Shell injection through command strings | Critical | Raw command executed by worker | `argv[]` DSL, no shell expansion, deny `sh -c` by default | Airlock/Worker | command DSL deny fixtures pass |
| Path traversal or symlink escape | Critical | Patch writes outside workspace | canonical paths, symlink checks, allowed roots | Workspace/Security | path attack fixtures pass |
| Overbroad worker identity | Critical | Worker can read/write unrelated resources | scoped managed identity/SAS, prefix-limited access, no SQL creds | Platform/Security | identity test passes |
| Unsafe artifact rendering | High | Generated markdown/html executes script | sanitized previews, sandboxed rendering, CSP | Frontend/Security | XSS fixture passes |
| Provider credential exfiltration | Critical | Credential chosen from substring/lookalike base URL or unsafe fallback | Parsed normalized HTTPS endpoint, exact allowlisted host/suffix/path/port/tenant/cloud binding before secret retrieval | Model Gateway/Security | lookalike/user-info/path/redirect/sovereign fixtures pass |
| Provider retention/tool policy violation | Critical | Stored Responses/background/hosted tools bypass app evidence, ZDR, Airlock, or egress | `store=false`; app-owned state; hosted tools off until governed adapters exist | Model Gateway/Data | request contract and provider-state-loss fixtures pass |

## 5. Platform and modernization risks

| Risk | Severity | Trigger | Mitigation | Owner | Release gate |
|---|---:|---|---|---|---|
| ACA Job latency harms UX | Medium | Patch/test loop feels slow | Benchmark jobs; Dynamic Sessions as v1.5 candidate | Platform | latency spike report |
| Dynamic Sessions adopted prematurely | Medium | Sessions become default before cost/isolation proof | keep as spike only until benchmark passes | Architecture | ADR required |
| Provider API churn | Medium | Model SDK/API changes | Model Gateway abstraction; v1 API preference; provider-contract tests | AI Platform | fake + Azure provider tests pass |
| Model retirement breaks profiles | High | Foundry model retirement schedule changes | exact deployment role profiles, four-lane eval, canary, pre-evaluated fallback graph, rollback | Operator/AI | profile health/eval/canary/rollback tests |
| .NET/Node/Python baseline drifts | Medium | local/CI versions diverge | `global.json`, `.nvmrc`/engines, per-worker uv locks, hosted CI version gates; TS6 sidecar isolated from TS7 app compiler | DevEx | clean-machine and hosted-CI version gates |
| TS 7/Node 26 adopted carelessly | Medium | TypeScript 7 tool compatibility or pre-LTS Node 26 breaks build | TS 7 compatibility report, Node 26 LTS wait, watchlist and ADR requirement | Frontend | dependency compatibility report |
| Hardware-incompatible developer path | High | Onboarding/build/test requires local Docker, Kubernetes, emulator, or model server | Direct pinned toolchains and fakes locally; ACR Tasks/hosted CI remote builds; Azure dev integration | DevEx/Platform | no-container clean-Windows smoke passes |
| `sealed_test_fake` mistaken for containment | Critical | Imported package, generated shell, dependency restore, or untrusted workspace command reaches fake executor | No shell/network/import surface; sealed fixture only; first real web isolation is fixed ACA Job | Security/Worker | negative fake-lane escape suite passes |
| Mutable or unproven image reaches ACA | Critical | Local/mutable tag build or runtime job override | Remote immutable build, scan/license/SBOM/provenance/signature/digest, fixed template, start-only identity | Supply Chain/Platform | provenance and override-denial gates pass |
| Cloud prerequisites arrive after consumers | High | Real provider/job work starts before identity/storage/registry/monitoring/rollback | Provision minimal cost-capped Azure foundation in Phase 2 | Architecture/Platform | clean IaC dev deploy and teardown pass before provider/job story |
| Source/license boundary is wrong | High | Root MIT assumed to cover bundled/restrictive content or no immutable ref exists | Source Intake, component inventory/decision, exclude restrictive Hermes PowerPoint skill, Odysseus clean-room | Legal/Supply Chain | provenance/license gate passes |

## 6. Cost and operations risks

| Risk | Severity | Trigger | Mitigation | Owner | Release gate |
|---|---:|---|---|---|---|
| Model cost runaway | Medium | repeated repair/model loops | budget windows, attempt caps, cost events | Operator/Model Gateway | budget-denial fixture |
| Blob retention grows unbounded | Medium | logs/snapshots never expire | lifecycle policy by prefix and retention class | Platform | retention policy deployed |
| Trace evidence too incomplete or too sensitive | High | privacy vs replay conflict | operational/evidence/privileged views with hashes/refs | Trace/Security | evidence bundle fixture |
| Alert fatigue | Low | too many low-signal alerts | dashboard SLOs and severity routing | Ops | alert review before prod |
