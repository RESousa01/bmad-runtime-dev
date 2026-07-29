#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

export const REQUIRED_COUNTS = Object.freeze({
  methodPrimarySkills: 48,
  builderProductionSkills: 5,
  builderPublicActions: 9,
  builderInternalActions: 1,
  methodSourceMembers: 597,
  builderSourceMembers: 299,
});

export const PINNED_SOURCES = Object.freeze({
  method: Object.freeze({
    version: "6.10.0",
    archiveSha256:
      "2993a38a36a9586d16a62a4eacb3400db25dd1ea594bbfe4682789d7f95ac02b",
  }),
  builder: Object.freeze({
    version: "2.1.0",
    archiveSha256:
      "d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852",
  }),
});

export const GATE_DEFINITIONS = Object.freeze(
  [
    ["source_identity", "source", "QAL-01", 168, "local"],
    ["source_closure", "closure", "QAL-01", 168, "local"],
    ["contracts", "contracts", "QAL-02", 168, "local"],
    ["method_semantics", "semantics", "QAL-03", 168, "local"],
    ["builder_conformance", "builder", "QAL-04", 168, "local"],
    ["runtime", "runtime", "QAL-05", 168, "local"],
    ["ui", "ui", "QAL-06", 168, "local"],
    ["support_plane", "support_plane", "QAL-07", 24, "external_or_release"],
    ["desktop_e2e", "desktop_e2e", "QAL-08", 24, "external_or_release"],
    ["security", "security", "QAL-09", 24, "external_or_release"],
    ["performance", "performance", "QAL-10", 24, "external_or_release"],
    ["persistence", "persistence", "QAL-11", 168, "local"],
    ["packaging", "packaging", "QAL-12", 24, "external_or_release"],
    ["deployment", "deployment", "QAL-13", 24, "external_or_release"],
    ["documentation", "documentation", "QAL-14", 168, "local"],
    ["pilot", "pilot", "QAL-15", 24, "external_or_release"],
    ["traceability", "traceability", "QAL-16", 168, "local"],
    [
      "independent_review",
      "independent_review",
      "QAL-18",
      4,
      "external_or_release",
    ],
  ].map(([id, category, taskId, maxAgeHours, phase]) =>
    Object.freeze({
      id,
      category,
      remediationTaskIds: Object.freeze([taskId]),
      evidencePath: `artifacts/bmad100/quality/${taskId.toLowerCase()}/report.json`,
      maxAgeHours,
      phase,
    }),
  ),
);

const REQUIRED_CONFORMANCE = Object.freeze([
  "interactive",
  "headless",
  "error",
  "resume",
  "cancel",
]);
const FINAL_SUPPORT_STATES = new Set(["conformant", "supported", "complete"]);
const FINAL_RELEASE_STATES = new Set(["released", "release_ready", "active"]);
const REJECTED_PRIMARY_TREATMENTS = new Set([
  "fixture",
  "distribution",
  "reject_nonruntime",
]);

function blocker(gate, code, message, details = undefined) {
  return {
    gate: gate.id,
    category: gate.category,
    code,
    message,
    remediation_task_ids: [...gate.remediationTaskIds],
    evidence_path: gate.evidencePath,
    ...(details === undefined ? {} : { details }),
  };
}

function scorecardBlocker(code, message, remediationTaskIds, details = undefined) {
  return {
    gate: "scorecard",
    category: "source",
    code,
    message,
    remediation_task_ids: remediationTaskIds,
    evidence_path: null,
    ...(details === undefined ? {} : { details }),
  };
}

function expectExact(
  blockers,
  label,
  actual,
  expected,
  remediationTaskIds,
) {
  if (actual !== expected) {
    blockers.push(
      scorecardBlocker(
        "count_mismatch",
        `${label} must equal ${expected}; observed ${String(actual)}.`,
        remediationTaskIds,
        { expected, actual: actual ?? null },
      ),
    );
  }
}

