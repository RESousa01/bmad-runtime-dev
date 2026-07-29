#!/usr/bin/env node
// Generates the canonical Builder action/capability catalog (BLD-02).
//
// The nine public module-help.csv rows plus the internal Eval Runner action
// are the only source of truth. This script is the source of truth for the
// two committed JSON files -- they are its output, never hand-edited. Run
// with --check to verify the committed files are current without writing.

import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const foundationRoot = resolve(dirname(scriptPath), "..");
const checkOnly = process.argv.includes("--check");

const PINNED_SOURCE_ROOT = resolve(foundationRoot, "source/builder/2.1.0");
const CSV_RELATIVE_PATH = "skills/module-help.csv";
const EVAL_SKILL_RELATIVE_PATH = "skills/bmad-eval-runner/SKILL.md";

const paths = {
  csv: resolve(PINNED_SOURCE_ROOT, CSV_RELATIVE_PATH),
  evalSkill: resolve(PINNED_SOURCE_ROOT, EVAL_SKILL_RELATIVE_PATH),
  catalog: resolve(foundationRoot, "normalized/builder-capability-catalog.json"),
  aliases: resolve(foundationRoot, "normalized/builder-alias-registry.json"),
};

// -- generic helpers ---------------------------------------------------

function sha256Hex(value) {
  return createHash("sha256").update(value).digest("hex");
}

function sha256Tag(value) {
  return `sha256:${sha256Hex(value)}`;
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

const CROCKFORD32 = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

// Deterministic, reproducible substitute for a random ULID: encodes the
// sha256 of `seed` as Crockford base32 (excludes I/L/O/U, matches the
// ContractId charset) and truncates to 26 chars. Same seed -> same id, so
// regeneration is diffable and no wall-clock or RNG ever leaks into a
// committed file.
function deterministicId(prefix, seed) {
  const digest = createHash("sha256").update(seed).digest();
  let bits = "";
  for (const byte of digest) {
    bits += byte.toString(2).padStart(8, "0");
  }
  let out = "";
  for (let index = 0; index + 5 <= bits.length && out.length < 26; index += 5) {
    out += CROCKFORD32[parseInt(bits.slice(index, index + 5), 2)];
  }
  while (out.length < 26) out += "0";
  return `${prefix}_${out}`;
}

function slugifyArgName(token) {
  const cleaned = token
    .trim()
    .replace(/^-+/, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  const named = cleaned.length > 0 ? cleaned : "arg";
  return /^[a-z]/.test(named) ? named : `arg_${named}`;
}

// -- CSV parsing ---------------------------------------------------------

// Minimal RFC4180-ish parser: fields are comma-separated, quoted fields may
// contain commas, and "" inside a quoted field is a literal quote. No
// multi-line quoted fields appear in the pinned source, so this stays
// single-line by design -- a multi-line field is out of scope, not silently
// mishandled.
function parseCsvLine(line) {
  const fields = [];
  let current = "";
  let inQuotes = false;
  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];
    if (inQuotes) {
      if (char === '"') {
        if (line[index + 1] === '"') {
          current += '"';
          index += 1;
        } else {
          inQuotes = false;
        }
      } else {
        current += char;
      }
    } else if (char === '"') {
      inQuotes = true;
    } else if (char === ",") {
      fields.push(current);
      current = "";
    } else {
      current += char;
    }
  }
  fields.push(current);
  return fields;
}

const CSV_COLUMNS = [
  "module",
  "skill",
  "displayName",
  "menuCode",
  "description",
  "action",
  "args",
  "phase",
  "precededBy",
  "followedBy",
  "required",
  "outputLocation",
  "outputs",
];

