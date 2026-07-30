#!/usr/bin/env node

// Per-skill conformance runner for the MTH-01..MTH-48 Method-skill program.
//
// Without this script, "is this skill's adapter actually conformant" would be
// answered by whatever ad hoc check the 48th task happened to invent last.
// This is the one gate every MTH task calls, either for its own skill
// (`--skill <id> --all-modes --strict`) or, in `--all` mode, as the final
// program-wide gate: it refuses a `pending` ledger row (Phase 2 is honestly
// incomplete until every row is decided) and refuses a skill whose declared
// invocation modes have no fixture evidence.
//
// Argument handling, output shape (one JSON summary object on stdout on
// success), and exit-code discipline (throw a message-carrying `fail`,
// caught once at the top, printed to stderr, `process.exitCode = 1`) mirror
// `packages/bmad-foundation/scripts/resource-closure.mjs` deliberately, so a
// caller invoking either script does not have to learn two conventions.

import {
  existsSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const packageRoot = resolve(dirname(scriptPath), "..");
const catalogPath = resolve(packageRoot, "capability-catalog.json");
const ledgerPath = resolve(packageRoot, "compatibility-ledger.json");
const fixturesRoot = resolve(packageRoot, "tests", "fixtures");

function fail(message) {
  throw new Error(`conformance failed: ${message}`);
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function usage() {
  fail(
    "usage: node packages/bmad-foundation/scripts/conformance.mjs (--skill <rowId> | --all) [--all-modes | --mode all] [--strict | --check] [--json <path>]",
  );
}

function parseArgs(argv) {
  const flags = {
    skill: null,
    all: false,
    allModes: false,
    strict: false,
    json: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--skill") {
      index += 1;
      flags.skill = argv[index];
      if (!flags.skill) usage();
    } else if (token === "--all") {
      flags.all = true;
    } else if (token === "--all-modes") {
      flags.allModes = true;
    } else if (token === "--mode") {
      index += 1;
      if (argv[index] !== "all") usage();
      flags.allModes = true;
    } else if (token === "--strict") {
      flags.strict = true;
    } else if (token === "--check") {
      flags.strict = true;
    } else if (token === "--json") {
      index += 1;
      flags.json = argv[index];
      if (!flags.json) usage();
    } else {
      usage();
    }
  }
  if (flags.all === (flags.skill !== null)) {
    // Exactly one of --all or --skill must be supplied, never both, never
    // neither -- an ambiguous invocation must not silently default to one.
    usage();
  }
  return flags;
}

// A row id such as "method:src/bmm-skills/1-analysis/.../SKILL.md" is not a
// safe directory component; this is the one place that mapping is defined,
// so every caller -- this script, and any MTH task authoring a fixture
// directory -- agrees on the same on-disk name.
export function fixtureDirName(rowId) {
  return rowId.replaceAll(/[/:]/gu, "__");
}

function methodRows(catalog) {
  return catalog.capabilities.filter(
    (capability) => capability.source.class === "primary_method_skill",
  );
}

function resolveSkillSelector(selector, catalog) {
  const rows = methodRows(catalog);
  const exact = rows.find((row) => row.rowId === selector);
  if (exact) return exact.rowId;
  const matches = rows.filter(
    (row) =>
      row.source.path === selector
      || row.source.path.endsWith(`/${selector}/SKILL.md`),
  );
  if (matches.length !== 1) {
    fail(`skill selector ${selector} did not resolve exactly once`);
  }
  return matches[0].rowId;
}

// Skills are addressed by `rowId`, not `canonicalId`: several source
// SKILL.md files (a v6 shim and the capability it delegates to, for example)
// legitimately share one canonical id, so `canonicalId` alone cannot select
// a single one of the 48 skills this program has to individually conform.
function checkSkill(rowId, catalog, ledger, { allModes }) {
  const failures = [];
  const catalogRow = methodRows(catalog).find((capability) => capability.rowId === rowId);
  if (!catalogRow) {
    failures.push(`unknown skill ${rowId} (not a method capability in the catalog)`);
    return failures;
  }
  const ledgerRow = ledger.rows.find((row) => row.rowId === rowId);
  if (!ledgerRow) {
    failures.push(`${rowId} has no compatibility-ledger row`);
    return failures;
  }
  if (ledgerRow.decision === "pending") {
    failures.push(`${rowId} ledger decision is still pending`);
  }
  if (ledgerRow.sourceSha256 !== `sha256:${catalogRow.source.sha256}`) {
    failures.push(`${rowId} ledger sourceSha256 does not match the catalog`);
  }
  const dirName = fixtureDirName(rowId);
  const fixtureDir = resolve(fixturesRoot, dirName);
  if (!existsSync(fixtureDir)) {
    failures.push(`${rowId} has no fixture directory at tests/fixtures/${dirName}`);
    return failures;
  }
  if (allModes) {
    const declaredModes = catalogRow.modes ?? [];
    if (declaredModes.length === 0) {
      failures.push(`${rowId} declares no invocation modes to check fixtures for`);
    }
    const present = new Set(
      readdirSync(fixtureDir, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => entry.name),
    );
    for (const mode of declaredModes) {
      if (!present.has(mode)) {
        failures.push(
          `${rowId} declares mode ${mode} with no fixture at tests/fixtures/${dirName}/${mode}`,
        );
      }
    }
  }
  return failures;
}

export function run(argv = process.argv.slice(2)) {
  let receiptPath = null;
  try {
    const flags = parseArgs(argv);
    receiptPath = flags.json;
    if (receiptPath) rmSync(receiptPath, { force: true });
    const catalog = readJson(catalogPath);
    const ledger = readJson(ledgerPath);
    const targets = flags.all
      ? methodRows(catalog).map((row) => row.rowId)
      : [resolveSkillSelector(flags.skill, catalog)];

    const failures = targets.flatMap((rowId) =>
      checkSkill(rowId, catalog, ledger, { allModes: flags.allModes || flags.all }),
    );
    if (failures.length > 0) fail(failures[0]);

    const summary = {
      schemaVersion: "sapphirus.bmad.conformance-report/v1",
      generatedBy: "packages/bmad-foundation/scripts/conformance.mjs",
      strict: flags.strict,
      allModes: flags.allModes || flags.all,
      skillsChecked: targets.length,
      skills: targets,
    };
    const serialized = `${JSON.stringify(summary)}\n`;
    if (receiptPath) writeFileSync(receiptPath, serialized, "utf8");
    process.stdout.write(serialized);
    return 0;
  } catch (error) {
    if (!receiptPath) {
      const index = argv.indexOf("--json");
      receiptPath = index >= 0 ? argv[index + 1] : null;
    }
    if (receiptPath) rmSync(receiptPath, { force: true });
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(scriptPath)) {
  process.exitCode = run();
}