function validatePinnedSources(sourceLock, blockers) {
  const sources = new Map(
    Array.isArray(sourceLock?.sources)
      ? sourceLock.sources.map((source) => [source.id, source])
      : [],
  );
  for (const [id, pinned] of Object.entries(PINNED_SOURCES)) {
    const source = sources.get(id);
    if (!source) {
      blockers.push(
        scorecardBlocker(
          "missing_source_identity",
          `Pinned ${id} source identity is missing.`,
          ["SRC-02", "QAL-01"],
        ),
      );
      continue;
    }
    const expectedFileCount =
      id === "method"
        ? REQUIRED_COUNTS.methodSourceMembers
        : REQUIRED_COUNTS.builderSourceMembers;
    const expectedSkillCount =
      id === "method"
        ? REQUIRED_COUNTS.methodPrimarySkills
        : REQUIRED_COUNTS.builderProductionSkills;
    for (const [field, actual, expected] of [
      ["version", source.version, pinned.version],
      ["archive_sha256", source.archive_sha256, pinned.archiveSha256],
      ["regular_file_count", source.regular_file_count, expectedFileCount],
      ["primary_skill_count", source.primary_skill_count, expectedSkillCount],
    ]) {
      if (actual !== expected) {
        blockers.push(
          scorecardBlocker(
            "source_identity_mismatch",
            `${id}.${field} does not match the pinned target.`,
            ["SRC-02", "QAL-01"],
            { expected, actual: actual ?? null },
          ),
        );
      }
    }
    if (id === "builder") {
      expectExact(
        blockers,
        "Builder public action identity count",
        source.public_action_count,
        REQUIRED_COUNTS.builderPublicActions,
        ["SRC-02", "QAL-01"],
      );
      expectExact(
        blockers,
        "Builder internal action identity count",
        source.internal_action_count,
        REQUIRED_COUNTS.builderInternalActions,
        ["SRC-02", "QAL-01"],
      );
    }
  }
}

function validateCounts(accounting, catalog, blockers) {
  const totals = accounting?.totals ?? {};
  expectExact(
    blockers,
    "Method source members",
    totals.source_members_by_source?.method,
    REQUIRED_COUNTS.methodSourceMembers,
    ["SRC-03", "SRC-04", "QAL-01"],
  );
  expectExact(
    blockers,
    "Builder source members",
    totals.source_members_by_source?.builder,
    REQUIRED_COUNTS.builderSourceMembers,
    ["SRC-03", "SRC-04", "QAL-01"],
  );
  expectExact(
    blockers,
    "Total source members",
    totals.source_members,
    REQUIRED_COUNTS.methodSourceMembers + REQUIRED_COUNTS.builderSourceMembers,
    ["SRC-03", "SRC-04", "QAL-01"],
  );
  expectExact(
    blockers,
    "Method primary skills in accounting",
    totals.method_primary_skills,
    REQUIRED_COUNTS.methodPrimarySkills,
    ["SRC-04", "QAL-01"],
  );
  expectExact(
    blockers,
    "Builder production skills in accounting",
    totals.builder_primary_skills,
    REQUIRED_COUNTS.builderProductionSkills,
    ["SRC-04", "QAL-01"],
  );
  expectExact(
    blockers,
    "Builder public actions in accounting",
    totals.builder_public_actions,
    REQUIRED_COUNTS.builderPublicActions,
    ["SRC-04", "QAL-04"],
  );
  expectExact(
    blockers,
    "Builder internal actions in accounting",
    totals.builder_internal_actions,
    REQUIRED_COUNTS.builderInternalActions,
    ["SRC-04", "QAL-04"],
  );

  const capabilities = Array.isArray(catalog?.capabilities)
    ? catalog.capabilities
    : [];
  const method = capabilities.filter(
    (capability) =>
      capability.source?.id === "method" &&
      capability.source?.class === "primary_method_skill",
  );
  const builder = capabilities.filter(
    (capability) =>
      capability.source?.id === "builder" &&
      capability.source?.class === "primary_builder_skill",
  );
  const builderSkills = new Set(builder.map((capability) => capability.source.path));
  const builderPublic = builder.filter(
    (capability) => capability.visibility === "public",
  );
  const builderInternal = builder.filter(
    (capability) => capability.visibility === "internal",
  );
  for (const [label, actual, expected, taskIds] of [
    [
      "Method catalog rows",
      method.length,
      REQUIRED_COUNTS.methodPrimarySkills,
      ["SRC-09", "QAL-03"],
    ],
    [
      "Builder production skill roots",
      builderSkills.size,
      REQUIRED_COUNTS.builderProductionSkills,
      ["SRC-09", "QAL-04"],
    ],
    [
      "Builder public catalog actions",
      builderPublic.length,
      REQUIRED_COUNTS.builderPublicActions,
      ["SRC-09", "QAL-04"],
    ],
    [
      "Builder internal catalog actions",
      builderInternal.length,
      REQUIRED_COUNTS.builderInternalActions,
      ["SRC-09", "QAL-04"],
    ],
    ["Catalog Method count", catalog?.counts?.method, 48, ["SRC-09", "QAL-03"]],
    [
      "Catalog Builder public count",
      catalog?.counts?.builderPublic,
      9,
      ["SRC-09", "QAL-04"],
    ],
    [
      "Catalog Builder internal count",
      catalog?.counts?.builderInternal,
      1,
      ["SRC-09", "QAL-04"],
    ],
    ["Catalog total count", catalog?.counts?.total, 58, ["SRC-09"]],
  ]) {
    expectExact(blockers, label, actual, expected, taskIds);
  }
}

