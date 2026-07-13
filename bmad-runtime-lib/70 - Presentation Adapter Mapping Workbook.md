---
title: "Presentation Adapter Mapping Workbook"
aliases:
  - "70 - Presentation Adapter Mapping Workbook"
tags:
  - bmad-runtime
  - vault/implementation-assets
section: "Implementation Assets"
order: 70
vault_role: "readable-note"
project: Sapphirus BMAD Runtime
type: presentation-adapter-workbook
status: implementation-guide
---



# Presentation Adapter Mapping Workbook

## V6.17 execution mapping

Keep one semantic mapping from BMAD artifact inputs/outputs to presentation operations, then declare a delivery adapter per operation. Web operations use a cloud snapshot and approved remote artifact worker. Desktop operations use app-owned temp/output storage or an approved selected-folder write with checkpoint; optional remote rendering uploads only the selected immutable inputs.

Each row records delivery applicability, workspace target, executor audience, egress/upload requirement, output authority, rollback class, and evidence fields. A remote artifact is downloaded/imported; it cannot write through to the local folder.

## 1. Inventory checklist

For the existing presentation workflow, capture:

- workflow name and entry command;
- required inputs;
- optional inputs;
- source ingestion rules;
- outline generation prompt/stage;
- user review checkpoints;
- slide drafting logic;
- templates/themes/assets;
- image/table/chart handling;
- export command;
- error handling;
- final artifact naming;
- known limitations.

## 2. Mapping table

| Existing workflow concept | BMAD adapter concept | Notes |
|---|---|---|
| Entry prompt | `SKILL.md` invocation | Preserve user-facing behavior. |
| Stages | workflow steps | Add trace events per stage. |
| Templates | module assets | Version and hash. |
| User review | approval checkpoint | Source/outline/draft/export approvals. |
| Output PPTX | artifact export | Hash and provenance required. |
| Existing tests/examples | golden fixtures | Compare output or normalized structure. |

## 3. Golden comparison

- Normalize timestamps, IDs, and nondeterministic metadata.
- Compare outline structure.
- Compare slide count and section order.
- Compare required speaker notes/content where deterministic.
- Compare export existence and file hash only when deterministic.
- Document intentional adapter deviations.

## 4. Adapter package shape

```text
packages/presentation-workflow-adapter/
  module.yaml
  module-help.csv
  SKILL.md
  workflows/presentation.workflow.yaml
  templates/
  scripts/
  fixtures/
    golden-basic/
    golden-source-heavy/
  Start Here.md
```
