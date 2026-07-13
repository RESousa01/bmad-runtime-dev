---
title: "Verification Register"
aliases:
  - "73 - Verification Register"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 73
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-verification-register
status: current
validated_on: 2026-07-09
---



# Verification Register

## V6.17 verification entries

| ID | Claim | Evidence required | Release effect |
|---|---|---|---|
| DUAL-01 | Project delivery model cannot change | DB/domain/property tests in C# and Rust; handoff creates linked object | blocks both |
| DUAL-02 | Specs cannot cross audiences/authorities | Golden vectors and negative execution/import tests | blocks both |
| DESK-01 | Child-process filesystem/network containment matches product claim | AppContainer/restricted-token/broker/toolchain matrix or narrowed claim/disabled commands | blocks desktop claim/release |
| DESK-V02 | App file API cannot escape selected root | NTFS device/reparse/hardlink/race property and integration suite | blocks desktop |
| DESK-V03 | Journal recovery preserves preimage or explicit quarantine | crash injection at every durable boundary | blocks desktop |
| DESK-V04 | Renderer has no generic filesystem/shell/token authority | generated IPC allowlist, Tauri capability audit, adversarial IPC tests | blocks desktop |
| DESK-V05 | Model/telemetry/sync egress matches consent | payload inspection and policy/retention tests | blocks desktop |
| DESK-V06 | Signed update/package chain and rollback work | clean-machine/ring/signature/revocation tests | blocks desktop |
| HANDOFF-01 | Remote result cannot directly apply | API/UI/Rust negative tests and fresh-candidate proof | blocks remote jobs |
| CONF-01 | C#/Rust/TS canonical hashes agree | cross-runtime golden/property vectors | blocks both |
| UX-01 | Users can distinguish delivery boundary and lifecycle state | Five-participant/proxy usability evidence for local/cloud and proposed/approved/applied/validated identification | blocks first-slice UI release |
| UX-02 | Approval review exposes human impact before execution | Keyboard and screen-reader review of files, command, network, risk, rollback, expiry, and boundary-specific decision | blocks governed UI release |
| UX-03 | Workbench remains operable across viewport and access modes | `1280×720`, `1440×900`, narrow/mobile review, 200% zoom, forced colors, keyboard-only, and reduced-motion evidence | blocks first-slice UI release |
| UX-04 | Selected diff/panel/virtualization libraries meet product needs | React 19/Vite 8/TS7, CSP, accessibility, focus, reconnect, and 50k-line performance spike | blocks dependency lock and first-slice UI release |
| UX-05 | Implemented UI matches the approved concept and tokens | Same-state/native-size concept-to-browser screenshot ledger across hierarchy, type, palette, spacing, icons, responsive behavior, and motion | blocks visual sign-off |

This register validates the highest-risk claims in the implementation library. It is intentionally focused: it does not add filler; it records which claims are source-backed, externally verified, project decisions, or still spike-dependent.

## 1. Verification classes

| Class | Meaning | Action |
|---|---|---|
| `PROJECT_SOURCE` | Directly derived from uploaded project context or critical review. | Keep unless source changes. |
| `EXTERNAL_VERIFIED` | Checked against official docs or standards. | Keep, but re-check before implementation if platform behavior matters. |
| `ARCHITECTURE_DECISION` | Intentional project decision, not an external fact. | Requires ADR for reversal. |
| `IMPLEMENTATION_SPEC` | Concrete implementation rule derived from source + decisions. | Requires tests/release gates. |
| `SPIKE_REQUIRED` | Plausible but not proven for this product. | Must be benchmarked before hardening. |
| `DEFERRED` | Not v1. | Do not build without ADR. |

## 2. Project-source validation