function validateAccounting(accounting, blockers) {
  const primaryRows = (accounting?.members ?? []).filter((member) =>
    ["primary_method_skill", "primary_builder_skill"].includes(member.class),
  );
  const badRows = primaryRows
    .filter(
      (member) =>
        member.runtime_denominator !== true ||
        REJECTED_PRIMARY_TREATMENTS.has(member.treatment) ||
        !FINAL_SUPPORT_STATES.has(member.support_state) ||
        !FINAL_RELEASE_STATES.has(member.release_state) ||
        typeof member.target_capability !== "string" ||
        member.target_capability.length === 0,
    )
    .map((member) => ({
      member: member.member_key ?? member.path ?? "unknown",
      treatment: member.treatment ?? null,
      support_state: member.support_state ?? null,
      release_state: member.release_state ?? null,
      target_capability: member.target_capability ?? null,
    }));
  const badActions = (accounting?.builder_actions ?? [])
    .filter(
      (action) =>
        !FINAL_SUPPORT_STATES.has(action.support_state) ||
        !FINAL_RELEASE_STATES.has(action.release_state),
    )
    .map((action) => ({
      action: action.id ?? action.action ?? "unknown",
      support_state: action.support_state ?? null,
      release_state: action.release_state ?? null,
    }));
  if (badRows.length > 0 || badActions.length > 0) {
    blockers.push(
      scorecardBlocker(
        "source_not_conformant",
        `${badRows.length} primary source rows and ${badActions.length} Builder actions are not in a final conformant state.`,
        ["SRC-04", "QAL-01", "QAL-03", "QAL-04"],
        { primary_rows: badRows, builder_actions: badActions },
      ),
    );
  }
}

