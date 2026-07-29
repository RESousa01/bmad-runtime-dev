import { createHash } from "node:crypto";
import { access, readFile, readdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const packageRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const repositoryRoot = path.resolve(packageRoot, "..", "..");
const builderRoot = path.resolve(packageRoot, "source", "builder", "2.1.0");
const defaults = Object.freeze({
  sourceNotes: path.resolve(builderRoot, "source-notes.json"),
  distributionDecision: path.resolve(builderRoot, "distribution-decision.json"),
  documentation: path.resolve(repositoryRoot, "docs", "bmad", "builder-source-notes.md"),
});

function fail(location, message) {
  throw new Error(`[builder_source_notes_invalid] ${location}: ${message}`);
}

function exactKeys(value, expected, location) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(location, "expected an object");
  }
  const actual = Object.keys(value).sort();
  const canonical = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(canonical)) {
    fail(location, `keys must be exactly ${canonical.join(", ")}`);
  }
}

function equal(actual, expected, location) {
  if (actual !== expected) fail(location, `expected ${JSON.stringify(expected)}`);
}

async function parseJson(file, location) {
  let source;
  try {
    source = await readFile(file, "utf8");
  } catch (error) {
    fail(location, `cannot read record: ${error.code ?? error.message}`);
  }
  try {
    return JSON.parse(source);
  } catch {
    fail(location, "invalid JSON");
  }
}

async function sha256(file) {
  return createHash("sha256").update(await readFile(file)).digest("hex");
}

function parseArguments(arguments_) {
  const paths = { ...defaults };
  const names = new Map([
    ["--source-notes", "sourceNotes"],
    ["--distribution-decision", "distributionDecision"],
    ["--documentation", "documentation"],
  ]);
  for (let index = 0; index < arguments_.length; index += 1) {
    if (arguments_[index] === "--package-only") {
      paths.documentation = null;
      continue;
    }
    const key = names.get(arguments_[index]);
    const value = arguments_[index + 1];
    if (key === undefined || value === undefined) {
      fail("arguments", "expected --package-only or a supported path option with a value");
    }
    paths[key] = path.resolve(process.cwd(), value);
    index += 1;
  }
  return paths;
}

function verifyIdentity(identity, lockIdentity, includePackageHash, location) {
  exactKeys(
    identity,
    includePackageHash
      ? ["source_id", "package_name", "version", "archive_sha256", "package_json_sha256"]
      : ["source_id", "package_name", "version", "archive_sha256"],
    location,
  );
  equal(identity.source_id, "builder", `${location}.source_id`);
  equal(identity.package_name, "bmad-builder", `${location}.package_name`);
  equal(identity.version, lockIdentity.version, `${location}.version`);
  equal(identity.archive_sha256, lockIdentity.archive_sha256, `${location}.archive_sha256`);
  if (includePackageHash) {
    equal(
      identity.package_json_sha256,
      lockIdentity.identity_members["package.json"],
      `${location}.package_json_sha256`,
    );
  }
}