| Claim | Classification | Validation | Library consequence |
|---|---|---|---|
| Product is a chat-first BMAD-native Azure application, not a generic chatbot or thin wrapper over external coding tools. | `PROJECT_SOURCE` | Source context explicitly states chat-first BMAD-native Azure runtime and non-generic framing. | `00`, `01`, `10`, `11`, `12` keep chat as primary shell. |
| Product must support governed agentic coding: inspect files, propose patches, apply approved changes, run approved commands/tests, repair, rollback, export, and trace. | `PROJECT_SOURCE` | Source context final product definition and v8.4 direction require this. | `01`, `10`, `12`, `16`, `19`, `20`, `21`, `38`, `56`. |
| BMAD is canonical; Cortex is lineage only. | `PROJECT_SOURCE` | Source context non-negotiables state BMAD canonical and no Cortex runtime namespace. | `09`, `13`, `35`, `39`; no `_cortex` runtime namespace. |
| Existing presentation workflow must be adapted, not rewritten. | `PROJECT_SOURCE` | Source context v8.2 correction requires existing workflow as seed Artifact Creator workflow. | `15`, `59`; adapter workbook exists. |
| v1 must avoid microservice sprawl and use logical modules inside few deployment units. | `PROJECT_SOURCE` | Source context v8.3 says blocks are modules/features, not independent microservices. | `02`, `11`, `63`; internal ports required. |
| Original MVP was too broad and must be narrowed to one executable vertical slice first. | `PROJECT_SOURCE` | Critical review bottom-line and priority recommendations state this. | `01`, `08`, `51`, `72`; Builder/Artifact breadth after substrate. |
| Runtime API risks becoming a god control plane unless internal ports are enforced. | `PROJECT_SOURCE` | Critical review explicitly calls out internal contracts/ports. | `11`, `63`; port interfaces and module boundaries. |
| Every side-effect entry point must require an Airlock-created `ApprovedExecutionSpec`. | `PROJECT_SOURCE` + `IMPLEMENTATION_SPEC` | Critical review states every side-effect entry point should require it. | `19`, `55`; bypass tests are release gates. |
| Workers should not mutate authoritative SQL lifecycle state. | `PROJECT_SOURCE` + `IMPLEMENTATION_SPEC` | Critical review says workers should write manifests and Runtime API should advance state. | `20`, `22`, `52`, `56`; worker SQL credentials disallowed. |
| Commands must be structured specs with `argv`, `cwd`, `env`, `networkMode`, `timeout`, resource/output/path controls. | `PROJECT_SOURCE` + `IMPLEMENTATION_SPEC` | Critical review says raw command strings are inadequate and defines fields. | `38`, `55`, `56`; shell denied by default. |
| Workspace concurrency must define single-writer/multi-reader and void stale proposals after newer checkpoint. | `PROJECT_SOURCE` + `IMPLEMENTATION_SPEC` | Critical review identifies concurrency gap and fix. | `16`, `29`, `57`; stale proposal tests required. |

## 3. External platform validation