function validateCatalogAndAliases(catalog, aliasCatalog, blockers) {
  const capabilities = Array.isArray(catalog?.capabilities)
    ? catalog.capabilities
    : [];
  const canonicalIds = new Set(
    capabilities.map((capability) => capability.canonicalId),
  );
  const advertisedAliases = new Map();
  const nonConformantCapabilities = [];
  for (const capability of capabilities) {
    if (
      !FINAL_SUPPORT_STATES.has(capability.supportState) ||
      !Array.isArray(capability.modes) ||
      !capability.modes.includes("interactive") ||
      !capability.modes.includes("headless")
    ) {
      nonConformantCapabilities.push({
        capability: capability.canonicalId ?? "unknown",
        source: capability.source?.id ?? null,
        support_state: capability.supportState ?? null,
        modes: capability.modes ?? null,
      });
    }
    for (const alias of capability.aliases ?? []) {
      if (
        advertisedAliases.has(alias) &&
        advertisedAliases.get(alias) !== capability.canonicalId
      ) {
        blockers.push(
          scorecardBlocker(
            "duplicate_alias",
            `Alias ${alias} is advertised with conflicting canonical targets.`,
            ["SRC-09"],
            {
              first_target: advertisedAliases.get(alias),
              conflicting_target: capability.canonicalId,
            },
          ),
        );
      }
      advertisedAliases.set(alias, capability.canonicalId);
    }
  }
  if (nonConformantCapabilities.length > 0) {
    blockers.push(
      scorecardBlocker(
        "capability_not_conformant",
        `${nonConformantCapabilities.length} catalog rows are not conformant in interactive and headless modes.`,
        ["SRC-09", "QAL-03", "QAL-04"],
        { capabilities: nonConformantCapabilities },
      ),
    );
  }
  const rows = Array.isArray(aliasCatalog?.aliases) ? aliasCatalog.aliases : [];
  const resolved = new Map();
  for (const row of rows) {
    const valid =
      typeof row.alias === "string" &&
      typeof row.target === "string" &&
      row.migrationPolicy === "resolve_before_dispatch" &&
      canonicalIds.has(row.target) &&
      !resolved.has(row.alias);
    if (!valid) {
      blockers.push(
        scorecardBlocker(
          "invalid_alias",
          `Alias ${row.alias ?? "unknown"} does not resolve exactly once to a canonical catalog row.`,
          ["SRC-09"],
          { target: row.target ?? null },
        ),
      );
    } else {
      resolved.set(row.alias, row.target);
    }
  }
  const missingOrDifferent = [...advertisedAliases]
    .filter(([alias, target]) => resolved.get(alias) !== target)
    .map(([alias, target]) => ({ alias, target }));
  const unadvertised = [...resolved]
    .filter(([alias, target]) => advertisedAliases.get(alias) !== target)
    .map(([alias, target]) => ({ alias, target }));
  if (missingOrDifferent.length > 0 || unadvertised.length > 0) {
    blockers.push(
      scorecardBlocker(
        "alias_set_mismatch",
        "The compatibility alias catalog and canonical capability catalog do not contain the same alias mappings.",
        ["SRC-09"],
        {
          missing_or_different: missingOrDifferent,
          unadvertised,
        },
      ),
    );
  }
}

