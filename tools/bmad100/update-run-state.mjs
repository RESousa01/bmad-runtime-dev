import assert from "node:assert/strict";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { tmpdir } from "node:os";

export const TASK_STATES = Object.freeze([
  "pending",
  "ready",
  "in_progress",
  "verification_failed",
  "blocked_external",
  "passed",
  "superseded",
]);

const STATE_ALIASES = Object.freeze({
  active: "in_progress",
  blocked_internal: "verification_failed",
  complete: "passed",
  failed: "verification_failed",
});

const SUCCESS = "passed";
const SHA_PATTERN = /^[a-f0-9]{40,64}$/u;

function invariant(condition, message) {
  if (!condition) throw new Error(message);
}

function isDateTime(value) {
  return typeof value === "string" && !Number.isNaN(Date.parse(value));
}

function compareTasks(left, right) {
  const leftPriority = Number.parseInt(left.priority.slice(1), 10);
  const rightPriority = Number.parseInt(right.priority.slice(1), 10);
  return leftPriority - rightPriority || left.order - right.order || left.id.localeCompare(right.id);
}

function artifactTree(artifact, kind) {
  const hashes = artifact?.source_hashes ?? {};
  const names = kind === "baseline"
    ? ["baseline_task_source_tree", "baseline_tree", "baseline_git_tree"]
    : ["final_task_source_tree", "final_integration_tree", "final_tree"];
  return names.map((name) => hashes[name]).find((value) => SHA_PATTERN.test(value ?? "")) ?? null;
}

function taskFromManifest(id, entry, legacy, evidence) {
  const legacyStatus = legacy.completed?.includes(id)
    ? "passed"
    : legacy.blocked?.includes(id)
      ? "blocked_external"
      : legacy.failed?.includes(id)
        ? "verification_failed"
        : legacy.current_task === id || legacy.active_leases?.some((lease) => lease.task_id === id)
          ? "in_progress"
          : "pending";
  const baselineTree = artifactTree(evidence?.artifact, "baseline")
    ?? legacy.active_leases?.find((lease) => lease.task_id === id)?.baseline_tree
    ?? null;
  const finalTree = artifactTree(evidence?.artifact, "final");
  const commands = legacyStatus === "passed"
    ? (evidence?.artifact?.commands ?? []).map((command) => ({
      command: command.command,
      cwd: command.cwd ?? ".",
      ...(command.started_at ? { started_at: command.started_at } : {}),
      ...(command.finished_at ? { finished_at: command.finished_at } : {}),
      exit_code: command.exit_code,
      source_tree: command.source_tree ?? finalTree,
      evidence_recorded_at: command.evidence_recorded_at ?? evidence.recorded_at,
      required_for_pass: command.required_for_pass ?? command.exit_code === 0,
      ...(command.log_path ? { log_path: command.log_path } : {}),
    }))
    : [];
  return {
    id,
    phase: entry.phase,
    priority: entry.priority,
    order: entry.order,
    dependencies: [...entry.depends],
    status: legacyStatus,
    baseline_tree: baselineTree,
    final_tree: finalTree,
    changed_files: [...(evidence?.artifact?.files_changed ?? [])],
    commands,
    counts: structuredClone(evidence?.artifact?.counts ?? {}),
    evidence_hashes: structuredClone(evidence?.artifact?.evidence_hashes ?? {}),
    blocker: null,
    replacement_task_id: null,
  };
}

export function normalizeState(state, manifest, legacyEvidence = {}) {
  if (Array.isArray(state.tasks)) {
    return structuredClone(state);
  }
  invariant(manifest?.task_index, "legacy run state requires an execution manifest");
  const commit = state.repository_commit;
  invariant(SHA_PATTERN.test(commit ?? ""), "legacy repository_commit must be a Git hash");
  const tasks = Object.entries(manifest.task_index)
    .map(([id, entry]) => taskFromManifest(id, entry, state, legacyEvidence[id]))
    .sort((left, right) => left.order - right.order || left.id.localeCompare(right.id));
  const recordedTree = state.active_leases?.find((lease) => lease.task_id === state.current_task)?.baseline_tree
    ?? [...tasks].reverse().find((task) => task.final_tree)?.final_tree
    ?? commit;
  const normalized = {
    schema_version: "1.0",
    run_id: state.run_id,
    repository: {
      commit,
      branch: state.branch ?? "unknown",
      worktree_tree: state.worktree_tree ?? recordedTree,
      dirty: state.dirty ?? true,
    },
    source_hashes: structuredClone(state.source_identities ?? {}),
    tasks,
    current_task: state.current_task ?? null,
    next_ready: [],
    checkpoints: [],
    updated_at: state.updated_at ?? new Date().toISOString(),
  };
  normalized.next_ready = nextReady(normalized).map((task) => task.id);
  return normalized;
}

