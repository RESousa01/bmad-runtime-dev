---
title: "Current Stack Baseline"
aliases:
  - "76 - Current Stack Baseline"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 76
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-modern-stack-baseline
status: current
validated_on: 2026-07-09
---



# Current Stack Baseline

## V6.17 current stack matrix

| Layer | Shared | `web_managed` | `windows_local` |
|---|---|---|---|
| UI | React/TypeScript, shared components | Vite SPA in browser | Vite UI in Tauri/WebView2 |
| Domain | JSON Schema/BMAD/Airlock semantics | .NET/ASP.NET Core modular monolith | Stable Rust/Tauri native host |
| State/evidence | canonical envelopes/hashes | Azure SQL + Blob | SQLite WAL + encrypted local CAS |
| Workspace | `WorkspaceTarget` union | immutable cloud snapshot/checkouts | user-selected folder capability |
| Execution | candidate/spec/result unions | fixed Azure remote worker; ACA reference | Rust patch engine + Win32 structured runner |
| Identity/model | Entra/model profiles | control-plane managed identity to Foundry/OpenAI | public-client sign-in → Model Access API → managed identity |
| Delivery | signed provenance/SBOM | Azure IaC/web/API/containers | signed MSI/NSIS/updater/enterprise rings |

No local Docker, Kubernetes, self-hosted server, local model, or GPU is an end-user requirement.

This file converts the library from “modern Azure stack in general” into explicit build targets. It intentionally avoids preview-first choices unless the feature is isolated behind a spike.

## 1. Baseline matrix

| Component | Baseline | Why this is the right 2026 choice | Validation status |
|---|---|---|---|
| Runtime API | ASP.NET Core runtime 10.0.9 on .NET 10 LTS; SDK 10.0.301 | Current .NET LTS, active support through 2028-11-14; monthly patch currency is required for support. | `EXTERNAL_VERIFIED` + `LOCKED` |
| Web app | React 19.2.7 client SPA + TypeScript 7.0 application compiler | React is stable. TypeScript 7 is GA but has no public programmatic API in 7.0, so generated clients, lint, bundler, declarations, and editor tooling must pass; a pinned TS6 package is isolated only for compiler-API consumers. | `REACT_LOCKED` + `TYPESCRIPT_LOCKED_WITH_GATE` |
| Web delivery | Vite 8.1.0 + React Router 8.0.1 SPA mode | The current product needs an authenticated client workbench, not SEO/SSR. Add Next.js or another server framework only through an ADR proving an SSR/BFF requirement. | `LOCKED_WITH_GATE` |
| Node toolchain | Node.js 24.18.0 LTS | Validated LTS patch; release line is supported through Apr 2028 and is compatible with pnpm 11 and the proposed SPA stack. | `EXTERNAL_VERIFIED` + `LOCKED` |
| Package manager | pnpm 11.4.0 | Modern monorepo support, deterministic CI behavior, and trust-policy features; the exact pnpm major must match the committed lockfile. | `EXTERNAL_VERIFIED` + `LOCKED` |
| Workers | Per-image Python/uv profile; Python 3.14.6 + uv 0.11.21 preferred | Current validated releases; every image proves its own script floor, wheels/native libraries, lock, import/startup, tests, license/SBOM, and remote build. | `EXTERNAL_VERIFIED` + `LOCKED_AFTER_COMPATIBILITY_GATE` |
| Worker alternative | Explicit scoped Python profile (for example 3.13) | Allowed only when one workload has dependency evidence blocking 3.14; never an automatic global fallback. | `COMPATIBILITY_PROFILE` |
| Development profile | Direct pinned toolchains + deterministic in-process fakes + temporary sealed fixtures | Supported hardware cannot run local deployment infrastructure. No Docker, Kubernetes, emulator, local model, package script, shell, or untrusted execution is required locally. | `LOCKED` |
| Remote image build | ACR Tasks (`az acr build`) or hosted CI | Cloud build supplies immutable lock/license/scan/SBOM/provenance/signature/digest evidence without a local Docker daemon. | `EXTERNAL_VERIFIED` + `LOCKED` |
| Execution | Azure Container Apps Jobs with fixed digest-pinned templates | Finite task model fits patch/test/build/export workers. Runtime requests may select an approved spec, but may not replace job image, command, environment, or secret bindings. | `EXTERNAL_VERIFIED` + `LOCKED` |
| Interactive execution candidate | Azure Container Apps Dynamic Sessions | Verified prewarmed isolated capability; product fit must be benchmarked. | `EXTERNAL_VERIFIED` + `SPIKE_REQUIRED` |
| Model integration | Microsoft Foundry / Azure OpenAI v1 + Responses behind Model Gateway | Use `store=false` and app-owned run/context state by default. Provider-hosted tools are off unless separately registered, governed by Airlock, and evidenced. Structured outputs use a provider schema projection plus canonical server validation. | `EXTERNAL_VERIFIED` + `LOCKED_WITH_CAPABILITY_PROFILE` |
| API description | OpenAPI 3.1.2 canonical contract | Matches ASP.NET Core on .NET 10 and avoids dual contract truth. OpenAPI 3.2 stays on the .NET 11/generator watchlist. | `EXTERNAL_VERIFIED` + `LOCKED` |
| Observability | Server OpenTelemetry + Azure Monitor; browser Application Insights JavaScript SDK | Azure supports OTel for API/workers, while the supported browser path is the Application Insights JavaScript SDK. Sampled telemetry is not audit evidence. | `EXTERNAL_VERIFIED` + `LOCKED` |
| IaC | Bicep + exact-version Azure Verified Modules where useful | Azure-native and repeatable. CI pins Bicep; AVM references use exact versions behind project wrappers with defaults/security reviewed. | `EXTERNAL_VERIFIED` + `LOCKED` |
| Azure identity and supply | Managed identities + Key Vault RBAC + ACR pull-only runtime roles | Production SQL, Blob, Key Vault, ACR, and Foundry access should avoid long-lived secrets; migration and CI publishing identities remain separate from runtime identities. | `ARCHITECTURE_DECISION` + `RELEASE_GATE` |
| Supply chain | digest-pinned images, SLSA provenance, CycloneDX SBOM | Complementary controls; none alone is sufficient. | `EXTERNAL_VERIFIED` + `LOCKED` |