function validateGate(gate, evidence, inputs, blockers) {
  if (!evidence || typeof evidence !== "object") {
    blockers.push(
      blocker(
        gate,
        "missing_evidence",
        `Required ${gate.id} evidence is missing.`,
      ),
    );
    return;
  }
  const identifiesGate =
    evidence.gate_id === gate.id ||
    (Array.isArray(evidence.gate_ids) && evidence.gate_ids.includes(gate.id));
  if (
    evidence.schema_version !== "sapphirus.bmad.gate-evidence/v1" ||
    !identifiesGate
  ) {
    blockers.push(
      blocker(
        gate,
        "invalid_evidence_contract",
        `${gate.id} evidence does not use the required schema and gate identity.`,
      ),
    );
  }
  for (const [field, code] of [
    ["skipped", "skipped_evidence"],
    ["unavailable", "unavailable_evidence"],
    ["partial", "partial_evidence"],
    ["incomparable", "incomparable_evidence"],
  ]) {
    if (evidence[field] === true) {
      blockers.push(
        blocker(
          gate,
          code,
          `${gate.id} evidence is marked ${field}; mandatory gates cannot use it.`,
        ),
      );
    }
  }
  if (!["pass", "passed"].includes(evidence.status) || evidence.complete === false) {
    blockers.push(
      blocker(
        gate,
        "gate_not_passed",
        `${gate.id} evidence does not record a pass.`,
        { status: evidence.status ?? null },
      ),
    );
  }
  const generatedAt = Date.parse(
    evidence.generated_at ?? evidence.generatedAt ?? evidence.completed_at ?? "",
  );
  const now = Date.parse(inputs.now);
  const ageHours = (now - generatedAt) / 3_600_000;
  if (
    !Number.isFinite(generatedAt) ||
    !Number.isFinite(now) ||
    ageHours < -5 / 60 ||
    ageHours > gate.maxAgeHours
  ) {
    blockers.push(
      blocker(
        gate,
        "stale_evidence",
        `${gate.id} evidence is missing a valid current timestamp or exceeds its ${gate.maxAgeHours}-hour freshness window.`,
        {
          generated_at:
            evidence.generated_at ??
            evidence.generatedAt ??
            evidence.completed_at ??
            null,
          max_age_hours: gate.maxAgeHours,
        },
      ),
    );
  }
  const commit =
    evidence.commit ?? evidence.repository?.commit ?? evidence.git?.commit;
  const sourceTree =
    evidence.source_tree ??
    evidence.sourceTree ??
    evidence.repository?.worktree_tree ??
    evidence.git?.tree;
  if (commit !== inputs.expectedCommit || sourceTree !== inputs.expectedTree) {
    blockers.push(
      blocker(
        gate,
        "identity_mismatch",
        `${gate.id} evidence is not bound to the current commit and source tree.`,
        {
          expected_commit: inputs.expectedCommit,
          actual_commit: commit ?? null,
          expected_tree: inputs.expectedTree,
          actual_tree: sourceTree ?? null,
        },
      ),
    );
  }
  const commands = Array.isArray(evidence.commands) ? evidence.commands : [];
  if (
    commands.length === 0 ||
    commands.some(
      (command) =>
        command.required_for_pass !== true ||
        command.exit_code !== 0 ||
        command.skipped === true,
    )
  ) {
    blockers.push(
      blocker(
        gate,
        "invalid_command_evidence",
        `${gate.id} must include at least one freshly successful required command and no skipped required command.`,
      ),
    );
  }
  const evidenceHashes = Object.values(evidence.evidence_hashes ?? {});
  if (
    evidenceHashes.length === 0 ||
    evidenceHashes.some(
      (hash) => typeof hash !== "string" || !/^[a-f0-9]{64}$/.test(hash),
    )
  ) {
    blockers.push(
      blocker(
        gate,
        "invalid_evidence_hashes",
        `${gate.id} must contain at least one valid lowercase SHA-256 evidence hash.`,
      ),
    );
  }
  if (
    gate.phase === "external_or_release" &&
    (evidence.environment?.executed !== true ||
      !evidence.environment?.kind ||
      ["mock", "synthetic", "fixture"].includes(evidence.environment.kind))
  ) {
    blockers.push(
      blocker(
        gate,
        "unqualified_environment",
        `${gate.id} requires execution in a real qualified environment.`,
      ),
    );
  }
  if (gate.id === "independent_review") {
    const approvals = new Map(
      (evidence.approvals ?? []).map((approval) => [
        approval.role,
        approval.approved === true &&
          typeof approval.approver === "string" &&
          approval.approver.length > 0,
      ]),
    );
    const missingApprovals = [
      "product",
      "runtime",
      "security",
      "qa",
      "operations",
    ].filter((role) => approvals.get(role) !== true);
    if (missingApprovals.length > 0) {
      blockers.push(
        blocker(
          gate,
          "missing_approval",
          "Independent review lacks required named-role approval.",
          { missing_roles: missingApprovals },
        ),
      );
    }
  }
  if (["method_semantics", "builder_conformance"].includes(gate.id)) {
    const missing = REQUIRED_CONFORMANCE.filter(
      (dimension) => evidence.conformance?.[dimension] !== true,
    );
    if (missing.length > 0) {
      blockers.push(
        blocker(
          gate,
          "missing_conformance",
          `${gate.id} is missing required mode/state conformance.`,
          { missing },
        ),
      );
    }
  }
}

