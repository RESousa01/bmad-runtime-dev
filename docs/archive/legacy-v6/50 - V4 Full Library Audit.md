---
title: "V4 Full Library Audit"
aliases:
  - "50 - V4 Full Library Audit"
tags:
  - bmad-runtime
  - vault/audit-and-validation
section: "Audit and Validation"
order: 50
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: v4-review-audit
status: reviewed
---



# V4 Full Library Audit

> Historical audit baseline. Preserve its findings as evidence, but do not treat its single cloud-first delivery description as current product authority. The V6.17 split is defined in [[93 - Split Web and Windows Desktop Architecture Plans]] through [[99 - Dual-Delivery Contract and Conformance Specification]].

## 1. Direct finding

The v3 library was large enough, but line count alone was the wrong success metric. The real issue was whether each file was an implementation guide. The v4 review found five weaknesses:

1. **Template repetition:** many component files repeated the same “implementation-depth contract” without enough component-specific execution detail.
2. **Boundary enforcement needed more mechanical detail:** the Airlock boundary, worker/SQL separation, and Orchestrator/Model Gateway split needed to appear in every affected component.
3. **API/event/table/blob traceability was too scattered:** a developer could still ask “which file owns this route/table/event?”
4. **Release gates were not connected to epics:** acceptance criteria existed, but not always as a phase-by-phase quality bar.
5. **Failure semantics needed a single operational playbook:** partial success, stale proposals, replays, repairs, and worker failures needed one reference.

## 2. V4 corrections

| Problem | V4 correction |
|---|---|
| Component files felt like plans | Added per-component build cards, APIs, domain events, SQL ownership, Blob layout, build steps, edge cases, and release gates. |
| Source context risked being summarized away | Preserved full source context and critical review files. |
| Review corrections could drift | Added boundary tests, Airlock bypass tests, worker SQL credential rule, command DSL rules, and release gates. |
| v1 scope could expand again | Re-centered the library on the executable vertical slice before BMAD breadth. |
| Integration ownership unclear | Added route/table/event/Blob ownership references and integration maps. |

## 3. Hard rules after v4

- Do not measure completeness by line count alone.
- Do not remove preserved source context or critical review.
- Do not let any implementation guide omit failure modes and tests.
- Do not allow a worker image to mutate authoritative SQL state.
- Do not allow side-effect APIs to accept raw proposals.
- Do not allow the BMAD Kernel to route general agentic coding tasks.
- Do not allow the Model Gateway to become the Proposal owner.
- Do not allow Builder Studio breadth to block the first executable slice.

## 4. Review checklist for future passes

1. Does each component file identify its package/path?
2. Does it list API/port touchpoints?
3. Does it list domain events?
4. Does it identify SQL ownership and Blob layout?
5. Does it include implementation steps, not just goals?
6. Does it include edge cases and acceptance tests?
7. Does it honor the corrected build order?
8. Does it maintain source-alignment with BMAD contracts?
9. Does it keep policy, model, and execution boundaries separate?
10. Does it make failure and partial success explicit?