// Reject, don't repair: this throws (with a distinct, greppable message) the
// moment a row is structurally broken, rather than quietly skipping it.
export function parseModuleHelpCsv(csvText) {
  const lines = csvText.split(/\r?\n/).filter((line) => line.length > 0);
  if (lines.length === 0) {
    throw new Error("module-help.csv is empty");
  }
  const header = parseCsvLine(lines[0]);
  const expectedHeader = [
    "module",
    "skill",
    "display-name",
    "menu-code",
    "description",
    "action",
    "args",
    "phase",
    "preceded-by",
    "followed-by",
    "required",
    "output-location",
    "outputs",
  ];
  if (header.length !== expectedHeader.length || header.some((cell, i) => cell !== expectedHeader[i])) {
    throw new Error(`module-help.csv header does not match the expected column set: ${header.join(",")}`);
  }

  const rows = [];
  for (let lineIndex = 1; lineIndex < lines.length; lineIndex += 1) {
    const cells = parseCsvLine(lines[lineIndex]);
    if (cells.length !== expectedHeader.length) {
      throw new Error(
        `module-help.csv row ${lineIndex + 1} has ${cells.length} columns, expected ${expectedHeader.length}`,
      );
    }
    const row = {};
    CSV_COLUMNS.forEach((key, columnIndex) => {
      row[key] = cells[columnIndex];
    });
    row.rowNumber = lineIndex + 1; // 1-based, header is row 1
    rows.push(row);
  }
  return rows;
}

function parseArgs(rawArgs) {
  if (!rawArgs || rawArgs.trim().length === 0) return [];
  const groups = rawArgs.split("|").map((group) => group.trim()).filter(Boolean);
  return groups.map((group) => {
    const inner = group.replace(/^\{/, "").replace(/\}$/, "");
    const colonIndex = inner.indexOf(":");
    const token = colonIndex === -1 ? inner : inner.slice(0, colonIndex);
    const isFlag = token.trim().startsWith("-");
    return {
      name: slugifyArgName(token),
      required: false,
      valueKind: isFlag ? "boolean" : "string",
    };
  });
}

// A predecessor/follower cell is authored as "skill:action". The schema
// field itself only carries the bare action id (hyphen-slug), so the
// qualified reference is validated for existence here and then narrowed.
function parseSkillActionRef(cell) {
  if (!cell || cell.trim().length === 0) return null;
  const trimmed = cell.trim();
  const colonIndex = trimmed.indexOf(":");
  if (colonIndex === -1) {
    throw new Error(`Malformed predecessor/follower reference (expected skill:action): ${trimmed}`);
  }
  return { skill: trimmed.slice(0, colonIndex), action: trimmed.slice(colonIndex + 1), raw: trimmed };
}

