# Task Execution Prompt Template

Execute `<TASK-ID>` from `12-implementation-backlog.json`.

## Inputs

- Dependency evidence:
- Pinned source paths/hashes:
- Existing implementation:
- Generated contracts/catalog:
- Files leased:
- Security invariants:

## Required process

1. Inspect only bounded context.
2. Establish failing/negative evidence.
3. Implement the complete task contract.
4. Run every verification command freshly.
5. Evaluate every acceptance criterion.
6. Review the relevant diff.
7. Write `artifacts/bmad100/tasks/<TASK-ID>.json`.
8. Return files, commands/results, acceptance/evidence, and blockers.

Do not modify run state, push, deploy, sign, publish, disable tests, weaken assertions, add hidden fallback, or claim completion.
