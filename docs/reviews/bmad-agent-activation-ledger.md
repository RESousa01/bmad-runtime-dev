# Sapphirus BMAD agent activation ledger

Generated 2026-07-21 from the sealed foundation (package promotion: `blocked_provenance`, operational authority: `none`).

Legend: **active** = sealed managed projection exists, capability runs through the governed lifecycle once the support plane is connected. **prompt reference — unavailable** = the upstream source prompt was not adopted; the menu entry is descriptive only until its member is re-treated and minted.

## 📊 Mary — Business Analyst (`bmad-agent-analyst`)

Channels Porter's strategic rigor and Minto's Pyramid Principle, grounds every finding in verifiable evidence, represents every stakeholder voice. Speaks like a treasure hunter narrating the find: thrilled by every clue, precise once the pattern emerges.

| Menu | Capability | Kind | Status | Detail |
|---|---|---|---|---|
| BP · Brainstorm Project | `bmm:bmad-brainstorming` | skill | **active** | document_artifact · model_with_reviewed_context |
| MR · Market Research | `bmm:bmad-market-research` | skill | **active** | document_artifact · model_with_reviewed_context |
| DR · Domain Research | `bmm:bmad-domain-research` | skill | **active** | document_artifact · model_with_reviewed_context |
| TR · Technical Research | `bmm:bmad-technical-research` | skill | **active** | document_artifact · model_with_reviewed_context |
| CB · Create Brief | `bmm:bmad-product-brief` | skill | **active** | document_artifact · model_with_reviewed_context |
| WB · PRFAQ Challenge | `bmm:bmad-prfaq` | skill | **active** | document_artifact · model_with_reviewed_context |
| DP · Document Project | `bmm:bmad-document-project` | skill | **active** | document_artifact · model_with_reviewed_context |

## 🏗️ Winston — System Architect (`bmad-agent-architect`)

Favors boring technology for stability, developer productivity as architecture, ties every decision to business value. Speaks like a seasoned engineer at the whiteboard: measured, always laying out trade-offs rather than verdicts.

| Menu | Capability | Kind | Status | Detail |
|---|---|---|---|---|
| CA · Architecture | `bmm:bmad-architecture` | skill | **active** | document_artifact · model_with_reviewed_context |
| IR · Check Implementation Readiness | `bmm:bmad-check-implementation-readiness` | skill | **active** | document_artifact · model_with_reviewed_context |

## 💻 Amelia — Senior Software Engineer (`bmad-agent-dev`)

Test-first discipline (red, green, refactor), 100% pass before review, no fluff all precision. Speaks like a terminal prompt: exact file paths, AC IDs, and commit-message brevity — every statement citable.

| Menu | Capability | Kind | Status | Detail |
|---|---|---|---|---|
| DS · Dev Story | `bmm:bmad-dev-story` | skill | **active** | governed_change_set · model_with_reviewed_context |
| QD · Quick Dev | `bmm:bmad-quick-dev` | skill | **active** | governed_change_set · model_with_reviewed_context |
| QA · QA Automation Test | `bmm:bmad-qa-generate-e2e-tests` | skill | **active** | governed_change_set · model_with_reviewed_context |
| CR · Code Review | `bmm:bmad-code-review` | skill | **active** | document_artifact · model_with_reviewed_context |
| SP · Sprint Planning | `bmm:bmad-sprint-planning` | skill | **active** | document_artifact · model_with_reviewed_context |
| CS · Create Story | `bmm:bmad-create-story` | skill | **active** | document_artifact · model_with_reviewed_context |
| ER · Retrospective | `bmm:bmad-retrospective` | skill | **active** | document_artifact · model_with_reviewed_context |

## 📋 John — Product Manager (`bmad-agent-pm`)

Drives Jobs-to-be-Done over template filling, user value first, technical feasibility is a constraint not the driver. Speaks like a detective interrogating a cold case: short questions, sharper follow-ups, every 'why?' tightening the net.

| Menu | Capability | Kind | Status | Detail |
|---|---|---|---|---|
| PRD · Create Edit and Review PRD | `bmm:bmad-prd` | skill | **active** | document_artifact · model_with_reviewed_context |
| CE · Create Epics and Stories | `bmm:bmad-create-epics-and-stories` | skill | **active** | document_artifact · model_with_reviewed_context |
| IR · Check Implementation Readiness | `bmm:bmad-check-implementation-readiness` | skill | **active** | document_artifact · model_with_reviewed_context |
| CC · Correct Course | `bmm:bmad-correct-course` | skill | **active** | document_artifact · model_with_reviewed_context |

## 📚 Paige — Technical Writer (`bmad-agent-tech-writer`)

Master of CommonMark, DITA, and OpenAPI; turns complex concepts into accessible structured docs, favors diagrams over walls of text, every word earning its place. Speaks like the patient teacher you wish you'd had, using analogies that make complex things feel simple.

| Menu | Capability | Kind | Status | Detail |
|---|---|---|---|---|
| DP · Document Project | `bmm:bmad-document-project` | skill | **active** | document_artifact · model_with_reviewed_context |
| WD · Write Document | `method-010` | prompt reference | **unavailable** (unavailable_reference_only) | decisions: adopt+adapt |
| MG · Generate Mermaid | `method-011` | prompt reference | **unavailable** (unavailable_reference_only) | decisions: adopt+adapt |
| VD · Validate Document | `method-012` | prompt reference | **unavailable** (unavailable_reference_only) | decisions: adopt+adapt |
| EC · Explain Concept | `method-013` | prompt reference | **unavailable** (unavailable_reference_only) | decisions: adopt+adapt |

## 🎨 Sally — UX Designer (`bmad-agent-ux-designer`)

Balances empathy with edge-case rigor, starts simple and evolves through feedback, every decision serves a genuine user need. Speaks like a filmmaker pitching the scene before the code exists, painting user stories that make you feel the problem.

| Menu | Capability | Kind | Status | Detail |
|---|---|---|---|---|
| CU · Create UX | `bmm:bmad-ux` | skill | **active** | document_artifact · model_with_reviewed_context |

## Totals

- Skill-target menu entries with **active** sealed capability: 22
- Skill-target entries not active: 0
- Prompt-reference entries (source prompt not adopted): 4
- Capability closure records: 29 (decided by: docs/adr/ADR-0005-full-bmad-capability-denominator.md)

## What gates actual execution (all agents)

1. **Support plane connection** — the D2 API code is complete (running locally in Docker); production composition requires the Azure deployment (Task 10, blocked on org approvals: Entra app registration, SQL admin group, tags, quotas) and desktop cloud enablement (stage 9, human-gated).
2. **Package promotion** — `promotionEligibility: blocked_provenance` pins the whole package until the provenance decision is made.
3. **Prompt-reference members** — each needs a re-treatment decision (adopt/adapt) and a remint of its sealed projection before its menu entry can become a runnable skill.
