/**
 * Checks that CI only calls things that exist, and that a Windows release is
 * proven on Windows.
 *
 * Backlog item QR-03 states these as its acceptance criteria. They are not
 * covered by check-workflows.test.mjs, which checks CI *hygiene* -- pinned
 * action SHAs, job timeouts, concurrency, credential persistence -- and never
 * asks whether the commands a workflow runs are real.
 *
 * That gap has teeth. A renamed package script or a moved tool keeps every
 * local gate green and only fails on the runner, minutes into a job, usually
 * on someone else's branch.
 */

import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname.slice(1)), "..");
const workflowsDir = path.join(root, ".github", "workflows");

const readJson = (relative) => JSON.parse(readFileSync(path.join(root, relative), "utf8"));
const rootManifest = readJson("package.json");

/** Workspace package name -> its scripts, so `--filter` targets can be checked. */
function workspaceScripts() {
  const byName = new Map();
  const globs = ["apps", "packages", "crates", "services", "tools"];
  for (const dir of globs) {
    const full = path.join(root, dir);
    if (!existsSync(full)) continue;
    for (const entry of readdirSync(full)) {
      const manifestPath = path.join(full, entry, "package.json");
      if (!existsSync(manifestPath)) continue;
      const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
      if (manifest.name) byName.set(manifest.name, manifest.scripts ?? {});
    }
  }
  return byName;
}

const packages = workspaceScripts();

/** Every `run:` line in a workflow, flattened out of block scalars. */
function runCommands(source) {
  const lines = source.split(/\r?\n/u);
  const commands = [];
  for (let i = 0; i < lines.length; i += 1) {
    // `[ \t]+` rather than `\s*`: with `\s*` the engine backtracks to zero
    // width, the lookahead then inspects the space instead of the `|`, and a
    // block scalar is captured as the one-character command "|" -- silently
    // skipping every line inside it. Requiring at least one space removes the
    // backtrack, so `run: |` can only reach the block branch below.
    const inline = /^\s*-?\s*run:[ \t]+(?![|>])(.+?)\s*$/u.exec(lines[i]);
    if (inline) {
      commands.push(inline[1]);
      continue;
    }
    if (!/^\s*-?\s*run:[ \t]*[|>][+-]?\s*$/u.test(lines[i])) continue;
    const indent = (/^(\s*)/u.exec(lines[i + 1] ?? "") ?? ["", ""])[1].length;
    for (let j = i + 1; j < lines.length; j += 1) {
      const line = lines[j];
      if (line.trim() === "") continue;
      if ((/^(\s*)/u.exec(line) ?? ["", ""])[1].length < indent) break;
      commands.push(line.trim());
    }
  }
  return commands;
}

// pnpm's own verbs, not package scripts.
const PNPM_BUILTINS = new Set([
  "install", "exec", "run", "dlx", "add", "remove", "store", "why",
  "recursive", "list", "outdated", "audit", "publish", "pack", "test",
]);

const problems = [];

const workflows = readdirSync(workflowsDir)
  .filter((name) => name.endsWith(".yml") || name.endsWith(".yaml"))
  .map((name) => ({ name, source: readFileSync(path.join(workflowsDir, name), "utf8") }));

for (const { name, source } of workflows) {
  for (const command of runCommands(source)) {
    // `node <path>`, including `node --test <path>`.
    for (const match of command.matchAll(/\bnode\s+(?:--[\w-]+\s+)*([\w./-]+\.m?js)\b/gu)) {
      const target = match[1];
      if (target.startsWith("$") || target.includes("*")) continue;
      if (!existsSync(path.join(root, target))) {
        problems.push(`${name}: runs \`node ${target}\`, which does not exist`);
      }
    }

    // `pnpm --filter <pkg> <script>`
    const filtered = /\bpnpm\s+--filter\s+(\S+)\s+(?:run\s+)?([\w:-]+)/u.exec(command);
    if (filtered) {
      const [, pkg, script] = filtered;
      if (PNPM_BUILTINS.has(script)) continue;
      const scripts = packages.get(pkg);
      if (!scripts) {
        problems.push(`${name}: filters on \`${pkg}\`, which is not a workspace package`);
      } else if (!(script in scripts)) {
        problems.push(`${name}: runs \`${script}\` in \`${pkg}\`, which has no such script`);
      }
      continue;
    }

    // `pnpm <script>` against the root manifest.
    const plain = /\bpnpm\s+(?:run\s+)?([\w:-]+)/u.exec(command);
    if (plain) {
      const script = plain[1];
      if (PNPM_BUILTINS.has(script)) continue;
      if (!(script in (rootManifest.scripts ?? {}))) {
        problems.push(`${name}: runs \`pnpm ${script}\`, which is not a root script`);
      }
    }
  }
}

// QR-03's second criterion. Anything that produces or signs a Windows artifact
// has to have been proven on a Windows runner; a Linux job cannot stand in for
// it, and saying so in prose is what this replaces.
const WINDOWS_EVIDENCE = /windows|win-x64|\.msi|\.exe|nsis/iu;
for (const { name, source } of workflows) {
  const producesWindows = /desktop:build|build-signed-windows-installer|tauri build/u.test(source);
  if (!producesWindows) continue;
  if (!/runs-on:\s*\S*windows\S*/iu.test(source) && !/runs-on:\s*\[[^\]]*windows/iu.test(source)) {
    problems.push(
      `${name}: produces a Windows desktop artifact but no job declares a Windows runner`,
    );
  }
  if (!WINDOWS_EVIDENCE.test(source)) {
    problems.push(`${name}: produces a Windows artifact without naming the evidence it keeps`);
  }
}

if (problems.length > 0) {
  console.error("\nWorkflow checks failed:\n");
  for (const problem of problems) console.error(`  - ${problem}`);
  console.error("");
}

assert.equal(problems.length, 0, "CI must only call commands and paths that exist.");

console.log(
  `Workflow check passed: ${workflows.length} workflows, every pnpm script and node path resolves.`,
);
