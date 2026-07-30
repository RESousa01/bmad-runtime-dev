# LLM Execution Support Files

- `execution-manifest.json`: task graph, topological order, source identities, and final command.
- `run-state.schema.json`: persistent run cursor.
- `task-result.schema.json`: required per-task evidence structure.
- `context-loading-map.json`: bounded context guidance by subsystem.
- `phase-checkpoint-template.md`: durable phase summary.
- `blocker-template.md`: exact external/technical blocker record.
- `review-checklist.md`: per-task and final diff/evidence checks.

Use these files with `../10-single-turn-agent-execution-runbook.md` and `../11-master-agent-prompt.md`.