function moduleSlug(rawModule) {
  return rawModule
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

// -- dependency vocabulary (crates/desktop-runtime/src/bmad/dependencies.rs) --

function dep(id, kind, edge, condition) {
  const out = { id, kind, edge };
  if (condition) out.condition = condition;
  return out;
}

// Per-action dependency template. Deliberately exercises all seven
// dependency kinds and all three edge kinds across the catalog as a whole,
// rather than cramming every kind onto a single action.
function dependenciesFor(action) {
  switch (action) {
    case "configure":
      return [
        dep("source_resource_skill_md", "source_resource", "required"),
        dep("configuration_bmad_builder_config", "configuration", "required"),
      ];
    case "build-process":
      return [
        dep("source_resource_skill_md", "source_resource", "required"),
        dep("output_contract_builder_draft", "output_contract", "required"),
        dep("model_profile_builder_default", "model_profile", "optional"),
      ];
    case "quality-analysis":
      return [
        dep("source_resource_skill_md", "source_resource", "required"),
        dep("output_contract_builder_draft", "output_contract", "required"),
        dep("model_profile_builder_default", "model_profile", "optional"),
      ];
    case "convert-process":
      return [
        dep("source_resource_skill_md", "source_resource", "required"),
        dep("output_contract_builder_draft", "output_contract", "required"),
        dep("model_profile_builder_default", "model_profile", "optional"),
        dep("ui_support_convert_flag", "ui_support", "conditional", "convert_flag_present"),
      ];
    case "ideate-module":
      return [
        dep("source_resource_skill_md", "source_resource", "required"),
        dep("output_contract_builder_draft", "output_contract", "required"),
        dep("model_profile_builder_default", "model_profile", "optional"),
      ];
    case "create-module":
      return [
        dep("source_resource_skill_md", "source_resource", "required"),
        dep("output_contract_builder_draft", "output_contract", "required"),
        dep("script_plan_scaffold_module", "script_plan", "required"),
      ];
    case "validate-module":
      return [
        dep("source_resource_skill_md", "source_resource", "required"),
        dep("output_contract_builder_draft", "output_contract", "required"),
        dep("runtime_port_validation_engine", "runtime_port", "required"),
      ];
    default:
      throw new Error(`No dependency template declared for action ${action}`);
  }
}

function evalRunnerDependencies() {
  return [
    dep("source_resource_skill_md", "source_resource", "required"),
    dep("runtime_port_eval_adapter", "runtime_port", "required"),
    dep("script_plan_run_evals", "script_plan", "required"),
    dep("configuration_bmad_builder_config", "configuration", "optional"),
  ];
}

// -- catalog assembly ------------------------------------------------------

function buildPublicCapability({ row, csvSourceId, csvHash }) {
  if (!row.skill || row.skill.trim().length === 0) {
    throw new Error(`Orphan action: module-help.csv row ${row.rowNumber} has an action but no skill`);
  }
  if (!row.action || row.action.trim().length === 0) {
    throw new Error(`Orphan skill: module-help.csv row ${row.rowNumber} has a skill (${row.skill}) but no action`);
  }
  if (!row.outputLocation?.trim() || !row.outputs?.trim()) {
    throw new Error(
      `Public action missing output contract: ${row.skill}:${row.action} (module-help.csv row ${row.rowNumber})`,
    );
  }

  const module = moduleSlug(row.module);
  const capabilityId = deterministicId("capability", `builder:${module}:${row.skill}:${row.action}`);
  const body = {
    schemaVersion: "sapphirus.builder-capability.v1",
    capabilityId,
    action: row.action,
    exposure: "public",
    module,
    skill: row.skill,
    displayName: row.displayName,
    menuCode: row.menuCode,
    description: row.description,
    phase: row.phase,
    required: row.required.trim().toLowerCase() === "true",
    args: parseArgs(row.args),
    outputs: {
      outputContractId: "sapphirus.bmad-artifact.v1",
      outputLocation: row.outputLocation,
    },
    provenance: {
      sourceId: csvSourceId,
      sourceFile: CSV_RELATIVE_PATH,
      sourceHash: csvHash,
      sourceRow: row.rowNumber,
    },
    dependencies: dependenciesFor(row.action),
  };
  const predecessor = parseSkillActionRef(row.precededBy);
  const follower = parseSkillActionRef(row.followedBy);
  if (predecessor) body.predecessor = predecessor.action;
  if (follower) body.follower = follower.action;
  const capabilityHash = sha256Tag(stableJson(body));
  return { ...body, capabilityHash };
}

const EVAL_RUNNER_TITLE_PATTERN = /^#\s+(.+)$/m;
const EVAL_RUNNER_DESCRIPTION_PATTERN = /^description:\s*(.+)$/m;

function buildEvalCapability({ evalSkillText, evalSourceId, evalHash }) {
  const titleMatch = evalSkillText.match(EVAL_RUNNER_TITLE_PATTERN);
  const descriptionMatch = evalSkillText.match(EVAL_RUNNER_DESCRIPTION_PATTERN);
  if (!titleMatch || !descriptionMatch) {
    throw new Error("bmad-eval-runner/SKILL.md is missing a title heading or description frontmatter");
  }
  const body = {
    schemaVersion: "sapphirus.builder-capability.v1",
    capabilityId: deterministicId("capability", "builder:bmad-builder:bmad-eval-runner:run-evals"),
    action: "run-evals",
    exposure: "internal",
    module: "bmad-builder",
    skill: "bmad-eval-runner",
    displayName: titleMatch[1].trim(),
    description: descriptionMatch[1].trim().slice(0, 4096),
    provenance: {
      sourceId: evalSourceId,
      sourceFile: EVAL_SKILL_RELATIVE_PATH,
      sourceHash: evalHash,
    },
    dependencies: evalRunnerDependencies(),
  };
  const capabilityHash = sha256Tag(stableJson(body));
  return { ...body, capabilityHash };
}

function validateRelationships(publicCapabilities) {
  const byRef = new Map(publicCapabilities.map((cap) => [`${cap.skill}:${cap.action}`, cap]));
  const menuCodes = new Set();
  for (const cap of publicCapabilities) {
    if (menuCodes.has(cap.menuCode)) {
      throw new Error(`Duplicate menu code: ${cap.menuCode} (capability ${cap.skill}:${cap.action})`);
    }
    menuCodes.add(cap.menuCode);
  }
}

function resolveRawPredecessorFollower(rows) {
  const byRef = new Set(rows.map((row) => `${row.skill}:${row.action}`));
  for (const row of rows) {
    for (const [label, cell] of [["preceded-by", row.precededBy], ["followed-by", row.followedBy]]) {
      const ref = parseSkillActionRef(cell);
      if (ref && !byRef.has(`${ref.skill}:${ref.action}`)) {
        throw new Error(
          `Unresolved ${label} reference "${ref.raw}" on row ${row.rowNumber} (${row.skill}:${row.action})`,
        );
      }
    }
  }
}

export function buildCatalog({ csvText, evalSkillText }) {
  const rows = parseModuleHelpCsv(csvText);
  const dataRows = rows.filter((row) => row.skill !== "_meta");
  resolveRawPredecessorFollower(dataRows);

  const csvHash = sha256Tag(csvText);
  const evalHash = sha256Tag(evalSkillText);
  const csvSourceId = deterministicId("source", `builder:${CSV_RELATIVE_PATH}`);
  const evalSourceId = deterministicId("source", `builder:${EVAL_SKILL_RELATIVE_PATH}`);

  const publicCapabilities = dataRows
    .map((row) => buildPublicCapability({ row, csvSourceId, csvHash }))
    .sort((a, b) => `${a.module}:${a.action}`.localeCompare(`${b.module}:${b.action}`));
  validateRelationships(publicCapabilities);

  const internalCapabilities = [buildEvalCapability({ evalSkillText, evalSourceId, evalHash })];

  const counts = {
    public: publicCapabilities.length,
    internal: internalCapabilities.length,
    total: publicCapabilities.length + internalCapabilities.length,
  };
  if (counts.public !== 9 || counts.internal !== 1) {
    throw new Error(`Pinned Builder capability counts do not match: ${JSON.stringify(counts)}`);
  }

  const catalogBody = {
    schemaVersion: "sapphirus.bmad.builder-capability-catalog/v1",
    generatedBy: "packages/bmad-foundation/scripts/generate-builder-catalog.mjs",
    pinnedSource: { id: "builder", version: "2.1.0" },
    counts,
    publicActions: publicCapabilities,
    internalActions: internalCapabilities,
  };
  const catalog = { ...catalogBody, catalogHash: sha256Tag(stableJson(catalogBody)) };
  return catalog;
}

// -- alias registry ----------------------------------------------------

// The five legacy Forge Builder action ids, found in
// crates/desktop-app/src/bmad_capability_host.rs (CAPABILITY_TABLE entries
// whose id starts with "builder:") and mirrored in
// apps/desktop-ui/src/generated/bmadCapabilities.ts.
const LEGACY_FORGE_BUILDER_IDS = [
  { legacyId: "builder:agent.analyze", skill: "bmad-agent-builder", action: "quality-analysis", translation: "identity" },
  { legacyId: "builder:agent.create_rebuild", skill: "bmad-agent-builder", action: "build-process", translation: "identity" },
  { legacyId: "builder:agent.edit", skill: "bmad-agent-builder", action: "build-process", translation: "input_mapped" },
  { legacyId: "builder:workflow.analyze", skill: "bmad-workflow-builder", action: "quality-analysis", translation: "identity" },
  { legacyId: "builder:workflow.build_edit", skill: "bmad-workflow-builder", action: "build-process", translation: "identity" },
];

export function buildAliasRegistry(catalog) {
  const byRef = new Map(catalog.publicActions.map((cap) => [`${cap.skill}:${cap.action}`, cap]));
  const aliases = LEGACY_FORGE_BUILDER_IDS.map(({ legacyId, skill, action, translation }) => {
    const canonical = byRef.get(`${skill}:${action}`);
    if (!canonical) {
      throw new Error(`Legacy Forge Builder id ${legacyId} has no canonical target ${skill}:${action}`);
    }
    return {
      legacyId,
      canonicalId: canonical.capabilityId,
      canonicalAction: `${canonical.module}:${canonical.skill}:${canonical.action}`,
      translation,
      status: "deprecated",
      source: "crates/desktop-app/src/bmad_capability_host.rs",
    };
  }).sort((a, b) => a.legacyId.localeCompare(b.legacyId));

  const registryBody = {
    schemaVersion: "sapphirus.bmad.builder-alias-registry/v1",
    generatedBy: "packages/bmad-foundation/scripts/generate-builder-catalog.mjs",
    sourceCatalogHash: catalog.catalogHash,
    counts: { legacyForgeIds: aliases.length, canonicalPublicActions: catalog.counts.public },
    aliases,
  };
  return { ...registryBody, registryHash: sha256Tag(stableJson(registryBody)) };
}

// Resolves a legacy Forge Builder id to its canonical action, or returns a
// migration error object for anything not in the registry. Never falls
// through silently -- an unknown id is a diagnostic, not a pass-through.
export function resolveLegacyBuilderId(legacyId, registry) {
  const match = registry.aliases.find((alias) => alias.legacyId === legacyId);
  if (match) {
    return { ok: true, canonicalId: match.canonicalId, canonicalAction: match.canonicalAction };
  }
  return {
    ok: false,
    migrationError: `unknown legacy Builder id: ${legacyId}`,
  };
}

// -- CLI -------------------------------------------------------------------

async function loadSources() {
  const [csvText, evalSkillText] = await Promise.all([
    readFile(paths.csv, "utf8"),
    readFile(paths.evalSkill, "utf8"),
  ]);
  return { csvText, evalSkillText };
}

async function main() {
  const { csvText, evalSkillText } = await loadSources();
  const catalog = buildCatalog({ csvText, evalSkillText });
  const registry = buildAliasRegistry(catalog);

  const outputs = new Map([
    [paths.catalog, stableJson(catalog)],
    [paths.aliases, stableJson(registry)],
  ]);

  const stale = [];
  for (const [path, expected] of outputs) {
    if (checkOnly) {
      let actual = null;
      try {
        actual = await readFile(path, "utf8");
      } catch (error) {
        if (error.code !== "ENOENT") throw error;
      }
      if (actual !== expected) stale.push(path);
      continue;
    }
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, expected, "utf8");
  }

  if (stale.length > 0) {
    throw new Error(
      `Generated Builder capability catalog is stale:\n${stale.map((path) => `- ${path}`).join("\n")}`,
    );
  }
  console.log(
    checkOnly
      ? "Builder capability catalog and alias registry are current."
      : "Generated Builder capability catalog and alias registry.",
  );
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1]);
if (isMain) {
  await main();
}
