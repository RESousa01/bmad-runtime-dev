# BMAD 100 execution-state protocol

The authoritative resume record is `.bmad100/run-state.json`. It is durable
execution state, not a progress estimate. A restarted agent loads and validates
that file before selecting work. Task evidence remains under
`artifacts/bmad100/tasks/`; it supports the state record but does not replace it.

## State vocabulary

The pinned SRC-11 contract uses `pending`, `ready`, `in_progress`,
`verification_failed`, `blocked_external`, `passed`, and `superseded`.
Campaign wording maps `active` to `in_progress`, `failed` and
`blocked_internal` to `verification_failed`, and `complete` to `passed`. These
are input aliases only; persisted state always uses the pinned names.

`superseded` is not success. It must name a replacement task and never satisfies
a dependency. An external blocker is also never success. It must first be
resolved by an explicit transition back to `ready`, after which normal
execution and fresh verification can occur.

## Durable record

Every run record contains:

- the run identity and repository commit, branch, worktree tree, and dirty flag;
- pinned source hashes;
- every execution-manifest task exactly once, with phase, priority, order, and
  exact dependencies;
- status, baseline and final tree hashes, changed files, command lines,
working directories, timestamps, exit codes, counts, and evidence hashes;
- a structured blocker when applicable;
- the current task, deterministically ordered next-ready list, checkpoint
  references, and update time.

The task list is checked against the execution manifest when loading legacy
state or validating a campaign. Unknown, duplicate, missing, or dependency-
drifted tasks fail validation. Legacy aggregate arrays are upgraded in memory;
the tool does not invent or maintain a parallel state file.

## Selection and advancement

Ready tasks have all dependencies in `passed`. Ordering is:

1. lowest numeric priority (`P0` before `P1`);
2. earliest manifest order;
3. lexical task ID.

A task cannot enter `ready`, `in_progress`, or `passed` with an unmet
dependency. A task can enter `passed` only when its blocker is empty, its
baseline and final trees are recorded, its final tree matches the current
worktree tree, every required command freshly exited zero against that tree,
and at least one evidence hash is present.
Initial red commands remain in the record with `required_for_pass: false`;
every command marked `required_for_pass: true` must be fresh, successful, and
bound to the final tree.

## Checkpoints and restart

After every completed or blocked task, atomically rewrite the run state and
write a task checkpoint conforming to
`artifacts/bmad100/checkpoint.schema.json`. At a phase gate, write a phase
checkpoint with the same repository identity and evidence hashes. Store the
checkpoint path in the run record only after both files are durable.

On restart:

1. load the execution manifest and `.bmad100/run-state.json`;
2. validate exact task coverage, dependencies, state invariants, and
   deterministic `next_ready`;
3. confirm the repository and pinned source hashes;
4. reload the current task or select the first `next_ready` task;
5. never infer success from chat history or source presence.

Use `node tools/bmad100/update-run-state.mjs --self-test` to exercise restart,
atomic reload, dependency, ordering, exact-coverage, and external-blocker
guards without changing the live campaign state. The CLI also accepts
`--state <path> --manifest <path>` to validate and summarize an existing state.
The module exports atomic run-state and checkpoint writers for the coordinator;
both validate their complete payload before rename and immediately reload the
durable file after rename.