function validateCommand(command, task) {
  invariant(command && typeof command === "object", `${task.id}: command must be an object`);
  for (const field of ["command", "cwd", "source_tree", "evidence_recorded_at"]) {
    invariant(typeof command[field] === "string" && command[field].length > 0, `${task.id}: command.${field} is required`);
  }
  if (command.started_at !== undefined) invariant(isDateTime(command.started_at), `${task.id}: command.started_at must be a date-time`);
  if (command.finished_at !== undefined) invariant(isDateTime(command.finished_at), `${task.id}: command.finished_at must be a date-time`);
  invariant(isDateTime(command.evidence_recorded_at), `${task.id}: command.evidence_recorded_at must be a date-time`);
  invariant(Number.isInteger(command.exit_code), `${task.id}: command.exit_code must be an integer`);
  invariant(typeof command.required_for_pass === "boolean", `${task.id}: command.required_for_pass must be boolean`);
  invariant(SHA_PATTERN.test(command.source_tree), `${task.id}: command.source_tree must be a Git tree hash`);
}

function validateBlocker(blocker, task) {
  invariant(blocker && typeof blocker === "object", `${task.id}: blocker is required`);
  invariant(["external", "internal"].includes(blocker.kind), `${task.id}: blocker.kind is invalid`);
  for (const field of ["reason", "required_condition", "command", "evidence_path"]) {
    invariant(typeof blocker[field] === "string" && blocker[field].length > 0, `${task.id}: blocker.${field} is required`);
  }
}

