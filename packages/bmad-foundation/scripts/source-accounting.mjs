#!/usr/bin/env node

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const manifestPath = join(packageRoot, "source-manifest.json");
const accountingPath =
  process.env.BMAD_SOURCE_ACCOUNTING_PATH ?? join(packageRoot, "source-accounting.json");

const TREATMENTS = ["adopt", "adapt", "shim", "fixture", "distribution", "reject_nonruntime"];
const SEMANTIC_TEST_SET = [
  "activation",
  "resource_closure",
  "interactive_headless",
  "state_resume",
  "authority_negative",
  "output_conformance",
];

// Phase 2 rows move here only after their executable adapter, canonical
// projection, Help/UI topology, and required evidence have all qualified.
// Keeping the state in the generator prevents a later accounting refresh from
// silently reopening a completed row.
const QUALIFIED_METHOD_ROWS = new Map([
  [
    "src/bmm-skills/2-plan-workflows/bmad-prd/SKILL.md",
    {
      supportState: "exact",
      uiState: "runtime_available",
      releaseState: "qualified",
    },
  ],
]);

const METHOD_TASK_NAMES = [
  "bmad-agent-analyst",
  "bmad-agent-tech-writer",
  "bmad-document-project",
  "bmad-prfaq",
  "bmad-product-brief",
  "bmad-agent-pm",
  "bmad-agent-ux-designer",
  "bmad-prd",
  "bmad-spec",
  "bmad-ux",
  "bmad-agent-architect",
  "bmad-architecture",
  "bmad-check-implementation-readiness",
  "bmad-create-epics-and-stories",
  "bmad-generate-project-context",
  "bmad-agent-dev",
  "bmad-checkpoint-preview",
  "bmad-code-review",
  "bmad-correct-course",
  "bmad-create-story",
  "bmad-dev-auto",
  "bmad-dev-story",
  "bmad-qa-generate-e2e-tests",
  "bmad-quick-dev",
  "bmad-retrospective",
  "bmad-sprint-planning",
  "bmad-sprint-status",
  "bmad-create-architecture",
  "bmad-create-prd",
  "bmad-domain-research",
  "bmad-edit-prd",
  "bmad-market-research",
  "bmad-technical-research",
  "bmad-validate-prd",
  "bmad-advanced-elicitation",
  "bmad-brainstorming",
  "bmad-customize",
  "bmad-deep-recon",
  "bmad-forge-idea",
  "bmad-help",
  "bmad-party-mode",
  "bmad-review",
  "bmad-editorial-review",
  "bmad-editorial-review-prose",
  "bmad-editorial-review-structure",
  "bmad-review-adversarial-general",
  "bmad-review-edge-case-hunter",
  "bmad-review-verification-gap",
];

const METHOD_TASKS = new Map(
  METHOD_TASK_NAMES.map((name, index) => [name, `MTH-${String(index + 1).padStart(2, "0")}`]),
);

const BUILDER_TASKS = new Map([
  ["bmad-agent-builder", "BLD-06"],
  ["bmad-bmb-setup", "BLD-04"],
  ["bmad-eval-runner", "BLD-25"],
  ["bmad-module-builder", "BLD-20"],
  ["bmad-workflow-builder", "BLD-14"],
]);

const SHIM_TARGETS = new Map([
  ["bmad-create-architecture", "bmm:bmad-architecture"],
  ["bmad-create-prd", "bmm:bmad-prd"],
  ["bmad-domain-research", "core:bmad-deep-recon"],
  ["bmad-edit-prd", "bmm:bmad-prd"],
  ["bmad-market-research", "core:bmad-deep-recon"],
  ["bmad-technical-research", "core:bmad-deep-recon"],
  ["bmad-validate-prd", "bmm:bmad-prd"],
  ["bmad-editorial-review", "core:bmad-review"],
  ["bmad-editorial-review-prose", "core:bmad-review"],
  ["bmad-editorial-review-structure", "core:bmad-review"],
  ["bmad-review-adversarial-general", "core:bmad-review"],
  ["bmad-review-edge-case-hunter", "core:bmad-review"],
  ["bmad-review-verification-gap", "core:bmad-review"],
]);