| Claim | Classification | Validation result | Source |
|---|---|---|---|
| ACA Jobs are suitable for finite containerized patch/test/build/export jobs. | `EXTERNAL_VERIFIED` | Verified. ACA Jobs run containerized tasks for finite duration and then stop. | https://learn.microsoft.com/en-us/azure/container-apps/jobs |
| ACA Jobs fit short-lived discrete background processing. | `EXTERNAL_VERIFIED` | Verified by Azure architecture guidance. | https://learn.microsoft.com/en-us/azure/architecture/best-practices/background-jobs |
| ACA environments define a secure boundary around container apps and jobs. | `EXTERNAL_VERIFIED` | Verified. | https://learn.microsoft.com/en-us/azure/container-apps/environment |
| ACA workload profiles include Consumption, Dedicated, and Flex. | `EXTERNAL_VERIFIED` | Verified. Exact profile choice remains an ADR/cost decision. | https://learn.microsoft.com/en-us/azure/container-apps/workload-profiles-overview |
| ACA Dynamic Sessions can provide prewarmed isolated session pools and are relevant for low-latency code execution. | `EXTERNAL_VERIFIED` + `SPIKE_REQUIRED` | Capability verified; product fit not proven. Keep as v1.5/Phase-0 candidate. | https://learn.microsoft.com/en-us/azure/container-apps/sessions |
| App Service supports built-in auth/authorization and Microsoft Entra provider. | `EXTERNAL_VERIFIED` | Verified. | https://learn.microsoft.com/en-us/azure/app-service/overview-authentication-authorization and https://learn.microsoft.com/en-us/azure/app-service/configure-authentication-provider-aad |
| Managed identities can be used with ACA for resource access such as ACR pull. | `EXTERNAL_VERIFIED` | Verified. | https://learn.microsoft.com/en-us/azure/container-apps/managed-identity |
| User delegation SAS is preferred for Blob delegated access where possible. | `EXTERNAL_VERIFIED` | Verified. Microsoft recommends user delegation SAS where possible. | https://learn.microsoft.com/en-us/azure/storage/blobs/storage-blob-user-delegation-sas-create-dotnet |
| Azure OpenAI JSON mode is not enough for schema guarantees. | `EXTERNAL_VERIFIED` | Verified. JSON mode guarantees valid JSON, not schema match; structured outputs are required for schema guarantees. | https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/json-mode |
| Structured outputs are the correct model-output mechanism when schema conformance is required. | `EXTERNAL_VERIFIED` | Verified. Still requires server-side validation. | https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/structured-outputs |
| OpenAPI 3.2.0 is a published API description standard. | `EXTERNAL_VERIFIED` | Verified; OAS 3.2.0 was published on 2025-09-19. V1 nevertheless uses one OpenAPI 3.1.2 contract because ASP.NET Core 10 generates 3.1; 3.2 adoption waits for .NET 11 plus complete generator/validator/client evidence. | https://spec.openapis.org/oas/v3.2.0.html |
| ASP.NET Core has built-in OpenAPI support from .NET 9 onward. | `EXTERNAL_VERIFIED` | Verified. .NET 10 defaults generated documents to OpenAPI 3.1; .NET 11 defaults to OpenAPI 3.2. | https://learn.microsoft.com/en-us/aspnet/core/fundamentals/openapi/aspnetcore-openapi |
| OpenTelemetry semantic conventions are appropriate for consistent telemetry attributes. | `EXTERNAL_VERIFIED` | Verified. | https://opentelemetry.io/docs/specs/semconv/ |
| Azure Monitor Application Insights supports OpenTelemetry collection via the Azure Monitor OpenTelemetry Distro. | `EXTERNAL_VERIFIED` | Verified. | https://learn.microsoft.com/en-us/azure/azure-monitor/app/opentelemetry-enable |
| OWASP LLM Top 10 is the right LLM threat checklist. | `EXTERNAL_VERIFIED` | Verified. | https://owasp.org/www-project-top-10-for-large-language-model-applications/ |
| Prompt injection must be treated as a first-class threat for workspace-sourced content. | `EXTERNAL_VERIFIED` | Verified by OWASP prompt injection risk description. | https://genai.owasp.org/llmrisk/llm01-prompt-injection/ |
| SLSA provenance is appropriate for artifact provenance; current docs point to v1.2. | `EXTERNAL_VERIFIED` | Verified. Avoid calling v1.0 current. | https://slsa.dev/spec/v1.0/provenance |
| CycloneDX is an OWASP full-stack Bill of Materials standard and ECMA-424. | `EXTERNAL_VERIFIED` | Verified. | https://cyclonedx.org/ and https://owasp.org/www-project-cyclonedx/ |

## 3.1 Frontend UX library validation

Checked against official project documentation on 2026-07-10. Capability does not replace the local compatibility, accessibility, security, performance, and visual gates in UX-01 through UX-05.

