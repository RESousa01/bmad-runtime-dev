import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  evaluateBmad100,
  exitCodeForResult,
  GATE_DEFINITIONS,
  REQUIRED_COUNTS,
} from "../../verify-bmad-100.mjs";

const NOW = "2026-07-28T12:00:00.000Z";
const COMMIT = "a".repeat(40);
const TREE = "b".repeat(40);

function completeInputs() {
  const methodCapabilities = Array.from({ length: 48 }, (_, index) => ({
    canonicalId: `method:${index + 1}`,
    source: {
      id: "method",
      path: `skills/${index + 1}/SKILL.md`,
      class: "primary_method_skill",
    },
    aliases: index === 0 ? ["method:legacy"] : [],
    supportState: "conformant",
    modes: ["interactive", "headless"],
  }));
  const builderSkills = Array.from({ length: 5 }, (_, index) => ({
    id: `builder-skill-${index + 1}`,
  }));
  const builderCapabilities = Array.from({ length: 10 }, (_, index) => ({
    canonicalId: `builder:action-${index + 1}`,
    source: {
      id: "builder",
      path: `skills/${(index % 5) + 1}/SKILL.md`,
      class: "primary_builder_skill",
    },
    action: `action-${index + 1}`,
    aliases: [],
    visibility: index === 9 ? "internal" : "public",
    supportState: "conformant",
    modes: ["interactive", "headless"],
  }));
  const capabilities = [...methodCapabilities, ...builderCapabilities];
  const aliases = [
    {
      alias: "method:legacy",
      target: "method:1",
      migrationPolicy: "resolve_before_dispatch",
    },
  ];
  const accounting = {
    totals: {
      source_members: 896,
      source_members_by_source: { method: 597, builder: 299 },
      method_primary_skills: 48,
      builder_primary_skills: 5,
      builder_public_actions: 9,
      builder_internal_actions: 1,
    },
    members: [
      ...methodCapabilities.map((capability, index) => ({
        member_key: `method:${index + 1}`,
        class: "primary_method_skill",
        runtime_denominator: true,
        treatment: "adapt",
        support_state: "conformant",
        release_state: "released",
        target_capability: capability.canonicalId,
      })),
      ...builderSkills.map((skill, index) => ({
        member_key: skill.id,
        class: "primary_builder_skill",
        runtime_denominator: true,
        treatment: "adapt",
        support_state: "conformant",
        release_state: "released",
        target_capability: `builder:action-${index + 1}`,
      })),
    ],
    builder_actions: builderCapabilities.map((capability) => ({
      id: capability.canonicalId,
      visibility: capability.visibility,
      support_state: "conformant",
      release_state: "released",
    })),
  };
  const evidence = Object.fromEntries(
    GATE_DEFINITIONS.map((gate) => [
      gate.id,
      {
        schema_version: "sapphirus.bmad.gate-evidence/v1",
        gate_id: gate.id,
        status: "pass",
        generated_at: NOW,
        commit: COMMIT,
        source_tree: TREE,
        evidence_hashes: {
          report: "c".repeat(64),
        },
        skipped: false,
        unavailable: false,
        partial: false,
        incomparable: false,
        commands: [
          {
            command: `verify ${gate.id}`,
            exit_code: 0,
            required_for_pass: true,
          },
        ],
        conformance:
          gate.id === "method_semantics" || gate.id === "builder_conformance"
            ? {
                interactive: true,
                headless: true,
                error: true,
                resume: true,
                cancel: true,
              }
            : undefined,
        environment:
          gate.phase === "external_or_release"
            ? { kind: "qualified_test_environment", executed: true }
            : undefined,
        approvals:
          gate.id === "independent_review"
            ? [
                "product",
                "runtime",
              "security",
              "qa",
              "operations",
              ].map((role) => ({
                role,
                approver: `test-${role}-owner`,
                approved: true,
              }))
            : undefined,
      },
    ]),
  );
  return {
    now: NOW,
    expectedCommit: COMMIT,
    expectedTree: TREE,
    sourceLock: {
      sources: [
        {
          id: "method",
          version: "6.10.0",
          archive_sha256:
            "2993a38a36a9586d16a62a4eacb3400db25dd1ea594bbfe4682789d7f95ac02b",
          regular_file_count: 597,
          primary_skill_count: 48,
        },
        {
          id: "builder",
          version: "2.1.0",
          archive_sha256:
            "d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852",
          regular_file_count: 299,
          primary_skill_count: 5,
          public_action_count: 9,
          internal_action_count: 1,
        },
      ],
    },
    accounting,
    catalog: {
      counts: { method: 48, builderPublic: 9, builderInternal: 1, total: 58 },
      capabilities,
    },
    aliases: { aliases },
    evidence,
  };
}