const BUILDER_ACTIONS = [
  ["public", "bmad-bmb-setup", "configure", "SB", ["BLD-04", "BLD-05"]],
  [
    "public",
    "bmad-agent-builder",
    "build-process",
    "BA",
    ["BLD-06", "BLD-07", "BLD-08", "BLD-09", "BLD-10"],
  ],
  ["public", "bmad-agent-builder", "quality-analysis", "AA", ["BLD-11", "BLD-12", "BLD-13"]],
  [
    "public",
    "bmad-workflow-builder",
    "build-process",
    "BW",
    ["BLD-14", "BLD-15", "BLD-16", "BLD-19"],
  ],
  ["public", "bmad-workflow-builder", "quality-analysis", "AW", ["BLD-17", "BLD-19"]],
  ["public", "bmad-workflow-builder", "convert-process", "CW", ["BLD-18"]],
  ["public", "bmad-module-builder", "ideate-module", "IM", ["BLD-20"]],
  ["public", "bmad-module-builder", "create-module", "CM", ["BLD-21", "BLD-22", "BLD-24"]],
  ["public", "bmad-module-builder", "validate-module", "VM", ["BLD-23"]],
  [
    "internal",
    "bmad-eval-runner",
    "run-evals",
    null,
    ["BLD-25", "BLD-26", "BLD-27", "BLD-28", "BLD-29", "BLD-30", "BLD-31"],
  ],
].map(([visibility, skill, action, menuCode, implementationTasks]) => ({
  id: `builder:${skill}:${action}`,
  source_skill: skill,
  visibility,
  action,
  menu_code: menuCode,
  treatment: "adapt",
  owner: "Forge Builder Runtime",
  rationale:
    "Project the pinned Builder action through typed plans, Airlock policy, deterministic execution, validation, and durable evidence.",
  implementation_tasks: implementationTasks,
  semantic_test_set: SEMANTIC_TEST_SET,
  support_state: "implementation_pending",
  ui_state: visibility === "public" ? "planned" : "internal",
  release_state: "blocked_pending_conformance",
}));

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function countBy(values, key) {
  return Object.fromEntries(
    [...new Set(values.map((value) => value[key]))]
      .sort()
      .map((name) => [name, values.filter((value) => value[key] === name).length]),
  );
}

function skillName(path) {
  return path.split("/").at(-2);
}

function capabilityId(sourceId, path, name) {
  if (sourceId === "builder") return `builder:${name}`;
  return path.startsWith("src/core-skills/") ? `core:${name}` : `bmm:${name}`;
}

function retainedNonruntime(member) {
  if (
    member.class === "upstream_test_fixture" ||
    member.class === "builder_sample_fixture" ||
    member.class === "nested_builder_template" ||
    member.path.startsWith("test/") ||
    member.path.startsWith("samples/")
  ) {
    return {
      treatment: "fixture",
      owner: "BMAD Conformance",
      retained_purpose:
        "Retained as pinned upstream test or sample input for parser, compatibility, upgrade, and negative conformance verification; never registered as a production capability.",
      verification_use: "source_import_and_conformance_fixture",
    };
  }

  if (
    member.class === "web_distribution_bundle" ||
    /^(?:web-bundles|website|docs|\.augment|\.claude-plugin|\.github)\//u.test(member.path)
  ) {
    return {
      treatment: "distribution",
      owner: "BMAD Source Provenance",
      retained_purpose:
        "Retained as pinned upstream documentation or distribution material for provenance, packaging comparison, and upgrade analysis; excluded from runtime capability counts.",
      verification_use: "distribution_and_upgrade_comparison",
    };
  }

  return {
    treatment: "reject_nonruntime",
    owner: "BMAD Source Provenance",
    retained_purpose:
      "Retained byte-for-byte for archive provenance, license review, deterministic re-import, and upgrade comparison; explicitly excluded from runtime authority and capability counts.",
    verification_use: "provenance_and_upgrade_comparison",
  };
}

