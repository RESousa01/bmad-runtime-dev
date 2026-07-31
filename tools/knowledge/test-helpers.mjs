import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";

export function fixtureRoot(prefix) {
  return mkdtempSync(join(tmpdir(), `forge-knowledge-${prefix}-`));
}

export function write(root, relativePath, content) {
  const path = join(root, relativePath);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, content, "utf8");
  return path;
}

export function writeJson(root, relativePath, value) {
  return write(root, relativePath, `${JSON.stringify(value, null, 2)}\n`);
}

export function runScript(scriptName, args = [], cwd = process.cwd()) {
  return spawnSync(process.execPath, [join(process.cwd(), "tools", "knowledge", scriptName), ...args], {
    cwd,
    encoding: "utf8",
  });
}

export function maintainedDoc({
  title = "Fixture",
  docType = "current",
  status = "current",
  body = "Fixture body.",
  extra = "",
} = {}) {
  return `---
title: "${title}"
doc_type: ${docType}
status: ${status}
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2099-01-01
source_commit: "1111111111111111111111111111111111111111"
source_paths: []
supersedes: []
${extra}---

# ${title}

${body}
`;
}

