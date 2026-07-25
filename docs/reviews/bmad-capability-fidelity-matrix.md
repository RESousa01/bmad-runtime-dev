# BMAD capability fidelity matrix

**Date:** 2026-07-24 (updated 2026-07-25: Architecture is now an ordered state
machine — see [P9](../implementation-packets/P9-architecture-state-machine-2026-07-25.md))
**Basis:** `packages/bmad-foundation/capability-closure-ledger.json` (29 capabilities),
the runtime generic capability lane
(`crates/desktop-app/src/bmad_model/capability_coordinator.rs`), and the
2026-07-23 comprehensive audit
([`docs/reviews/forge-v2-comprehensive-audit-2026-07-23.md`](forge-v2-comprehensive-audit-2026-07-23.md)).
**Runtime source of the labels:** `apps/desktop-ui/src/lib/bmadFidelity.ts`
(rendered as a badge on every runnable Method menu in `BmadLibraryPanel`).

## Why this matrix exists

Every runnable Method capability executes through **one generic lane**: a single
reviewed, one-shot model request that returns one document or change set. That is
a legitimate Forge adaptation, but it is **not** the upstream BMAD workflow —
several of those are ordered, scripted, or multi-turn/collaborative. Presenting a
runnable capability as if it were the full upstream workflow would overclaim. This
matrix marks each capability's honest status; the UI shows the same label inline.

## Fidelity levels

| Level | Meaning |
|---|---|
| **Adapted (one-shot)** | Active. Runs as a single reviewed request. Deliberately **not** asserted as complete upstream fidelity. |
| **Stepwise (ordered)** | Active. Runs as an ordered progression the host validates — no step may be skipped, repeated, or finalized early — with a stable decision log distilled into one inert artifact. Deliberately **not** asserted as the upstream collaborative dialogue. |
| **Inert draft** | Builder output only. Produces an inert draft; cannot install, register, or execute anything. |

## Method capabilities (24)

| Capability | Menu | Fidelity | Note |
|---|---|---|---|
| `bmm:bmad-brainstorming` | BP | Adapted (one-shot) | One reviewed request; not the upstream facilitation loop. |
| `bmm:bmad-market-research` | MR | Adapted (one-shot) | Records external facts as **open questions** — no live retrieval. |
| `bmm:bmad-domain-research` | DR | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-technical-research` | TR | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-product-brief` | CB | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-prfaq` | WB | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-document-project` | DP | Adapted (one-shot) | One reviewed request. |
| `bmm:tech-writer-write-document` | WD | Adapted (one-shot) | One reviewed request. |
| `bmm:tech-writer-mermaid-gen` | MG | Adapted (one-shot) | One reviewed request. |
| `bmm:tech-writer-validate-doc` | VD | Adapted (one-shot) | One reviewed request. |
| `bmm:tech-writer-explain-concept` | EC | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-prd` | PRD | Adapted (one-shot) | One reviewed request; not the upstream iterative PRD build. |
| `bmm:bmad-create-epics-and-stories` | CE | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-check-implementation-readiness` | IR | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-correct-course` | CC | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-ux` | CU | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-architecture` | CA | **Stepwise (ordered)** | Eight host-selected steps (system context, components, trust boundaries, data, failure handling, observability, deployment, verification), stable `AD-n` decision log, locally distilled spine. Not the upstream coaching dialogue, memlog, or reviewer-gate subagents. |
| `bmm:bmad-dev-story` | DS | Adapted (one-shot) | Schema-first one-shot change proposal, **not** the upstream ordered dev-story workflow. |
| `bmm:bmad-quick-dev` | QD | Adapted (one-shot) | Schema-first one-shot change proposal, **not** the upstream scripted quick-dev workflow. |
| `bmm:bmad-qa-generate-e2e-tests` | QA | Adapted (one-shot) | One reviewed change-set request. |
| `bmm:bmad-code-review` | CR | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-sprint-planning` | SP | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-create-story` | CS | Adapted (one-shot) | One reviewed request. |
| `bmm:bmad-retrospective` | ER | Adapted (one-shot) | One reviewed request. |

## Builder capabilities (5) — all inert drafts

`builder:agent.analyze`, `builder:agent.create_rebuild`, `builder:agent.edit`,
`builder:workflow.analyze`, `builder:workflow.build_edit`. Each produces an inert
draft only; none can install, register, or execute an agent or workflow.

## Summary

- Adapted (one-shot): **23** Method capabilities.
- Stepwise (ordered): **1** — Architecture (CA).
- Inert draft: **5** Builder capabilities.
- Unavailable: **0**. Every reviewed menu path is now runnable.
- **Faithful upstream execution: still 0.** The ordered lane is a real,
  host-validated state machine, but it is eight reviewed one-shot steps — not the
  upstream coaching dialogue with its memlog, reviewer-gate subagents, and web
  verification. Availability is not fidelity, and this matrix does not upgrade
  the claim.
- Durable multi-step lanes: the Method Help lifecycle (durable, a separate
  mechanism) and the architecture progression (an in-memory session over durably
  stored per-step evidence; an interrupted progression restarts at step one).

## Help reconciliation

**Resolved:** the sealed Help binding recommends **Create Architecture**, and that
recommendation is now actionable — the ordered state machine exists, is verified
end to end, and the menu path is enabled. The contradiction between sealed Help
data and runtime availability is closed without editing any sealed byte.

**Still open:** the Help catalog remains narrower than the enabled Method
denominator (two installed skills and two actions against 24 unique
capabilities), so Help cannot yet sequence or explain the full enabled set. The
Help binding is sealed foundation data (hash-verified), so widening it is tracked
as part of the production model-backed Help milestone rather than changed ad
hoc.