async function generateAccounting() {
  const manifestBytes = await readFile(manifestPath);
  const manifest = JSON.parse(manifestBytes);
  const versions = Object.fromEntries(manifest.sources.map((source) => [source.id, source.version]));
  const primaryMembers = manifest.members.filter((member) =>
    ["primary_method_skill", "primary_builder_skill"].includes(member.class),
  );

  const primaryRows = [];
  for (const member of primaryMembers) {
    const name = skillName(member.path);
    const taskId = (member.source_id === "method" ? METHOD_TASKS : BUILDER_TASKS).get(name);
    assert.ok(taskId, `no implementation task is assigned to ${member.source_id}:${member.path}`);

    const sourcePath = join(
      packageRoot,
      "source",
      member.source_id,
      versions[member.source_id],
      ...member.path.split("/"),
    );
    const text = await readFile(sourcePath, "utf8");
    const declaredName = /^name:\s*['"]?([^'"\r\n]+)['"]?\s*$/mu.exec(text)?.[1];
    assert.equal(declaredName, name, `${member.path}: frontmatter name drifted`);

    const isShim = member.path.includes("/v6-shims/");
    const canonical = capabilityId(member.source_id, member.path, name);
    const qualifiedState = member.source_id === "method"
      ? QUALIFIED_METHOD_ROWS.get(member.path)
      : undefined;
    primaryRows.push({
      member_key: `${member.source_id}:${member.path}`,
      source_id: member.source_id,
      version: versions[member.source_id],
      path: member.path,
      sha256: member.sha256,
      class: member.class,
      entry_type: member.entry_type,
      treatment: isShim ? "shim" : "adapt",
      owner: member.source_id === "builder" ? "Forge Builder Runtime" : "Forge BMAD Runtime",
      retained_purpose:
        "Pinned production skill input retained for a native governed projection; source instructions remain untrusted data and grant no runtime authority.",
      runtime_denominator: true,
      skill_name: name,
      implementation_task: taskId,
      rationale: isShim
        ? "Preserve the pinned compatibility invocation while routing to one canonical governed implementation."
        : "Adapt the pinned semantics to the native typed runtime without transferring source-text authority.",
      target_capability: isShim ? SHIM_TARGETS.get(name) : canonical,
      source_alias: isShim ? canonical : null,
      target_package: `sapphirus.bmad.skill.${member.source_id}.${name}.v1`,
      semantic_test_set: SEMANTIC_TEST_SET,
      support_state: qualifiedState?.supportState ?? "implementation_pending",
      ui_state: qualifiedState?.uiState ?? "planned",
      release_state: qualifiedState?.releaseState ?? "blocked_pending_conformance",
    });
  }

  const primaryRoots = primaryRows
    .map((row) => ({ row, root: `${dirname(row.path).replaceAll("\\", "/")}/` }))
    .sort((left, right) => right.root.length - left.root.length);
  const primaryByKey = new Map(primaryRows.map((row) => [row.member_key, row]));

  const members = manifest.members.map((member) => {
    const memberKey = `${member.source_id}:${member.path}`;
    const primary = primaryByKey.get(memberKey);
    if (primary) return primary;

    const owned = primaryRoots.find(
      ({ row, root }) => row.source_id === member.source_id && member.path.startsWith(root),
    )?.row;
    if (owned && member.class !== "nested_builder_template") {
      return {
        member_key: memberKey,
        source_id: member.source_id,
        version: versions[member.source_id],
        path: member.path,
        sha256: member.sha256,
        class: member.class,
        entry_type: member.entry_type,
        treatment: owned.treatment,
        owner: owned.owner,
        retained_purpose: `Required retained resource in the recursively owned source subtree for ${owned.skill_name}; execution remains subject to the target capability's typed authority boundary.`,
        runtime_denominator: false,
        owning_skill: owned.skill_name,
        target_capability: owned.target_capability,
        verification_use: "recursive_skill_resource_closure",
      };
    }

    if (
      (member.source_id === "method" && member.path.startsWith("src/")) ||
      (member.source_id === "builder" && member.path.startsWith("skills/"))
    ) {
      return {
        member_key: memberKey,
        source_id: member.source_id,
        version: versions[member.source_id],
        path: member.path,
        sha256: member.sha256,
        class: member.class,
        entry_type: member.entry_type,
        treatment: "adapt",
        owner: member.source_id === "builder" ? "Forge Builder Runtime" : "Forge BMAD Runtime",
        retained_purpose:
          "Retained shared source resource for governed parsing, configuration, routing, or resource resolution; it is not independently executable.",
        runtime_denominator: false,
        owning_skill: null,
        target_capability: `${member.source_id}:shared-runtime-source`,
        verification_use: "shared_runtime_resource_closure",
      };
    }

    return {
      member_key: memberKey,
      source_id: member.source_id,
      version: versions[member.source_id],
      path: member.path,
      sha256: member.sha256,
      class: member.class,
      entry_type: member.entry_type,
      ...retainedNonruntime(member),
      runtime_denominator: false,
      owning_skill: null,
      target_capability: null,
    };
  });

  const productionSkills = members.filter((member) => member.runtime_denominator);
  const totals = {
    source_members: members.length,
    source_members_by_source: countBy(members, "source_id"),
    treatments: countBy(members, "treatment"),
    method_primary_skills: productionSkills.filter(
      (member) => member.class === "primary_method_skill",
    ).length,
    builder_primary_skills: productionSkills.filter(
      (member) => member.class === "primary_builder_skill",
    ).length,
    builder_public_actions: BUILDER_ACTIONS.filter((action) => action.visibility === "public").length,
    builder_internal_actions: BUILDER_ACTIONS.filter(
      (action) => action.visibility === "internal",
    ).length,
    runtime_support: countBy(productionSkills, "support_state"),
    ui_support: countBy(productionSkills, "ui_state"),
    release_state: countBy(productionSkills, "release_state"),
  };

  return {
    schema_version: "sapphirus.bmad.source-accounting/v1",
    generated_by: "packages/bmad-foundation/scripts/source-accounting.mjs",
    source_manifest: {
      path: "packages/bmad-foundation/source-manifest.json",
      sha256: sha256(manifestBytes),
      member_count: manifest.members.length,
    },
    authority:
      "Treatments describe governed product handling only. Retained source bytes, scripts, prompts, templates, links, and generated content remain untrusted data and grant no authority.",
    treatments: TREATMENTS,
    totals,
    builder_actions: BUILDER_ACTIONS,
    members,
  };
}