## 2. Version pinning requirements

```text
global.json                    # SDK 10.0.301; rollForward limited to latestPatch in the selected feature band
.nvmrc                         # 24.18.0
package.json                   # packageManager: pnpm@11.4.0; engines.node: >=24.18.0 <25
package.json                   # direct pins: react/react-dom 19.2.7; typescript 7.0.0; proposed vite 8.1.0/react-router 8.0.1 after ADR
pnpm-lock.yaml                 # committed; generated by pnpm 11.4.0; `pnpm install --frozen-lockfile` in CI
workers/.python-version        # 3.14.6; a 3.13 worker profile requires a scoped ADR
workers/pyproject.toml         # dependencies and tool config
workers/uv.lock                # committed; uv 0.11.21; `uv lock --check` and `uv sync --locked` in CI
worker/container definitions   # base image and uv binary pinned by immutable digest; attestations verified
infra/main.bicep               # deployment source of truth; CI Bicep CLI pin recorded with release evidence
infra modules                  # AVM references use exact semantic versions, never floating tags
openapi/runtime-api.yaml       # OpenAPI 3.1.2 canonical contract
```

## 3. Not using yet

| Technology/capability | Reason it is not v1 baseline | Required before adoption |
|---|---|---|
| TypeScript 6.x as default compiler | Superseded for application compilation by TypeScript 7.0 GA on 2026-07-08. | Use only as an isolated exact compatibility alias for tooling that still needs the TS 6 programmatic API; document owner and retirement condition. |
| Node.js 26 | Current, scheduled for Active LTS on 2026-10-28. | Wait for LTS and dependency audit before replacing Node 24 LTS. |
| Next.js/server-rendered React as the default | No SEO, SSR, React Server Components, or BFF requirement is established for the workbench. | Prefer the Vite 8 + React Router 8 SPA ADR; spike a server framework only when a concrete requirement and hosting/auth impact exist. |
| Foundry Agent Service as main orchestrator | Would obscure product-owned Airlock/run-state/workspace semantics. | Spike after vertical slice; prove it can preserve all governance contracts. |
| ACA Dynamic Sessions as default executor | Product latency/cost/isolation not benchmarked. | Complete Phase-0 spike against ACA Jobs. |
| AKS | Operational complexity not justified for v1. | ADR with concrete ACA blockers. |
| Broad MCP marketplace | Tool sprawl before governance maturity. | Tool registry threat model and approval DSL. |