function validateHashMap(value, label) {
  invariant(value && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  for (const [name, hash] of Object.entries(value)) {
    invariant(name.length > 0 && SHA_PATTERN.test(hash), `${label}.${name} must be a hash`);
  }
}

export function validateState(input, manifest) {
  const state = normalizeState(input, manifest);
  invariant(state.schema_version === "1.0", "run state schema_version must be 1.0");
  invariant(typeof state.run_id === "string" && state.run_id.length > 0, "run_id is required");
  invariant(state.repository && typeof state.repository === "object", "repository is required");
  invariant(SHA_PATTERN.test(state.repository.commit ?? ""), "repository.commit must be a Git hash");
  invariant(SHA_PATTERN.test(state.repository.worktree_tree ?? ""), "repository.worktree_tree must be a Git tree hash");
  invariant(typeof state.repository.branch === "string" && state.repository.branch.length > 0, "repository.branch is required");
  invariant(typeof state.repository.dirty === "boolean", "repository.dirty must be boolean");
  validateHashMap(state.source_hashes, "source_hashes");
  invariant(isDateTime(state.updated_at), "updated_at must be a date-time");
  invariant(Array.isArray(state.tasks), "tasks must be an array");

  const taskById = new Map();
  for (const task of state.tasks) {
    invariant(task && typeof task === "object", "each task must be an object");
    invariant(typeof task.id === "string" && task.id.length > 0, "task.id is required");
    invariant(!taskById.has(task.id), `duplicate task: ${task.id}`);
    invariant(TASK_STATES.includes(task.status), `${task.id}: invalid status ${task.status}`);
    invariant(Number.isInteger(task.phase) && task.phase >= 0, `${task.id}: phase must be non-negative`);
    invariant(/^P[0-9]+$/u.test(task.priority), `${task.id}: priority is invalid`);
    invariant(Number.isInteger(task.order) && task.order >= 1, `${task.id}: order is invalid`);
    invariant(Array.isArray(task.dependencies), `${task.id}: dependencies must be an array`);
    invariant(new Set(task.dependencies).size === task.dependencies.length, `${task.id}: duplicate dependency`);
    invariant(Array.isArray(task.changed_files), `${task.id}: changed_files must be an array`);
    invariant(Array.isArray(task.commands), `${task.id}: commands must be an array`);
    invariant(task.counts && typeof task.counts === "object" && !Array.isArray(task.counts), `${task.id}: counts must be an object`);
    for (const [name, count] of Object.entries(task.counts)) {
      invariant(name.length > 0 && Number.isInteger(count) && count >= 0, `${task.id}: counts.${name} must be non-negative`);
    }
    validateHashMap(task.evidence_hashes, `${task.id}.evidence_hashes`);
    task.commands.forEach((command) => validateCommand(command, task));
    if (task.status === "blocked_external") {
      validateBlocker(task.blocker, task);
      invariant(task.blocker.kind === "external", `${task.id}: blocked_external requires an external blocker`);
    }
    if (task.status === "verification_failed" && task.blocker !== null) {
      validateBlocker(task.blocker, task);
      invariant(task.blocker.kind === "internal", `${task.id}: verification_failed blockers must be internal`);
    }
    if (!["blocked_external", "verification_failed"].includes(task.status)) {
      invariant(task.blocker === null, `${task.id}: ${task.status} cannot retain a blocker`);
    }
    if (task.status === "passed") {
      invariant(task.blocker === null, `${task.id}: a passed task cannot retain a blocker`);
      invariant(SHA_PATTERN.test(task.baseline_tree ?? ""), `${task.id}: passed requires baseline_tree`);
      invariant(SHA_PATTERN.test(task.final_tree ?? ""), `${task.id}: passed requires final_tree`);
      invariant(Object.keys(task.evidence_hashes ?? {}).length > 0, `${task.id}: passed requires evidence hashes`);
      invariant(task.commands.length > 0, `${task.id}: passed requires fresh command evidence`);
      const requiredCommands = task.commands.filter((command) => command.required_for_pass);
      invariant(requiredCommands.length > 0, `${task.id}: passed requires at least one required command`);
      invariant(requiredCommands.every((command) => command.exit_code === 0), `${task.id}: passed command evidence must succeed`);
      invariant(
        requiredCommands.every((command) => command.source_tree === task.final_tree),
        `${task.id}: passed command evidence must match final_tree`,
      );
    }
    if (task.status === "superseded") {
      invariant(typeof task.replacement_task_id === "string" && task.replacement_task_id.length > 0, `${task.id}: superseded requires replacement_task_id`);
    }
    taskById.set(task.id, task);
  }

  if (manifest?.task_index) {
    const expected = Object.keys(manifest.task_index).sort();
    const actual = [...taskById.keys()].sort();
    assert.deepEqual(actual, expected, "run state must represent every manifest task exactly once");
    for (const id of expected) {
      const task = taskById.get(id);
      const entry = manifest.task_index[id];
      assert.deepEqual(task.dependencies, entry.depends, `${id}: dependencies drifted from execution manifest`);
      invariant(task.phase === entry.phase, `${id}: phase drifted from execution manifest`);
      invariant(task.priority === entry.priority, `${id}: priority drifted from execution manifest`);
      invariant(task.order === entry.order, `${id}: order drifted from execution manifest`);
    }
  }

  for (const task of state.tasks) {
    for (const dependency of task.dependencies) {
      invariant(taskById.has(dependency), `${task.id}: unknown dependency ${dependency}`);
    }
    if (["ready", "in_progress", "passed"].includes(task.status)) {
      const unmet = task.dependencies.filter((id) => taskById.get(id).status !== SUCCESS);
      invariant(unmet.length === 0, `${task.id}: unmet dependencies: ${unmet.join(", ")}`);
    }
  }

  const expectedReady = nextReady(state).map((task) => task.id);
  assert.deepEqual(state.next_ready, expectedReady, "next_ready is stale or nondeterministic");
  if (state.current_task !== null) {
    const current = taskById.get(state.current_task);
    invariant(current?.status === "in_progress", "current_task must name an in_progress task");
  }
  return state;
}

export function nextReady(state) {
  const taskById = new Map(state.tasks.map((task) => [task.id, task]));
  return state.tasks
    .filter((task) => ["pending", "ready"].includes(task.status))
    .filter((task) => task.dependencies.every((id) => taskById.get(id)?.status === SUCCESS))
    .sort(compareTasks);
}

function requirePassEvidence(task, patch, repository) {
  invariant(patch.blocker === null, `${task.id}: passed cannot retain a blocker`);
  invariant(SHA_PATTERN.test(patch.baseline_tree ?? ""), `${task.id}: passed requires baseline_tree`);
  invariant(SHA_PATTERN.test(patch.final_tree ?? ""), `${task.id}: passed requires final_tree`);
  invariant(patch.final_tree === repository.worktree_tree, `${task.id}: final_tree must match the current worktree tree`);
  invariant(Array.isArray(patch.commands) && patch.commands.length > 0, `${task.id}: passed requires fresh command evidence`);
  const requiredCommands = patch.commands.filter((command) => command.required_for_pass);
  invariant(requiredCommands.length > 0, `${task.id}: passed requires at least one required command`);
  invariant(
    requiredCommands.every((command) => command.exit_code === 0 && command.source_tree === patch.final_tree),
    `${task.id}: every passing command must succeed against final_tree`,
  );
  invariant(Object.keys(patch.evidence_hashes ?? {}).length > 0, `${task.id}: passed requires evidence hashes`);
}

export function transitionTask(input, manifest, taskId, requestedStatus, patch = {}) {
  const state = validateState(input, manifest);
  const status = STATE_ALIASES[requestedStatus] ?? requestedStatus;
  invariant(TASK_STATES.includes(status), `unknown task state: ${requestedStatus}`);
  const tasks = state.tasks.map((task) => ({ ...task }));
  const task = tasks.find((candidate) => candidate.id === taskId);
  invariant(task, `unknown task: ${taskId}`);
  const taskById = new Map(tasks.map((candidate) => [candidate.id, candidate]));
  const unmet = task.dependencies.filter((id) => taskById.get(id).status !== SUCCESS);
  if (["ready", "in_progress", "passed"].includes(status)) {
    invariant(unmet.length === 0, `${task.id}: cannot advance with unmet dependencies: ${unmet.join(", ")}`);
  }
  invariant(
    !(task.status === "blocked_external" && status === "passed"),
    `${task.id}: external blocker must be explicitly resolved before success`,
  );
  if (status === "passed") requirePassEvidence(task, { ...task, ...patch, status }, state.repository);
  Object.assign(task, patch, { status });
  if (status === "blocked_external") {
    validateBlocker(task.blocker, task);
    invariant(task.blocker.kind === "external", `${task.id}: blocked_external requires an external blocker`);
  }
  const nextState = {
    ...state,
    tasks,
    current_task: status === "in_progress"
      ? task.id
      : state.current_task === task.id
        ? null
        : state.current_task,
    updated_at: patch.updated_at ?? new Date().toISOString(),
  };
  nextState.next_ready = nextReady(nextState).map((candidate) => candidate.id);
  return validateState(nextState, manifest);
}

export function loadRunState(path, manifest) {
  const raw = JSON.parse(readFileSync(path, "utf8"));
  if (Array.isArray(raw.tasks) || !manifest?.task_index) return validateState(raw, manifest);
  const repositoryRoot = dirname(dirname(resolve(path)));
  const legacyEvidence = {};
  for (const [id, entry] of Object.entries(manifest.task_index)) {
    if (!raw.completed?.includes(id)) continue;
    const evidencePath = (entry.evidence ?? [])
      .filter((candidate) => candidate.endsWith(".json") && !candidate.includes("*"))
      .map((candidate) => resolve(repositoryRoot, candidate))
      .find((candidate) => existsSync(candidate));
    if (!evidencePath) continue;
    legacyEvidence[id] = {
      artifact: JSON.parse(readFileSync(evidencePath, "utf8")),
      recorded_at: statSync(evidencePath).mtime.toISOString(),
    };
  }
  return validateState(normalizeState(raw, manifest, legacyEvidence), manifest);
}

export function writeRunStateAtomic(path, state, manifest) {
  const validated = validateState(state, manifest);
  mkdirSync(dirname(path), { recursive: true });
  const temporary = `${path}.${process.pid}.${Date.now()}.tmp`;
  writeFileSync(temporary, `${JSON.stringify(validated, null, 2)}\n`, { encoding: "utf8", flag: "wx" });
  renameSync(temporary, path);
  return loadRunState(path, manifest);
}

export function createCheckpoint(stateInput, manifest, {
  checkpoint_id,
  kind,
  task_id = null,
  phase,
  created_at = new Date().toISOString(),
}) {
  const state = validateState(stateInput, manifest);
  invariant(["task", "phase"].includes(kind), "checkpoint kind must be task or phase");
  invariant(typeof checkpoint_id === "string" && checkpoint_id.length > 0, "checkpoint_id is required");
  invariant(Number.isInteger(phase) && phase >= 0, "checkpoint phase must be non-negative");
  if (kind === "task") {
    invariant(typeof task_id === "string" && task_id.length > 0, "task checkpoint requires task_id");
  } else {
    invariant(task_id === null, "phase checkpoint task_id must be null");
  }
  const selectedTasks = kind === "task"
    ? state.tasks.filter((task) => task.id === task_id)
    : state.tasks.filter((task) => task.phase === phase);
  invariant(selectedTasks.length > 0, "checkpoint does not select a known task or phase");
  const blockers = selectedTasks
    .filter((task) => task.blocker !== null)
    .map((task) => structuredClone(task.blocker));
  const checkpoint = {
    schema_version: "1.0",
    checkpoint_id,
    run_id: state.run_id,
    kind,
    task_id,
    phase,
    created_at,
    repository: structuredClone(state.repository),
    source_hashes: structuredClone(state.source_hashes),
    changed_files: [...new Set(selectedTasks.flatMap((task) => task.changed_files))].sort(),
    commands: selectedTasks.flatMap((task) => structuredClone(task.commands)),
    counts: Object.fromEntries(selectedTasks.flatMap((task) =>
      Object.entries(task.counts).map(([name, count]) => [`${task.id}.${name}`, count]))),
    evidence_hashes: Object.fromEntries(selectedTasks.flatMap((task) =>
      Object.entries(task.evidence_hashes).map(([name, hash]) => [`${task.id}.${name}`, hash]))),
    blockers,
    next_task: state.next_ready[0] ?? null,
  };
  return validateCheckpoint(checkpoint);
}

export function validateCheckpoint(checkpoint) {
  invariant(checkpoint?.schema_version === "1.0", "checkpoint schema_version must be 1.0");
  for (const field of ["checkpoint_id", "run_id"]) {
    invariant(typeof checkpoint[field] === "string" && checkpoint[field].length > 0, `checkpoint ${field} is required`);
  }
  invariant(["task", "phase"].includes(checkpoint.kind), "checkpoint kind is invalid");
  invariant(Number.isInteger(checkpoint.phase) && checkpoint.phase >= 0, "checkpoint phase is invalid");
  invariant(isDateTime(checkpoint.created_at), "checkpoint created_at must be a date-time");
  invariant(SHA_PATTERN.test(checkpoint.repository?.commit ?? ""), "checkpoint repository.commit is invalid");
  invariant(SHA_PATTERN.test(checkpoint.repository?.worktree_tree ?? ""), "checkpoint repository.worktree_tree is invalid");
  invariant(typeof checkpoint.repository?.branch === "string" && checkpoint.repository.branch.length > 0, "checkpoint repository.branch is required");
  invariant(typeof checkpoint.repository?.dirty === "boolean", "checkpoint repository.dirty must be boolean");
  validateHashMap(checkpoint.source_hashes, "checkpoint.source_hashes");
  invariant(Array.isArray(checkpoint.changed_files), "checkpoint changed_files must be an array");
  invariant(Array.isArray(checkpoint.commands), "checkpoint commands must be an array");
  checkpoint.commands.forEach((command) => validateCommand(command, { id: "checkpoint" }));
  invariant(checkpoint.counts && typeof checkpoint.counts === "object", "checkpoint counts must be an object");
  validateHashMap(checkpoint.evidence_hashes, "checkpoint.evidence_hashes");
  invariant(Array.isArray(checkpoint.blockers), "checkpoint blockers must be an array");
  checkpoint.blockers.forEach((blocker) => validateBlocker(blocker, { id: "checkpoint" }));
  invariant(checkpoint.next_task === null || typeof checkpoint.next_task === "string", "checkpoint next_task is invalid");
  if (checkpoint.kind === "task") {
    invariant(typeof checkpoint.task_id === "string" && checkpoint.task_id.length > 0, "task checkpoint requires task_id");
  } else {
    invariant(checkpoint.task_id === null, "phase checkpoint task_id must be null");
  }
  return checkpoint;
}

export function writeCheckpointAtomic(path, checkpoint) {
  const validated = validateCheckpoint(checkpoint);
  mkdirSync(dirname(path), { recursive: true });
  const temporary = `${path}.${process.pid}.${Date.now()}.tmp`;
  writeFileSync(temporary, `${JSON.stringify(validated, null, 2)}\n`, { encoding: "utf8", flag: "wx" });
  renameSync(temporary, path);
  return validateCheckpoint(JSON.parse(readFileSync(path, "utf8")));
}

function selfTest() {
  const hashA = "a".repeat(40);
  const hashB = "b".repeat(40);
  const now = "2026-07-28T12:00:00.000Z";
  const manifest = {
    task_index: {
      "TASK-B": { phase: 0, priority: "P1", order: 2, depends: [] },
      "TASK-A": { phase: 0, priority: "P0", order: 1, depends: [] },
      "TASK-C": { phase: 1, priority: "P0", order: 3, depends: ["TASK-A"] },
    },
  };
  const legacy = {
    schema_version: "1.0",
    run_id: "self-test",
    repository_commit: hashA,
    branch: "self-test",
    worktree_tree: hashB,
    dirty: true,
    completed: [],
    failed: [],
    blocked: [],
    current_task: null,
    source_identities: { method: "c".repeat(64) },
    updated_at: now,
  };
  let state = validateState(normalizeState(legacy, manifest), manifest);
  assert.deepEqual(state.next_ready, ["TASK-A", "TASK-B"]);
  assert.throws(() => transitionTask(state, manifest, "TASK-C", "active"), /unmet dependencies/u);
  state = transitionTask(state, manifest, "TASK-A", "ready", { updated_at: now });
  state = transitionTask(state, manifest, "TASK-A", "active", {
    baseline_tree: hashA,
    updated_at: now,
  });
  const command = {
    command: "node self-test",
    cwd: ".",
    started_at: now,
    finished_at: now,
    exit_code: 0,
    source_tree: hashB,
    evidence_recorded_at: now,
    required_for_pass: true,
  };
  state = transitionTask(state, manifest, "TASK-A", "complete", {
    baseline_tree: hashA,
    final_tree: hashB,
    changed_files: ["one"],
    commands: [command],
    counts: { tests: 1 },
    evidence_hashes: { task: "d".repeat(64) },
    blocker: null,
    updated_at: now,
  });
  assert.deepEqual(state.next_ready, ["TASK-C", "TASK-B"]);
  state = transitionTask(state, manifest, "TASK-B", "blocked_external", {
    blocker: {
      kind: "external",
      reason: "device unavailable",
      required_condition: "Windows signing device",
      command: "sign --verify",
      evidence_path: "artifacts/signing.json",
    },
    updated_at: now,
  });
  assert.throws(
    () => transitionTask(state, manifest, "TASK-B", "passed", {
      baseline_tree: hashA,
      final_tree: hashB,
      commands: [command],
      evidence_hashes: { task: "e".repeat(64) },
      blocker: null,
    }),
    /explicitly resolved/u,
  );
  const directory = mkdtempSync(join(tmpdir(), "bmad100-run-state-"));
  try {
    const path = join(directory, "run-state.json");
    const reloaded = writeRunStateAtomic(path, state, manifest);
    assert.deepEqual(reloaded, state);
    assert.deepEqual(writeRunStateAtomic(path, reloaded, manifest), state);
    const checkpoint = createCheckpoint(state, manifest, {
      checkpoint_id: "task-a",
      kind: "task",
      task_id: "TASK-A",
      phase: 0,
      created_at: now,
    });
    assert.equal(checkpoint.next_task, "TASK-C");
    assert.deepEqual(
      writeCheckpointAtomic(join(directory, "checkpoint.json"), checkpoint),
      checkpoint,
    );
    const duplicate = structuredClone(state);
    duplicate.tasks.push(structuredClone(duplicate.tasks[0]));
    assert.throws(() => validateState(duplicate, manifest), /duplicate task/u);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
  return { tasks: state.tasks.length, next_ready: state.next_ready };
}

function option(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function main() {
  try {
    if (process.argv.includes("--self-test")) {
      const result = selfTest();
      console.log(`run-state self-test passed: tasks=${result.tasks} next_ready=${result.next_ready.join(",")}`);
      return;
    }
    const statePath = resolve(option("--state") ?? ".bmad100/run-state.json");
    const manifestPath = option("--manifest");
    const manifest = manifestPath ? JSON.parse(readFileSync(resolve(manifestPath), "utf8")) : undefined;
    const state = loadRunState(statePath, manifest);
    console.log(JSON.stringify({
      run_id: state.run_id,
      current_task: state.current_task,
      next_ready: state.next_ready,
      tasks: state.tasks.length,
    }, null, 2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  main();
}