export function evaluateBmad100(inputs) {
  const blockers = [];
  validatePinnedSources(inputs.sourceLock, blockers);
  validateCounts(inputs.accounting, inputs.catalog, blockers);
  validateAccounting(inputs.accounting, blockers);
  validateCatalogAndAliases(inputs.catalog, inputs.aliases, blockers);
  for (const gate of GATE_DEFINITIONS) {
    validateGate(gate, inputs.evidence?.[gate.id], inputs, blockers);
  }
  const complete = blockers.length === 0;
  return {
    schema_version: "sapphirus.bmad.binary-scorecard-evaluation/v1",
    complete,
    completion_percent: complete ? 100 : 0,
    evaluated_at: inputs.now,
    source_identity: {
      commit: inputs.expectedCommit,
      source_tree: inputs.expectedTree,
      method: PINNED_SOURCES.method,
      builder: PINNED_SOURCES.builder,
    },
    exact_counts: REQUIRED_COUNTS,
    gates: GATE_DEFINITIONS.map((gate) => ({
      id: gate.id,
      category: gate.category,
      passed: !blockers.some((entry) => entry.gate === gate.id),
      evidence_path: gate.evidencePath,
      max_age_hours: gate.maxAgeHours,
      remediation_task_ids: [...gate.remediationTaskIds],
    })),
    blockers,
  };
}

export function exitCodeForResult(result) {
  return result.complete === true && result.completion_percent === 100 ? 0 : 1;
}

function readJson(root, relativePath, required = true) {
  try {
    return JSON.parse(readFileSync(path.join(root, relativePath), "utf8"));
  } catch (error) {
    if (!required && error?.code === "ENOENT") {
      return undefined;
    }
    throw new Error(`Cannot read ${relativePath}: ${error.message}`, {
      cause: error,
    });
  }
}

function currentCommit(root) {
  return execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: root,
    encoding: "utf8",
  }).trim();
}

function loadCliInputs(root, now) {
  const runState = readJson(root, ".bmad100/run-state.json", false);
  const evidence = Object.fromEntries(
    GATE_DEFINITIONS.map((gate) => [
      gate.id,
      readJson(root, gate.evidencePath, false),
    ]),
  );
  return {
    now,
    expectedCommit: currentCommit(root),
    expectedTree:
      runState?.repository?.worktree_tree ??
      runState?.repository?.tree ??
      "unrecorded-worktree",
    sourceLock: readJson(root, "packages/bmad-foundation/source-lock.json"),
    accounting: readJson(
      root,
      "packages/bmad-foundation/source-accounting.json",
    ),
    catalog: readJson(
      root,
      "packages/bmad-foundation/capability-catalog.json",
    ),
    aliases: readJson(
      root,
      "packages/bmad-foundation/compatibility-aliases.json",
    ),
    evidence,
  };
}

function explain(result) {
  console.log(
    `BMAD 100: ${result.complete ? "COMPLETE" : "INCOMPLETE"} (${result.completion_percent}%)`,
  );
  console.log(
    `Pinned Method ${PINNED_SOURCES.method.version} ${PINNED_SOURCES.method.archiveSha256}`,
  );
  console.log(
    `Pinned Builder ${PINNED_SOURCES.builder.version} ${PINNED_SOURCES.builder.archiveSha256}`,
  );
  if (result.blockers.length === 0) {
    console.log("All mandatory gates passed with current exact evidence.");
    return;
  }
  for (const entry of result.blockers) {
    console.log(
      `- [${entry.gate}:${entry.code}] ${entry.message} Remediation: ${entry.remediation_task_ids.join(", ")}.`,
    );
  }
}

function parseCli(argv) {
  const flags = new Set(argv);
  const nowIndex = argv.indexOf("--now");
  return {
    explain: flags.has("--explain"),
    json: flags.has("--json"),
    now:
      nowIndex >= 0 && typeof argv[nowIndex + 1] === "string"
        ? argv[nowIndex + 1]
        : new Date().toISOString(),
  };
}

async function main() {
  const options = parseCli(process.argv.slice(2));
  const root = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "..",
  );
  const result = evaluateBmad100(loadCliInputs(root, options.now));
  if (options.json || !options.explain) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    explain(result);
  }
  process.exitCode = exitCodeForResult(result);
}

const invokedPath = process.argv[1]
  ? path.resolve(process.argv[1])
  : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  await main();
}