## 4. Modern engineering methods to apply

1. **Contract-first API:** OpenAPI 3.1.2 is the single canonical contract, with generated clients, contract tests, and no undocumented frontend calls. Track 3.2 adoption with .NET 11/tooling evidence.
2. **Ports inside modular monolith:** in-process is acceptable; bypassing internal interfaces is not.
3. **Command DSL over shell strings:** `argv[]`, canonical `cwd`, bounded env, policy hash, image digest.
4. **Manifest-based worker output:** worker writes Blob manifest/logs; Runtime API imports and advances SQL lifecycle state.
5. **Schema-first model outputs:** project canonical schemas into each provider's supported generation subset, record both hashes, then run full canonical JSON Schema and semantic validation before Airlock policy validation.
6. **Evidence by construction:** every plan/proposal/approval/job/artifact creates event references and hashes.
7. **Zero trust workspace content:** workspace files can inform context but cannot override policies or instructions.
8. **Replayable failure cases:** every major failure class must have a replay fixture before release.

## 5. Consolidated source-review stack addendum

Source: [[89 - Consolidated AI Workspace Source Review and Architecture Improvements]].

The reviewed AI workspaces confirm the stack choices but add these gates:

| Stack area | Added gate |
|---|---|
| TypeScript 7 | Treat as a conditional CLI baseline with migration evidence: generated clients, `tsc --build`, Vite/React Router, lint, declaration output, and editor/LSP config. Keep TS 6 isolated only for programmatic-API compatibility. |
| React 19.2 | Use Actions and optimistic UI for explicit operations only with the locked Vite 8 + React Router 8 SPA; do not hide runtime state transitions or approvals in client-only state. |
| ACA Jobs | Keep as v1 default for finite workers; use fixed templates and a start-only dispatcher identity, emit manifest/evidence, and never write lifecycle SQL. |
| Dynamic Sessions | Keep as spike until isolation, latency, cost, network controls, and manifest protocol are proven. |
| OpenAI/Foundry APIs | Gateway maps Responses/v1/structured-output/provider differences into Sapphirus contracts; `store=false`, app-owned state, hosted tools off, and canonical-schema validation are defaults. |
| Bicep/AVM | Pin the Bicep CLI and exact AVM versions; use wrappers, review defaults, and record policy gaps and restored artifact evidence. |
| Python 3.14 | Keep as worker target; Python 3.13 fallback needs scoped ADR and dependency-blocker evidence. |

## 6. Azure integration ordering rule

Real model and worker integrations must not precede the minimum Azure foundation. Before enabling Foundry calls or ACA dispatch outside fakes, provision through reviewed Bicep:

- environment-specific Entra app registrations, API scopes, and managed identities;
- Key Vault in Azure RBAC mode with least-privilege secret/key roles;
- ACR with separate CI push and runtime pull identities plus digest-pinned images;
- managed-identity access to SQL and Blob with separate migration identity;
- the Foundry deployment/profile and its managed-identity binding; and
- Azure Monitor/Application Insights resources, redaction, and explicit sampling policy.

The later infrastructure phase is production promotion, resilience, restore, networking, and disaster-recovery hardening rather than the first creation of these dependencies.