| Claim | Classification | Validation result | Source |
|---|---|---|---|
| React Aria Components provides unstyled composable React components and interaction/focus utilities suitable for a source-owned design system. | `EXTERNAL_VERIFIED` + `LOCKED_WITH_COMPATIBILITY_GATE` | Verified; Sapphirus still owns styling, anatomy, state vocabulary, and component wrappers. | https://react-aria.adobe.com/getting-started |
| Tailwind CSS 4 theme variables can expose design tokens as both utilities and CSS variables. | `EXTERNAL_VERIFIED` + `ARCHITECTURE_DECISION` | Verified; semantic CSS custom properties remain the product source of truth. | https://tailwindcss.com/docs/theme |
| Motion for React can honor the user's reduced-motion preference globally and replace large transforms with opacity behavior. | `EXTERNAL_VERIFIED` + `IMPLEMENTATION_SPEC` | Verified; Motion is limited to structural continuity and loaded lazily. | https://motion.dev/docs/react-accessibility |
| React Resizable Panels supports horizontal/vertical groups, persisted default layouts, resize callbacks, and minimum resize target sizes. | `EXTERNAL_VERIFIED` + `LOCKED_WITH_A11Y_GATE` | Capability verified; keyboard, focus, pointer target, and responsive-sheet integration require local tests. | https://github.com/bvaughn/react-resizable-panels |
| TanStack Virtual is a headless virtualizer and explicitly supports chat, AI streams, logs, and reverse feeds. | `EXTERNAL_VERIFIED` + `IMPLEMENTATION_SPEC` | Verified; selection, text search, focus, and reconnect scroll anchors remain product gates. | https://tanstack.com/virtual/latest/docs/introduction |
| Pierre Diffs supports React, split/stacked views, theming, annotations, and virtualization for large diff/code review surfaces. | `EXTERNAL_VERIFIED` + `SPIKE_REQUIRED` | Core capability verified. React 19, CSP/Shadow DOM, keyboard/screen-reader behavior, large-fixture performance, and theme integration are not yet proven for Sapphirus; experimental mutation/worker APIs stay out of v1. | https://diffs.com/docs |
| Lucide React supports standalone tree-shakable typed icon imports. | `EXTERNAL_VERIFIED` + `IMPLEMENTATION_SPEC` | Verified; Sapphirus locks optical size, stroke, meaning, and wrapper ownership. | https://lucide.dev/guide/react |
| Storybook's accessibility addon uses axe and can fail component tests in CI. | `EXTERNAL_VERIFIED` + `LOCKED_WITH_VITE8_TS7_GATE` | Verified; automated results are only the first line of accessibility QA. | https://storybook.js.org/docs/writing-tests/accessibility-testing |
| Playwright integrates `@axe-core/playwright`, while its docs explicitly require manual/inclusive testing for issues automation cannot detect. | `EXTERNAL_VERIFIED` + `IMPLEMENTATION_SPEC` | Verified; axe, keyboard, screen-reader smoke, focus order, zoom, forced colors, and reduced motion are combined gates. | https://playwright.dev/docs/accessibility-testing |

## 4. Claims converted from “fact” to “decision” or “spike”

| Previous wording risk | V6 correction |
|---|---|
| “Dynamic Sessions should be used for fast execution.” | Dynamic Sessions are a Phase-0/v1.5 candidate. ACA Jobs remain v1 baseline until latency spike shows otherwise. |
| “SignalR preferred; SSE acceptable.” | Streaming is a temporary decision: SignalR for scale/bidirectional complexity, SSE for simplest event stream. The chosen option needs ADR-021. |
| “Workload-profile environment is production baseline.” | Kept as baseline, but exact Consumption/Dedicated/Flex profile mix requires capacity/cost/security ADR. |
| “Structured outputs guarantee correctness.” | Corrected: they improve schema adherence, but server-side JSON Schema validation and policy validation remain mandatory. |
| “SLSA provenance/SBOM solve supply chain.” | Corrected: provenance, SBOM, signatures, digest pinning, image scanning, and release gates are complementary controls. |
| “App Service auth solves RBAC.” | Corrected: App Service/Entra handles identity entry; project/resource authorization remains Runtime API responsibility. |
| “OpenAPI-first means implementation is safe.” | Corrected: OpenAPI stabilizes contracts; contract tests and authorization tests are still required. |

## 5. V6 validation verdict

- No high-impact platform claim is left as an unsupported assertion in the active reference files.
- Platform capabilities are separated from product decisions.
- Spike-dependent claims are explicitly marked `SPIKE_REQUIRED`.
- Historical source files are preserved, not silently rewritten.
- The library still requires real implementation validation once code exists: contract tests, policy tests, worker manifest tests, IaC what-if, smoke tests, replay fixtures, and threat-model tests.
