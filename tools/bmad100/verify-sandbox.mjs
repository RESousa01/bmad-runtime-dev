#!/usr/bin/env node
// Static verification for the BMAD script sandbox (SEC-02).
//
// This does not execute the sandbox itself — `cargo test -p desktop-runtime
// --test bmad_script_sandbox` is the runtime proof. This script verifies the
// two things a passing test run cannot, on its own, guarantee: that every
// declared script profile actually declares every required isolation
// control (so a future profile can't quietly drop one), and that every
// required negative case named in the task has a real, named test behind it
// (so a required break can't quietly go untested while the suite still
// passes green).

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const repositoryRoot = resolve(dirname(scriptPath), "..", "..");

const profileModulePath = resolve(
  repositoryRoot,
  "crates/desktop-runtime/src/execution/script_profiles/mod.rs",
);
const sandboxTestPath = resolve(
  repositoryRoot,
  "crates/desktop-runtime/tests/bmad_script_sandbox.rs",
);

// The controls acceptance criterion 1 (process, filesystem, environment,
// network, time, output, resource isolation) requires every profile to
// declare. Each entry is a field name that must appear in the
// `ScriptProfile` struct definition.
const REQUIRED_PROFILE_CONTROLS = [
  ["script_hash", "content-bound script identity"],
  ["interpreter_hash", "content-bound interpreter identity"],
  ["read_only_mounts", "filesystem isolation (read)"],
  ["read_write_mounts", "filesystem isolation (write)"],
  ["environment_allowlist", "environment isolation"],
  ["network_policy", "network isolation"],
  ["command_schema", "process isolation (command shape)"],
  ["quotas", "resource isolation (cpu/memory/process/output/time)"],
  ["expected_outputs", "output isolation"],
  ["cleanup_policy", "cleanup / restart-safety"],
];

// Every negative case the task requires, mapped to a substring that must
// appear in a `#[test] fn bmad_script_sandbox_*` name in the sandbox test
// file. Fails loudly, naming the missing case, rather than silently passing
// on partial coverage.
const REQUIRED_NEGATIVE_CASES = [
  ["path_traversal", "dot_dot_traversal"],
  ["absolute_path_as_authority", "absolute_path_as_declared_input"],
  ["symlink_or_junction_escape", "symlink_escape"],
  ["alternate_data_stream", "alternate_data_stream"],
  ["fork_bomb_or_process_tree_explosion", "process_tree_explosion"],
  ["output_flood", "output_flood"],
  ["cpu_exhaustion", "cpu_exhaustion"],
  ["memory_exhaustion", "memory_exhaustion"],
  ["child_process_persists_after_kill", "process_kill_as_unavailable"],
  ["network_access", "network_access_when_policy_is_denied"],
  ["cloud_metadata_endpoint", "cloud_metadata_endpoint"],
  ["environment_secret_leakage", "environment_secret_even_when_allowlisted"],
  ["tampered_script_bytes", "tampered_script_bytes"],
];

function readText(path, label) {
  try {
    return readFileSync(path, "utf8");
  } catch (error) {
    fail(`could not read ${label} at ${path}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function fail(message) {
  throw new Error(message);
}

function extractStructFields(source, structName) {
  const structStart = source.indexOf(`struct ${structName} {`);
  if (structStart === -1) fail(`could not find \`struct ${structName}\` in the profile module`);
  const bodyStart = source.indexOf("{", structStart);
  let depth = 0;
  let index = bodyStart;
  for (; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) break;
    }
  }
  const body = source.slice(bodyStart + 1, index);
  return body;
}

function extractTestNames(source) {
  const names = [];
  const pattern = /fn\s+(bmad_script_sandbox_[a-z0-9_]+)\s*\(/g;
  let match = pattern.exec(source);
  while (match !== null) {
    names.push(match[1]);
    match = pattern.exec(source);
  }
  return names;
}

function verifyAllProfiles(profileSource) {
  const fields = extractStructFields(profileSource, "ScriptProfile");
  const missing = REQUIRED_PROFILE_CONTROLS.filter(([field]) => !fields.includes(`pub ${field}:`)).map(
    ([field, purpose]) => `${field} (${purpose})`,
  );
  if (missing.length > 0) {
    fail(
      `ScriptProfile is missing required isolation controls: ${missing.join(", ")}. ` +
        `Every declared profile must carry all of: ${REQUIRED_PROFILE_CONTROLS.map(([field]) => field).join(", ")}.`,
    );
  }
  return REQUIRED_PROFILE_CONTROLS.length;
}

function verifyNegativeCoverage(testSource) {
  const testNames = extractTestNames(testSource);
  const missing = REQUIRED_NEGATIVE_CASES.filter(
    ([, needle]) => !testNames.some((name) => name.includes(needle)),
  ).map(([caseName]) => caseName);
  if (missing.length > 0) {
    fail(
      `the sandbox test suite is missing a named negative test for: ${missing.join(", ")}. ` +
        "Every required negative case must have a bmad_script_sandbox_* test asserting a specific SandboxDenial variant.",
    );
  }
  const positiveComplements = testNames.filter((name) => name.includes("accepts_")).length;
  if (positiveComplements === 0) {
    fail(
      "no positive-complement test found (a case proving the same check would succeed without " +
        "the sandbox denial) — negative tests alone cannot prove they are discriminating.",
    );
  }
  return { testCount: testNames.length, positiveComplements };
}

function parseArgs(argv) {
  const flags = new Set(argv);
  return {
    allProfiles: flags.has("--all-profiles"),
    negative: flags.has("--negative"),
  };
}

export function run(argv = process.argv.slice(2)) {
  try {
    const { allProfiles, negative } = parseArgs(argv);
    if (!allProfiles && !negative) {
      fail("usage: verify-sandbox.mjs --all-profiles --negative");
    }

    const report = { schema_version: "sapphirus.bmad.script-sandbox-verify/v1" };

    if (allProfiles) {
      const profileSource = readText(profileModulePath, "script profile module");
      report.profile_controls_checked = verifyAllProfiles(profileSource);
    }

    if (negative) {
      const testSource = readText(sandboxTestPath, "sandbox negative test file");
      const { testCount, positiveComplements } = verifyNegativeCoverage(testSource);
      report.negative_cases_required = REQUIRED_NEGATIVE_CASES.length;
      report.tests_found = testCount;
      report.positive_complements_found = positiveComplements;
    }

    process.stdout.write(`${JSON.stringify(report)}\n`);
    return 0;
  } catch (error) {
    process.stderr.write(`verify-sandbox: ${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(scriptPath)) {
  process.exitCode = run();
}