export async function verifySourceNotes(paths = defaults) {
  const lock = await parseJson(path.resolve(packageRoot, "source-lock.json"), "source-lock.json");
  const lockIdentity = lock.sources?.find((source) => source.id === "builder");
  if (lockIdentity === undefined) fail("source-lock.json", "Builder identity is missing");
  equal(lockIdentity.version, "2.1.0", "source-lock.json.builder.version");
  equal(
    lockIdentity.archive_sha256,
    "d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852",
    "source-lock.json.builder.archive_sha256",
  );
  equal(lockIdentity.public_action_count, 9, "source-lock.json.builder.public_action_count");
  equal(lockIdentity.internal_action_count, 1, "source-lock.json.builder.internal_action_count");
  equal(
    await sha256(path.resolve(builderRoot, "package.json")),
    lockIdentity.identity_members["package.json"],
    "source/builder/2.1.0/package.json",
  );

  const notes = await parseJson(paths.sourceNotes, "source-notes.json");
  exactKeys(
    notes,
    [
      "schema_version",
      "source_identity",
      "record_kind",
      "upstream_bytes_policy",
      "runtime_authority",
      "corrections",
      "compatibility_metadata",
    ],
    "source-notes.json",
  );
  equal(notes.schema_version, "sapphirus.bmad.builder-source-notes/v1", "source-notes.json.schema_version");
  verifyIdentity(notes.source_identity, lockIdentity, true, "source-notes.json.source_identity");
  equal(notes.record_kind, "forge_compatibility_metadata", "source-notes.json.record_kind");
  equal(
    notes.upstream_bytes_policy,
    "verbatim_archive_members_unchanged",
    "source-notes.json.upstream_bytes_policy",
  );
  equal(
    notes.runtime_authority,
    "non_authoritative_untrusted_source_evidence",
    "source-notes.json.runtime_authority",
  );
  exactKeys(
    notes.compatibility_metadata,
    ["documentation", "distribution_decision"],
    "source-notes.json.compatibility_metadata",
  );
  equal(
    notes.compatibility_metadata.documentation,
    "docs/bmad/builder-source-notes.md",
    "source-notes.json.compatibility_metadata.documentation",
  );
  equal(
    notes.compatibility_metadata.distribution_decision,
    "source/builder/2.1.0/distribution-decision.json",
    "source-notes.json.compatibility_metadata.distribution_decision",
  );

  const correctionExpectations = new Map([
    [
      "builder-agents-layout-drift",
      {
        classification: "stale_contributor_guidance",
        claim: "The runtime source is a src/ tree of YAML agents and workflows.",
        decision: "Do not reconstruct this historical layout; use the five pinned production skills and canonical Forge runtime contracts.",
      },
    ],
    [
      "builder-agents-script-drift",
      {
        classification: "stale_contributor_guidance",
        claim: "npm test and the named schema/reference validation scripts are available.",
        decision: "Do not invoke scripts absent from the pinned package.json; use Forge verification commands.",
      },
    ],
    [
      "builder-agents-distribution-drift",
      {
        classification: "non_authoritative_distribution_guidance",
        claim: "Builder modules are npm-published src/ packages installed directly by npx bmad-method install.",
        decision: "Forge uses the governed bundled/custom-source promotion decision in distribution-decision.json.",
      },
    ],
  ]);
  if (!Array.isArray(notes.corrections) || notes.corrections.length !== correctionExpectations.size) {
    fail("source-notes.json.corrections", "exactly three reviewed corrections are required");
  }
  const correctionIds = new Set();
  for (const [index, correction] of notes.corrections.entries()) {
    const location = `source-notes.json.corrections[${index}]`;
    exactKeys(correction, ["id", "upstream_path", "classification", "claim", "decision"], location);
    equal(correction.upstream_path, "AGENTS.md", `${location}.upstream_path`);
    const expected = correctionExpectations.get(correction.id);
    if (expected === undefined || correctionIds.has(correction.id)) fail(`${location}.id`, "unknown or duplicate correction");
    correctionIds.add(correction.id);
    equal(correction.classification, expected.classification, `${location}.classification`);
    equal(correction.claim, expected.claim, `${location}.claim`);
    equal(correction.decision, expected.decision, `${location}.decision`);
  }

  const agents = await readFile(path.resolve(builderRoot, "AGENTS.md"), "utf8");
  if (!agents.includes("### Source Structure (`src/`)")) {
    fail("source/builder/2.1.0/AGENTS.md", "stale src/ layout claim is no longer evidenced");
  }
  if (!agents.includes("npm test") || !agents.includes("published as npm packages")) {
    fail("source/builder/2.1.0/AGENTS.md", "stale scripts or distribution claim is no longer evidenced");
  }
  const rootEntries = await readdir(builderRoot);
  if (rootEntries.includes("src")) fail("source/builder/2.1.0", "expected documented src/ layout drift");
  const upstreamPackage = await parseJson(path.resolve(builderRoot, "package.json"), "source/builder/2.1.0/package.json");
  equal(upstreamPackage.private, true, "source/builder/2.1.0/package.json.private");
  for (const absentScript of ["test", "test:schemas", "test:refs", "validate:schemas", "validate:refs"]) {
    if (Object.hasOwn(upstreamPackage.scripts, absentScript)) {
      fail("source/builder/2.1.0/package.json.scripts", `${absentScript} is no longer absent`);
    }
  }

  const distribution = await parseJson(paths.distributionDecision, "distribution-decision.json");
  exactKeys(
    distribution,
    ["schema_version", "decision_id", "status", "source_identity", "distribution", "promotion_lifecycle", "eval_runner"],
    "distribution-decision.json",
  );
  equal(
    distribution.schema_version,
    "sapphirus.bmad.builder-distribution-decision/v1",
    "distribution-decision.json.schema_version",
  );
  equal(distribution.decision_id, "SRC-10-builder-2.1.0-distribution", "distribution-decision.json.decision_id");
  equal(distribution.status, "active", "distribution-decision.json.status");
  verifyIdentity(distribution.source_identity, lockIdentity, false, "distribution-decision.json.source_identity");
  const expectedDistribution = {
    bundled_source: "hash_pinned_forge_input",
    custom_source: "governed_import_and_promotion_only",
    marketplace_metadata: "untrusted_compatibility_metadata",
    direct_npm_install: "prohibited",
    direct_marketplace_activation: "prohibited",
    file_presence_activation: "prohibited",
  };
  exactKeys(distribution.distribution, Object.keys(expectedDistribution), "distribution-decision.json.distribution");
  for (const [key, value] of Object.entries(expectedDistribution)) {
    equal(distribution.distribution[key], value, `distribution-decision.json.distribution.${key}`);
  }
  const expectedLifecycle = [
    "source_acquisition_and_identity_verification",
    "validation",
    "analysis",
    "evaluation",
    "review",
    "approval",
    "staging",
    "installation",
    "atomic_catalog_activation",
    "live_conformance",
    "receipt",
  ];
  if (JSON.stringify(distribution.promotion_lifecycle) !== JSON.stringify(expectedLifecycle)) {
    fail("distribution-decision.json.promotion_lifecycle", "governed lifecycle is incomplete or reordered");
  }

  exactKeys(
    distribution.eval_runner,
    [
      "skill_path",
      "public_help_path",
      "visibility",
      "reason",
      "public_action_count",
      "internal_action_count",
      "total_action_count",
      "invocation_boundary",
    ],
    "distribution-decision.json.eval_runner",
  );
  const evalExpected = {
    skill_path: "skills/bmad-eval-runner/SKILL.md",
    public_help_path: "skills/module-help.csv",
    visibility: "internal",
    reason: "present_in_pinned_source_but_absent_from_public_help",
    public_action_count: 9,
    internal_action_count: 1,
    total_action_count: 10,
    invocation_boundary: "typed_builder_evaluation_orchestration",
  };
  for (const [key, value] of Object.entries(evalExpected)) {
    equal(distribution.eval_runner[key], value, `distribution-decision.json.eval_runner.${key}`);
  }
  const evalSkill = await readFile(path.resolve(builderRoot, distribution.eval_runner.skill_path), "utf8");
  if (!/^name: bmad-eval-runner$/mu.test(evalSkill)) {
    fail(distribution.eval_runner.skill_path, "Eval Runner source skill is missing");
  }
  const help = await readFile(path.resolve(builderRoot, distribution.eval_runner.public_help_path), "utf8");
  const helpRows = help.trimEnd().split(/\r?\n/u).slice(1);
  equal(helpRows.length, 10, distribution.eval_runner.public_help_path);
  const publicActionRows = helpRows.filter((row) => !row.startsWith("BMad Builder,_meta,"));
  equal(publicActionRows.length, 9, `${distribution.eval_runner.public_help_path}.public_actions`);
  if (help.includes("bmad-eval-runner")) {
    fail(distribution.eval_runner.public_help_path, "Eval Runner must remain absent from public help");
  }

  if (paths.documentation !== null) {
    await access(paths.documentation);
    const documentation = await readFile(paths.documentation, "utf8");
    for (const required of [
      "untrusted runtime input",
      "Direct npm installation",
      "9 public actions",
      "1 internal Eval Runner action",
      "typed Builder evaluation orchestration",
    ]) {
      if (!documentation.includes(required)) fail("docs/bmad/builder-source-notes.md", `missing required decision text: ${required}`);
    }
  }

  return { publicActions: 9, internalActions: 1, totalActions: 10 };
}

if (process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const result = await verifySourceNotes(parseArguments(process.argv.slice(2)));
    process.stdout.write(
      `Builder 2.1.0 source notes verified: ${result.publicActions} public actions + ${result.internalActions} internal Eval Runner = ${result.totalActions} total actions.\n`,
    );
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
