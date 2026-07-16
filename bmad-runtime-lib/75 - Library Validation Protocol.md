---
title: "Library Validation Protocol"
aliases:
  - "75 - Library Validation Protocol"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 75
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: library-validation-protocol
status: active
---



# Library Validation Protocol

## Living knowledge validation

The living authority layer is validated offline. Run these commands from the repository root before accepting a knowledge change:

```powershell
py -3 -B -m unittest discover -s bmad-runtime-lib/_source_review/tests -p "test_*.py"
pnpm vault:test
py -3 -B bmad-runtime-lib/_source_review/validate_library.py
node tools/verify-reference-vault.mjs
```

The validator checks closed claim/source registries, evidence depth, note-catalog coverage, current-note claim references, repository pins, root legacy-status hygiene, and both manifests. External facts must be researched from official primary sources and corroborated where practical; repository facts require implementation evidence plus tests or checks.

The sections below describe the retained legacy-library checks. They remain useful, but the `knowledge-base/current` notes now own current product authority.

## V6.17 architecture-consistency checks

Validation scans every active note for an applicability statement or an unambiguous shared/web/desktop context. It fails on: cloud services presented as desktop local authority; generic Tauri filesystem/shell bridges; `local` used for `sealed_test_fake`; provider keys on device; Job Objects called a sandbox; multi-file apply called atomic; remote result direct apply; last-write-wins on protected facts; missing delivery/authority discriminator on durable contracts; or broken links to 93–99.

It also validates JSON/Markdown, manifest counts/hashes, balanced code/Mermaid fences, unique ADR IDs, canonical field casing, no authority-changing project mutation, and agreement between Start Here, Vault Map, stack baseline, decision summary, roadmap, release gates, and contract docs.

Use this protocol before accepting future changes to the Markdown implementation library.

## 1. Validation rule

A library change is acceptable only if it passes all four checks:

1. **Source check:** does not contradict the preserved project context or critical review unless an ADR explicitly supersedes them.
2. **External check:** platform/security/standard claims are tied to current official docs or downgraded to spike/ADR language.
3. **Implementation check:** component guidance includes owner, contract, state/data responsibility, failure modes, tests, and release gate.
4. **Consistency check:** route/table/event/blob ownership does not conflict across files.

## 2. Per-file checklist

For every active component file, verify:

- purpose and non-goals are stated;
- deployment/module boundary is explicit;
- owned APIs or ports are listed;
- consumed/emitted events are listed;
- SQL tables are owned or referenced through ports only;
- Blob paths are listed for bulky payloads;
- state transitions are explicit where applicable;
- failure states are explicit;
- security controls are explicit;
- observability attributes/events are explicit;
- implementation steps are concrete;
- tests and release gates are concrete;
- spike-dependent claims are marked as such.

## 3. Cross-file consistency checklist

Before publishing a new version, run these checks manually or with a script:

| Check | Expected result |
|---|---|
| Duplicate route ownership | No route has two owning components unless explicitly mediated by a port. |
| Duplicate SQL writer ownership | No table has multiple direct writers unless explicitly modeled. |
| Worker SQL lifecycle mutation | Forbidden. Worker writes Blob manifest; API imports. |
| Raw command strings | Forbidden for execution specs; only display examples may use shell-like text. |
| Governed mutation without exact candidate policy and `ApprovedExecutionSpec` | Forbidden. Ordinary authenticated CRUD and offline Source Intake use their documented authority classes. |
| Model Gateway proposal ownership | Forbidden. Gateway returns typed model output only. |
| BMAD Kernel general routing | Forbidden. Orchestrator owns general routing. |
| Dynamic Sessions v1 baseline | Forbidden unless ADR-020 passes. |
| SignalR/SSE permanent lock | Forbidden until ADR-021. |
| Raw trace payload retention by default | Forbidden in production. |
| Lifecycle/evidence/outbox split transaction | Forbidden where one domain transition requires all three; telemetry is not a substitute. |
| Source/root-license assumption | Forbidden. Every copied/derived component needs immutable provenance where available plus a path-level license/notice decision. |
| `sealed_test_fake` described as sandbox | Forbidden. It has no shell/network/package/dependency surface and cannot prove containment. |
| Local container/model prerequisite | Forbidden for the supported workflow; local tests use direct toolchains/fakes and builds run in ACR Tasks/hosted CI. |
| Dynamic ACA job override | Forbidden. Image, entrypoint, identity, secrets, network profile, and arbitrary environment are fixed by reviewed IaC. |
| Unevaluated model profile/fallback | Forbidden. Exact capabilities, schema projection/canonical validation, four-lane eval, canary, rollback, and critical thresholds are required. |

## 4. Suggested automated audit commands

These commands are examples for the future repository; adapt paths as needed.

```bash
# Size overview: line counts per note, smallest first
find docs/implementation-library -name '*.md' -print0 | xargs -0 wc -l | sort -n

# Stale version markers: find V3/V4 references outside the archive notes
rg -n 'V3|v3|V4|v4' docs/implementation-library --glob '!05 - Preserved Source Context.md' --glob '!06 - Preserved Critical Review.md' --glob '!50 - V4 Full Library Audit.md'

# Raw-shell leakage: side-effect notes must not describe shell-string execution
rg -n 'sh -c|shell string|raw command|pnpm test|npm test' docs/implementation-library/19 - Airlock Policy and Approvals.md docs/implementation-library/20 - Execution Lanes and Container App Jobs.md docs/implementation-library/38 - Worker Images and Command DSL.md

# Hedge words: implementation guides must not contain unresolved language
rg -n 'maybe|possibly|probably|TBD|TODO|unresolved' docs/implementation-library --glob '!05 - Preserved Source Context.md' --glob '!06 - Preserved Critical Review.md'
```

## 5. Evidence required for future changes

| Change type | Required evidence |
|---|---|
| Azure platform claim | Official Microsoft Learn source, date checked, and impact note. |
| OpenAPI/JSON Schema claim | Official spec or tooling documentation. |
| OTel claim | OpenTelemetry spec/docs or Azure Monitor docs. |
| LLM security claim | OWASP/NIST/vendor source plus local threat-model consequence. |
| Supply-chain claim | SLSA/CycloneDX/SPDX/vendor source plus release-gate consequence. |
| Execution model change | Policy tests, worker-manifest tests, failure-mode tests. |
| Data ownership change | Route/table/blob ownership update and migration impact. |
| Scope change | ADR and roadmap update. |
| Source snapshot or derived content | Origin/ref/archive hash, safe extraction, component license/notice decision, copied/derived map, and revalidation trigger. |
| Model/profile/fallback change | Exact deployment capabilities, credential/retention policy, schema projection, frozen eval evidence, canary, and rollback. |
| Worker image/build change | Remote immutable build, lock/component-license/scan/SBOM/provenance/signature/digest, fixed-template and no-local-Docker smoke. |

## 6. Publication gate

Do not publish a future V6+ library unless:

- `Library Quality Report.md` is regenerated;
- `manifest.json` is regenerated with hashes;
- `Start Here.md` identifies the current version;
- `73 - Verification Register.md` or its successor is updated;
- stale version labels are either removed or intentionally historical;
- source context and critical review remain preserved;
- at least one human-readable changelog explains what changed and why.