function validateAccounting(accounting) {
  assert.equal(accounting.schema_version, "sapphirus.bmad.source-accounting/v1");
  assert.deepEqual(accounting.treatments, TREATMENTS);
  assert.equal(accounting.members.length, 896);
  assert.equal(new Set(accounting.members.map((member) => member.member_key)).size, 896);

  const primary = accounting.members.filter((member) => member.runtime_denominator);
  assert.equal(primary.filter((member) => member.class === "primary_method_skill").length, 48);
  assert.equal(primary.filter((member) => member.class === "primary_builder_skill").length, 5);
  assert.equal(accounting.builder_actions.filter((action) => action.visibility === "public").length, 9);
  assert.equal(
    accounting.builder_actions.filter((action) => action.visibility === "internal").length,
    1,
  );

  for (const member of accounting.members) {
    assert.ok(TREATMENTS.includes(member.treatment), `${member.member_key}: invalid treatment`);
    assert.ok(member.owner, `${member.member_key}: owner is required`);
    assert.ok(member.retained_purpose, `${member.member_key}: retained purpose is required`);
    if (member.runtime_denominator) {
      assert.notEqual(member.treatment, "reject_nonruntime", `${member.member_key}: primary rejected`);
      assert.ok(member.implementation_task, `${member.member_key}: task is required`);
      assert.ok(member.rationale, `${member.member_key}: rationale is required`);
      assert.ok(member.target_capability, `${member.member_key}: target capability is required`);
      assert.ok(member.target_package, `${member.member_key}: target package is required`);
      assert.deepEqual(member.semantic_test_set, SEMANTIC_TEST_SET);
      assert.ok(member.support_state, `${member.member_key}: support state is required`);
      assert.ok(member.ui_state, `${member.member_key}: UI state is required`);
      assert.ok(member.release_state, `${member.member_key}: release state is required`);
      assert.ok(
        !/(?:defer|hidden|normalized.only|unprojected)/iu.test(
          `${member.treatment} ${member.support_state} ${member.ui_state} ${member.release_state}`,
        ),
        `${member.member_key}: prohibited incomplete classification`,
      );
    } else {
      assert.ok(member.verification_use, `${member.member_key}: verification use is required`);
    }
  }
}

async function main() {
  const expected = await generateAccounting();
  validateAccounting(expected);

  if (process.argv.includes("--write")) {
    await writeFile(accountingPath, stableJson(expected), { encoding: "utf8", flag: "w" });
  } else {
    assert.ok(process.argv.includes("--check"), "use --write or --check");
    const actual = JSON.parse(await readFile(accountingPath, "utf8"));
    validateAccounting(actual);
    assert.deepEqual(actual, expected, "source-accounting.json is stale; run with --write");
  }

  console.log(
    "source accounting verified: members=896 primary_method=48 primary_builder=5 actions=10",
  );
}

main().catch((error) => {
  console.error(error.stack ?? error);
  process.exitCode = 1;
});
