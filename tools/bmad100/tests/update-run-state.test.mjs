import assert from "node:assert/strict";
import test from "node:test";

import {
  createCheckpoint,
  normalizeState,
  nextReady,
  transitionTask,
  validateState,
} from "../update-run-state.mjs";

const HASH_A = "a".repeat(40);
const HASH_B = "b".repeat(40);
const NOW = "2026-07-28T12:00:00.000Z";
const manifest = {
  task_index: {
    "P1-FIRST": { phase: 0, priority: "P1", order: 1, depends: [] },
    "P0-LATER": { phase: 0, priority: "P0", order: 9, depends: [] },
    CHILD: { phase: 1, priority: "P0", order: 10, depends: ["P0-LATER"] },
  },
};

function baseline() {
  return normalizeState({
    schema_version: "1.0",
    run_id: "fixture",
    repository_commit: HASH_A,
    branch: "fixture",
    worktree_tree: HASH_B,
    dirty: true,
    completed: [],
    failed: [],
    blocked: [],
    current_task: null,
    source_identities: {},
    updated_at: NOW,
  }, manifest);
}

test("represents every manifest task exactly once and sorts next-ready deterministically", () => {
  const state = validateState(baseline(), manifest);
  assert.deepEqual(nextReady(state).map((task) => task.id), ["P0-LATER", "P1-FIRST"]);
  const missing = structuredClone(state);
  missing.tasks.pop();
  assert.throws(() => validateState(missing, manifest), /exactly once/u);
});

test("forbids dependency advancement and aliases governing state vocabulary", () => {
  const state = validateState(baseline(), manifest);
  assert.throws(() => transitionTask(state, manifest, "CHILD", "active"), /unmet dependencies/u);
  const active = transitionTask(state, manifest, "P0-LATER", "active", {
    baseline_tree: HASH_A,
    updated_at: NOW,
  });
  assert.equal(active.tasks.find((task) => task.id === "P0-LATER").status, "in_progress");
});

test("external blockers cannot transition directly to success", () => {
  let state = validateState(baseline(), manifest);
  state = transitionTask(state, manifest, "P1-FIRST", "blocked_external", {
    blocker: {
      kind: "external",
      reason: "credential unavailable",
      required_condition: "managed identity",
      command: "verify-identity",
      evidence_path: "artifacts/identity.json",
    },
    updated_at: NOW,
  });
  assert.throws(
    () => transitionTask(state, manifest, "P1-FIRST", "complete", {
      baseline_tree: HASH_A,
      final_tree: HASH_B,
      commands: [{
        command: "verify",
        cwd: ".",
        started_at: NOW,
        finished_at: NOW,
        exit_code: 0,
        source_tree: HASH_B,
        evidence_recorded_at: NOW,
        required_for_pass: true,
      }],
      evidence_hashes: { task: "c".repeat(64) },
      blocker: null,
    }),
    /explicitly resolved/u,
  );
});

test("passed requires fresh successful evidence against the current source tree", () => {
  let state = validateState(baseline(), manifest);
  state = transitionTask(state, manifest, "P0-LATER", "active", {
    baseline_tree: HASH_A,
    updated_at: NOW,
  });
  assert.throws(
    () => transitionTask(state, manifest, "P0-LATER", "passed", {
      baseline_tree: HASH_A,
      final_tree: HASH_B,
      commands: [],
      evidence_hashes: {},
      blocker: null,
    }),
    /fresh command evidence/u,
  );
});

test("creates a complete durable phase checkpoint from state", () => {
  const state = validateState(baseline(), manifest);
  const checkpoint = createCheckpoint(state, manifest, {
    checkpoint_id: "phase-zero",
    kind: "phase",
    phase: 0,
    created_at: NOW,
  });
  assert.equal(checkpoint.kind, "phase");
  assert.equal(checkpoint.next_task, "P0-LATER");
  assert.deepEqual(checkpoint.repository, state.repository);
});