test("the mandatory gate set covers the full binary completion boundary", () => {
  assert.deepEqual(REQUIRED_COUNTS, {
    methodPrimarySkills: 48,
    builderProductionSkills: 5,
    builderPublicActions: 9,
    builderInternalActions: 1,
    methodSourceMembers: 597,
    builderSourceMembers: 299,
  });
  assert.deepEqual(
    new Set(GATE_DEFINITIONS.map((gate) => gate.category)),
    new Set([
      "source",
      "closure",
      "contracts",
      "semantics",
      "builder",
      "runtime",
      "ui",
      "support_plane",
      "desktop_e2e",
      "security",
      "performance",
      "persistence",
      "packaging",
      "deployment",
      "documentation",
      "pilot",
      "traceability",
      "independent_review",
    ]),
  );
});

test("the inert scorecard template stays synchronized with the verifier", () => {
  const template = JSON.parse(
    readFileSync(
      new URL(
        "../../../packages/bmad-foundation/bmad-100-scorecard.json",
        import.meta.url,
      ),
      "utf8",
    ),
  );
  assert.equal(template.template_only, true);
  assert.equal(template.complete, false);
  assert.equal(template.completion_percent, 0);
  assert.equal(
    template.pinned_sources.method.archive_sha256,
    "2993a38a36a9586d16a62a4eacb3400db25dd1ea594bbfe4682789d7f95ac02b",
  );
  assert.equal(
    template.pinned_sources.builder.archive_sha256,
    "d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852",
  );
  assert.deepEqual(
    template.mandatory_gates.map(
      ({ id, category, evidence_path, max_age_hours }) => ({
        id,
        category,
        evidencePath: evidence_path,
        maxAgeHours: max_age_hours,
      }),
    ),
    GATE_DEFINITIONS.map(
      ({ id, category, evidencePath, maxAgeHours }) => ({
        id,
        category,
        evidencePath,
        maxAgeHours,
      }),
    ),
  );
});

test("genuine, current, exact evidence is the only zero-result state", () => {
  const result = evaluateBmad100(completeInputs());
  assert.equal(result.complete, true);
  assert.equal(result.completion_percent, 100);
  assert.deepEqual(result.blockers, []);
  assert.equal(exitCodeForResult(result), 0);
  assert.equal(
    exitCodeForResult({ complete: true, completion_percent: 99 }),
    1,
  );
});

test("missing, stale, partial, skipped, unavailable, and incomparable gates fail precisely", () => {
  const inputs = completeInputs();
  delete inputs.evidence.runtime;
  inputs.evidence.ui.generated_at = "2026-07-01T00:00:00.000Z";
  inputs.evidence.security.partial = true;
  inputs.evidence.packaging.skipped = true;
  inputs.evidence.deployment.unavailable = true;
  inputs.evidence.pilot.incomparable = true;

  const result = evaluateBmad100(inputs);
  assert.equal(result.complete, false);
  assert.equal(result.completion_percent, 0);
  assert.equal(exitCodeForResult(result), 1);
  for (const [gate, code] of [
    ["runtime", "missing_evidence"],
    ["ui", "stale_evidence"],
    ["security", "partial_evidence"],
    ["packaging", "skipped_evidence"],
    ["deployment", "unavailable_evidence"],
    ["pilot", "incomparable_evidence"],
  ]) {
    assert.ok(
      result.blockers.some(
        (blocker) => blocker.gate === gate && blocker.code === code,
      ),
      `${gate} must report ${code}`,
    );
  }
});

test("source counts, catalog counts, aliases, accounting, and conformance are binary", () => {
  const inputs = completeInputs();
  inputs.accounting.totals.method_primary_skills = 47;
  inputs.catalog.counts.builderPublic = 8;
  inputs.aliases.aliases[0].target = "method:missing";
  inputs.accounting.members[0].support_state = "implementation_pending";
  inputs.evidence.method_semantics.conformance.cancel = false;

  const result = evaluateBmad100(inputs);
  assert.equal(result.complete, false);
  assert.ok(result.blockers.some((blocker) => blocker.code === "count_mismatch"));
  assert.ok(result.blockers.some((blocker) => blocker.code === "invalid_alias"));
  assert.ok(
    result.blockers.some((blocker) => blocker.code === "source_not_conformant"),
  );
  assert.ok(
    result.blockers.some((blocker) => blocker.code === "missing_conformance"),
  );
});

test("unit/source evidence cannot substitute for release and independent gates", () => {
  const inputs = completeInputs();
  for (const gate of GATE_DEFINITIONS.filter(
    (candidate) => candidate.phase === "external_or_release",
  )) {
    delete inputs.evidence[gate.id];
  }
  const result = evaluateBmad100(inputs);
  assert.equal(result.complete, false);
  assert.ok(
    result.blockers.some((blocker) => blocker.gate === "desktop_e2e"),
  );
  assert.ok(result.blockers.some((blocker) => blocker.gate === "packaging"));
  assert.ok(result.blockers.some((blocker) => blocker.gate === "deployment"));
  assert.ok(
    result.blockers.some(
      (blocker) => blocker.gate === "independent_review",
    ),
  );
});
