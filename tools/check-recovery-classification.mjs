#!/usr/bin/env node
/**
 * Fails when the recovery classification is re-implemented outside its owner.
 *
 * `recovery_required` and `integrity_failure` are the host saying its own state
 * is not trustworthy. Every other host error is an ordinary failure with a
 * message for the user. Getting that distinction wrong is silent in the worst
 * direction: a host demanding recovery reported as a generic error leaves the
 * app running, offering actions that cannot work, against a host that has told
 * it to stop.
 *
 * There were seven copies of the check before FE-10 — the workspace controller,
 * the changes runner, the guidance request, the projection poller, the changes
 * failure handler, the BMAD recovery effect, and the model path. Seven copies
 * is seven chances to disagree, and they only have to disagree once. This is
 * how the count stays at one.
 *
 * What counts as a re-implementation
 * ----------------------------------
 * A *branch* on both codes, not a mention of them. `contracts.ts` declares the
 * union of every host code and `workspaceProtocol.ts` parses it; both name
 * these two legitimately and neither classifies anything. So the pattern looks
 * for a comparison — `code === "recovery_required"` — rather than the string.
 *
 * Usage:
 *   node tools/check-recovery-classification.mjs [--json] [--root <dir>]
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_ROOT = "apps/desktop-ui/src";
/** The one module allowed to make the comparison. */
const OWNER = "app/hostFailure.ts";

/**
 * A decision made on one of the two codes.
 *
 * A branch, not a mention. A union type listing every host code, or a parser
 * validating against all of them, does not match — those describe the codes rather than
 * deciding anything with them.
 */
const CODE = String.raw`"(?:recovery_required|integrity_failure)"`;

/**
 * Two line-level forms, because both are natural ways to write the same branch
 * and the first version of this checker caught only the comparison:
 *
 *   `code === "recovery_required"`   a comparison
 *   `case "recovery_required":`      a switch arm
 */
const CLASSIFICATION_PATTERNS = [
  new RegExp(String.raw`[=!]==\s*${CODE}`, "u"),
  new RegExp(String.raw`\bcase\s+${CODE}\s*:`, "u"),
];

/**
 * A list holding *only* these two codes.
 *
 * That is a classifier however it is consulted -- `.includes(code)`,
 * `new Set([...]).has(code)`, `.some(...)`. A list carrying them among the
 * other host codes is a description and does not match. Applied to the whole
 * file rather than a line, because such a list wraps.
 */
const CODE_PAIR_LIST = new RegExp(
  String.raw`\[\s*${CODE}\s*,\s*${CODE}\s*,?\s*\]`,
  "u",
);

function listSourceFiles(dir) {
  const found = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      if (entry === "node_modules" || entry === "dist") continue;
      found.push(...listSourceFiles(full));
      continue;
    }
    if (!/\.tsx?$/u.test(entry)) continue;
    // Tests may construct these codes to prove the classifier works.
    if (/\.test\.tsx?$/u.test(entry)) continue;
    found.push(full);
  }
  return found;
}

function offendingLines(file) {
  const source = readFileSync(file, "utf8");
  const found = source
    .split(/\r?\n/u)
    .map((line, index) => ({ line: index + 1, text: line.trim() }))
    .filter(({ text }) => CLASSIFICATION_PATTERNS.some((p) => p.test(text)));

  const pair = CODE_PAIR_LIST.exec(source);
  if (pair !== null) {
    const line = source.slice(0, pair.index).split(/\r?\n/u).length;
    if (!found.some((entry) => entry.line === line)) {
      found.push({ line, text: pair[0].replace(/\s+/gu, " ") });
    }
  }
  return found.sort((a, b) => a.line - b.line);
}

function main() {
  const argv = process.argv.slice(2);
  const asJson = argv.includes("--json");
  const rootIndex = argv.indexOf("--root");
  const root = rootIndex === -1 ? DEFAULT_ROOT : argv[rootIndex + 1];
  const full = resolve(REPO_ROOT, root ?? DEFAULT_ROOT);

  let files;
  try {
    files = listSourceFiles(full);
  } catch {
    process.stdout.write(`No sources under ${root}; nothing to check.\n`);
    return;
  }

  const offenders = [];
  for (const file of files) {
    const relativePath = relative(REPO_ROOT, file).replaceAll("\\", "/");
    if (relativePath.endsWith(OWNER)) continue;
    const lines = offendingLines(file);
    if (lines.length > 0) offenders.push({ file: relativePath, lines });
  }

  if (asJson) {
    process.stdout.write(`${JSON.stringify({ offenders, scanned: files.length }, null, 2)}\n`);
  } else if (offenders.length === 0) {
    process.stdout.write(
      `Recovery classification has one definition (${files.length} modules scanned).\n`,
    );
  } else {
    process.stdout.write(`Recovery classification re-implemented in ${offenders.length} file(s):\n`);
    for (const { file, lines } of offenders) {
      for (const { line, text } of lines) {
        process.stdout.write(`\n  ${file}:${line}\n    ${text}\n`);
      }
    }
    process.stdout.write(
      `\nUse \`isRecoveryFailure\` from ${OWNER}. A second copy is a second chance\n`
      + "to disagree, and disagreeing here leaves the app running against a host\n"
      + "that has told it to stop.\n",
    );
  }
  process.exitCode = offenders.length === 0 ? 0 : 1;
}

main();
