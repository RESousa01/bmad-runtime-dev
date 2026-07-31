---
title: "Deprecation and Preview Watchlist"
aliases:
  - "78 - Deprecation and Preview Watchlist"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 78
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-deprecation-preview-watchlist
status: legacy-reference
validated_on: 2026-07-09
---



# Deprecation and Preview Watchlist

## V6.17 desktop watchlist

Track Tauri 2 security/capability/updater changes, WebView2 runtime servicing, Windows app packaging/signing requirements, WAM/OAuth APIs, Windows sandbox/AppContainer/restricted-token behavior, Rust MSRV and critical crates, SQLite format/bindings, enterprise deployment channel policy, Azure AI Foundry/OpenAI API/model/retention changes, Entra public-client/Conditional Access changes, and Azure Monitor desktop SDK/privacy behavior.

Preview features cannot become the sole identity, containment, update, recovery, or model-access path without an accepted ADR, supported fallback, and release evidence.

This file prevents “modern” from becoming “unstable.” Modern baseline means current and supportable, not blindly adopting every preview.

## 1. Avoid for v1 baseline

| Item | Status | Risk | V6 action |
|---|---|---|---|
| .NET 8 / .NET 9 for new API code | Supported until Nov 10, 2026 | Short runway for a new product. | Do not start new Runtime API on these. Use .NET 10 LTS. |
| Node.js 26 | Current, scheduled for Active LTS on Oct 28, 2026 | Not LTS during this validation. | Defer until LTS and dependency audit. |
| TypeScript 6.x as default application compiler | Superseded by TypeScript 7.0 GA on Jul 8, 2026 | Staying on the old compiler loses the native compiler performance baseline. | Use the TypeScript 7 CLI only after compatibility gates; keep an exact, isolated TS 6 alias solely where tooling needs the programmatic API. |
| TypeScript 7 as an unconditional ecosystem lock | GA compiler, but 7.0 does not ship a public programmatic API | Lint, generators, embedded tooling, and editor integrations can still require TS 6 APIs. | Keep status `SPIKE_REQUIRED` until generated-client, lint, Vite, declaration, and editor/LSP evidence passes. |
| OpenAPI 3.2 as the v1 canonical contract | Published standard; native ASP.NET Core generation starts with .NET 11 | Hand-authoring 3.2 while .NET 10 generates 3.1 creates dual truth and client-generator drift. | Use OpenAPI 3.1.2 for v1; re-evaluate 3.2 with .NET 11 and full generator/validator compatibility evidence. |
| Azure AI Inference beta SDK | Deprecated/retirement path appears in Microsoft docs. | SDK churn and migration risk. | Prefer OpenAI/v1-compatible API path where applicable. |
| Foundry classic agents | Deprecation path toward new Foundry Agent Service. | Building against classic agent semantics creates migration debt. | Do not use classic agents. |
| Foundry Agent Service as v1 orchestrator | Managed agent platform, but product has custom governance semantics. | Could hide Airlock/run-state/evidence control. | Spike only after custom vertical slice exists. |
| Dynamic Sessions as primary executor | Useful but cost/latency/isolation still product-specific. | Premature infra complexity. | Benchmark against ACA Jobs before adoption. |
| Container Apps Sandboxes as v1 baseline | Newer preview isolation surface with region/product-fit uncertainty. | Preview coupling and a second execution protocol before fixed Jobs are proven. | Keep on the post-Phase-4 spike list; adoption must preserve the same spec/result/evidence contracts. |
| Local Docker/model/emulator workflow as required baseline | Common developer convention but unsupported by the user's hardware constraint. | Blocks onboarding and creates a second deployment truth. | Direct toolchains/fakes locally; ACR Tasks/hosted CI remote builds; Azure dev resources for real integration. |
| ACA Flex workload profile | Preview on the validation date | Preview capacity/networking behavior is not appropriate as an unreviewed production dependency. | Use supported Consumption/Dedicated profiles; adopt Flex only through an explicit preview-risk ADR. |
| Responses background mode/provider-stored state | Available capability; background execution requires stored provider state. | Conflicts with app-owned lifecycle/evidence and default minimal-retention posture. | Keep `store=false` for v1 calls; spike background mode only with an approved retention class and replay/evidence design. |
| Provider-hosted model tools as default | Responses can expose hosted web, code, computer, and MCP-style tools. | Provider approval flags do not enforce Sapphirus Tool Registry, Airlock, workspace, egress, or evidence rules. | Disable by default; adopt each hosted tool only as a registered capability with policy, approval, sandbox, and evidence tests. |
| OpenTelemetry GenAI semantic conventions as a locked domain contract | Conventions continue to evolve. | Attribute churn can break dashboards and leak high-cardinality or sensitive prompt/tool data. | Keep stable internal `sapphirus.*` attributes and version-pinned mappings; never emit raw prompts/outputs/tool arguments as span attributes. |

## 2. Watch monthly

- Azure OpenAI / Foundry model retirement schedule.
- Azure OpenAI v1 API support and SDK behavior.
- Structured-output schema limits by deployed model.
- Responses retention, `store=false`, background-mode, and field/tool parity by deployed provider/model.
- ACA Jobs and Dynamic Sessions limits/quotas/region availability.
- ACR Tasks quotas, builder/runtime identity separation, remote-build provenance, and Container Apps Sandboxes preview/region status.
- ACA Jobs start-template override behavior and custom-role permissions.
- ACA workload-profile preview status, especially Flex.
- Node.js release line status.
- TypeScript 7 point releases, public API availability, editor support, Vite/lint/generated-client compatibility, and TS 6 sidecar retirement.
- Vite 8 and React Router 8 supported patch lines and their Node/React compatibility floors.
- Python 3.14 dependency compatibility for worker packages.
- pnpm security/trust-policy changes.
- OpenAPI 3.2 support across .NET, generators, validators, and mock servers.
- OpenTelemetry generative-AI semantic-convention stability and Azure browser telemetry guidance.
- ACR private-endpoint/Premium requirements and hosted-CI connectivity.

## 3. Required response to a watched change

When a watched platform changes, update:

1. this file;
2. `60 - External Platform References and Verification Sources.md`;
3. `76 - Current Stack Baseline.md`;
4. affected component guide;
5. `31 - Architecture Decision Records.md` if a decision changes;
6. hosted CI and local-toolchain pins if a baseline changes; optional devcontainer pins only when that convenience profile exists.
