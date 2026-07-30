#!/usr/bin/env node

// Statically proves that skill prose cannot grant authority.
//
// A Method skill's source is instructions for a model, not code the runtime
// trusts -- but nothing stops a SKILL.md, a step file, or a customized
// adapter from *saying* "run this shell command" or *importing* a
// filesystem/process/network/model capability the harness never grants. This
// script is the one static check every MTH task runs (`pnpm
// verify:bmad:authority -- --skill <rowId>`, and `--all` as the final gate):
// it scans a skill's pinned source directory for a literal authority-shaped
// reference, and, once the skill's Rust adapter exists at the conventional
// path (`crates/desktop-runtime/src/bmad/skills/adapters/<fixtureDirName>.rs`),
// scans that too and asserts it declares no capability beyond what
// `crates/desktop-runtime/src/bmad/skills/harness.rs` structurally permits
// -- nothing, by construction.
//
// Argument handling and exit-code discipline mirror
// `packages/bmad-foundation/scripts/resource-closure.mjs`: a `fail(message)`
// throws, is caught once at the top, printed to stderr, and sets
// `process.exitCode = 1`. A successful run prints one JSON summary to
// stdout.

import {
  existsSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const foundationRoot = resolve(repositoryRoot, "packages/bmad-foundation");
const catalogPath = resolve(foundationRoot, "capability-catalog.json");
const methodSourceRoot = resolve(foundationRoot, "source/method/6.10.0");
const adapterDir = resolve(
  repositoryRoot,
  "crates/desktop-runtime/src/bmad/skills/adapters",
);

// Tokens that would grant real authority if they appeared as live code in a
// skill's source tree or its Rust adapter -- the same forbidden set
// `harness.rs`'s own structural-scan tests assert are absent from the
// harness itself, so a skill or adapter cannot smuggle in through prose or
// an import what the harness itself refuses to expose.
const FORBIDDEN_TOKENS = [
  "std::fs",
  "std::process",
  "std::net",
  "tokio::net",
  "tokio::process",
  "reqwest",
  "Command::new",
  "model_port",
  "ModelPort",
  "child_process",
  "require(\"fs\")",
  "require('fs')",
  "fs.readFile",
  "fs.writeFile",
  "readFileSync",
  "writeFileSync",
  "fetch(",
  "XMLHttpRequest",
  "http.request",
  "net.connect",
  "eval(",
  "subprocess.",
  "os.system",
];

const TEXT_EXTENSIONS = new Set([
  ".md",
  ".mdx",
  ".toml",
  ".yaml",
  ".yml",
  ".csv",
  ".js",
  ".mjs",
  ".cjs",
  ".ts",
  ".rs",
  ".txt",
]);

function fail(message) {
  throw new Error(`authority verification failed: ${message}`);
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function usage() {
  fail(
    "usage: node tools/bmad100/verify-authority.mjs (--skill <rowId> | --all)",
  );
}

function parseArgs(argv) {
  const flags = { skill: null, all: false, json: null };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--skill") {
      index += 1;
      flags.skill = argv[index];
      if (!flags.skill) usage();
    } else if (token === "--all") {
      flags.all = true;
    } else if (token === "--json") {
      index += 1;
      flags.json = argv[index];
      if (!flags.json) usage();
    } else {
      usage();
    }
  }
  if (flags.all === (flags.skill !== null)) usage();
  return flags;
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

function listFiles(root) {
  const out = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const full = resolve(root, entry.name);
    if (entry.isDirectory()) {
      out.push(...listFiles(full));
    } else if (entry.isFile()) {
      out.push(full);
    }
  }
  return out;
}

// Comment lines are stripped before scanning code files (`.rs`, `.js`, etc.)
// so a Rust or JS file's own doc comments describing this rule cannot trip
// it, mirroring `chat_bmad_capability_separation.rs`'s technique. Prose
// files (`.md`, `.toml`, ...) are scanned verbatim: a skill's authored
// instructions have no comment syntax to strip, and a literal token there is
// exactly what this script exists to catch.
function violatingLines(path, text) {
  const isCode = [".rs", ".js", ".mjs", ".cjs", ".ts"].includes(extname(path));
  const violations = [];
  text.split(/\r?\n/u).forEach((line, index) => {
    const scanned = isCode ? line.split("//")[0] : line;
    for (const token of FORBIDDEN_TOKENS) {
      if (scanned.includes(token)) {
        violations.push({ line: index + 1, token, text: line.trim() });
      }
    }
  });
  return violations;
}

function scanDirectory(root) {
  const violations = [];
  for (const path of listFiles(root)) {
    if (!TEXT_EXTENSIONS.has(extname(path))) continue;
    const text = readFileSync(path, "utf8");
    for (const violation of violatingLines(path, text)) {
      violations.push({ path, ...violation });
    }
  }
  return violations;
}

function checkSkill(rowId, catalog) {
  const failures = [];
  const catalogRow = methodRows(catalog).find((capability) => capability.rowId === rowId);
  if (!catalogRow) {
    failures.push(`unknown skill ${rowId} (not a method capability in the catalog)`);
    return failures;
  }
  const skillSourceDir = resolve(methodSourceRoot, dirname(catalogRow.source.path));
  if (!existsSync(skillSourceDir) || !statSync(skillSourceDir).isDirectory()) {
    failures.push(`${rowId} has no pinned source directory at ${skillSourceDir}`);
    return failures;
  }
  for (const violation of scanDirectory(skillSourceDir)) {
    failures.push(
      `${rowId} source ${violation.path}:${violation.line} names forbidden capability ${violation.token}: ${violation.text}`,
    );
  }

  const adapterName = catalogRow.source.path.split("/").at(-2).replaceAll("-", "_");
  const adapterPath = resolve(adapterDir, `${adapterName}.rs`);
  if (existsSync(adapterPath)) {
    const text = readFileSync(adapterPath, "utf8");
    for (const violation of violatingLines(adapterPath, text)) {
      failures.push(
        `${rowId} adapter ${adapterPath}:${violation.line} declares a capability beyond the harness: ${violation.token}`,
      );
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
    const targets = flags.all
      ? methodRows(catalog).map((row) => row.rowId)
      : [resolveSkillSelector(flags.skill, catalog)];

    const failures = targets.flatMap((rowId) => checkSkill(rowId, catalog));
    if (failures.length > 0) fail(failures[0]);

    const summary = {
      schemaVersion: "sapphirus.bmad.authority-report/v1",
      generatedBy: "tools/bmad100/verify-authority.mjs",
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

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  process.exitCode = run();
}
