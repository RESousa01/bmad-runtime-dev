---
title: "Corrections Applied"
aliases:
  - "79 - Corrections Applied"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 79
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v6-corrections-applied
status: legacy-reference
validated_on: 2026-07-09
---



# Corrections Applied

## 2026-07-10 — Dual-delivery migration

The library now treats `web_managed` and `windows_local` as separate internally consistent architectures. Files 93–99 own the split, native host/IPC, local workspace/execution, local state/evidence/recovery, desktop security, Azure support plane, and cross-runtime contracts. Active implementation notes were scoped or extended; historical source/audit notes were preserved with supersession banners.

Key correctness fixes: immutable project discriminator; `sealed_test_fake` terminology; no Azure authority over ordinary local edits; no generic renderer filesystem/shell authority; no provider key on device; no multi-file atomicity claim; no Job-Object sandbox claim; replica-only optional sync; and no direct apply from remote results.

## 1. Corrections applied to the library

| Correction | Files affected | Result |
|---|---|---|
| Added explicit runtime/toolchain versions. | `02`, `36`, `60`, `76`, `77`, Start Here | The library now says what to build on, not only what class of tool to use. |
| Separated stable baselines from preview/spike capabilities. | `02`, `60`, `76`, `78` | TypeScript 7 is now promoted to baseline after GA; Dynamic Sessions, Foundry Agent Service, and Node 26 still require explicit adoption gates. |
| Updated AI platform language from generic Azure OpenAI to Microsoft Foundry/OpenAI v1 + Responses API where applicable. | `18`, `60`, `76`, `77`, `78` | Model Gateway remains provider abstraction; no direct provider-object coupling. |
| Added version pinning requirements. | `36`, `76`, `77` | Dev and CI can enforce the same stack. |
| Added deprecation/preview watchlist. | `78` | The plan has a mechanism to stay current. |
| Reclassified Aspire as DevEx candidate, not production IaC source of truth. | `36`, `76` | Prevents dual deployment-authority drift. |

## 2. Validation philosophy

This pass does not claim that every historical sentence in preserved source files is externally verified. Preserved source files are evidence. Active implementation files must classify claims and cite or mark them appropriately.

V6 standard:

- external platform fact → source and date;
- project decision → ADR or locked decision table;
- implementation rule → test/release gate;
- preview/RC feature → spike, not baseline;
- historical/source text → preserved but not automatically current.
