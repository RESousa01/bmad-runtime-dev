import { lstat, readFile, readdir } from "node:fs/promises";
import { extname, join, relative } from "node:path";
import process from "node:process";

const root = process.cwd();
const maximumFileBytes = 8 * 1024 * 1024;
const scanRoots = [
  ".github",
  "apps",
  "crates",
  "docs",
  "packages",
  "tools",
];
const rootFiles = [
  ".editorconfig",
  ".gitattributes",
  ".gitignore",
  ".npmrc",
  ".node-version",
  ".nvmrc",
  "Cargo.lock",
  "Cargo.toml",
  "README.md",
  "deny.toml",
  "package.json",
  "pnpm-lock.yaml",
  "pnpm-workspace.yaml",
  "rust-toolchain.toml",
  "rustfmt.toml",
  "tsconfig.base.json",
];
const ignoredDirectories = new Set([
  ".git",
  ".agents",
  ".codex",
  "bmad-runtime-lib",
  "coverage",
  "dist",
  "node_modules",
  "target",
]);
const textExtensions = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".jsx",
  ".key",
  ".md",
  ".mjs",
  ".pem",
  ".properties",
  ".ps1",
  ".py",
  ".rs",
  ".sh",
  ".toml",
  ".ts",
  ".tsx",
  ".txt",
  ".yaml",
  ".yml",
]);
const textFileNames = new Set([
  "Containerfile",
  "Dockerfile",
  "Makefile",
]);
const detectors = [
  ["private key", /-----BEGIN (?:[A-Z0-9]+ )?PRIVATE KEY-----/g],
  ["AWS access key", /\b(?:AKIA|ASIA)[A-Z0-9]{16}\b/g],
  ["GitHub token", /\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b/g],
  ["GitLab token", /\bglpat-[A-Za-z0-9_-]{20,}\b/g],
  ["Slack token", /\bxox[baprs]-[A-Za-z0-9-]{20,}\b/g],
  ["Stripe live key", /\b(?:sk|rk)_live_[A-Za-z0-9]{16,}\b/g],
  ["Azure storage account key", /\bAccountKey=[A-Za-z0-9+/]{40,}={0,2}\b/g],
];
const assignmentDetector =
  /\b(?:api[_-]?key|client[_-]?secret|password|refresh[_-]?token|access[_-]?token|id[_-]?token|account[_-]?key|private[_-]?key|signing[_-]?key|sas[_-]?token|connection[_-]?string)\b["']?\s*[:=]\s*(?:"([^"\r\n]{8,})"|'([^'\r\n]{8,})'|([^\s#,;\]}]{8,}))/giu;

function isAllowedNonSecret(value) {
  const normalized = value.trim();
  return /^(?:example|fixture|placeholder|redacted|replace-me|test-only)$/iu.test(normalized)
    || /^must-not-cross(?:[-_][a-z0-9]+)*$/iu.test(normalized)
    || /^<\s*(?:redacted|replace[-_ ]?me|secret)\s*>$/iu.test(normalized)
    || /^\$\{[A-Za-z_][A-Za-z0-9_]*\}$/u.test(normalized)
    || /^(?:process\.env\.|import\.meta\.env\.)[A-Z][A-Z0-9_]*$/u.test(normalized);
}

function isTextCandidate(name) {
  const lowerName = name.toLowerCase();
  return lowerName === ".env"
    || lowerName.startsWith(".env.")
    || textFileNames.has(name)
    || textExtensions.has(extname(lowerName));
}

function lineAt(source, offset) {
  return source.slice(0, offset).split(/\r\n|\r|\n/u).length;
}

const findings = [];

async function walk(directory) {
  const files = [];
  let entries;
  try {
    const metadata = await lstat(directory);
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
      findings.push({ label: "scan root is not a regular directory", path: directory, line: 1 });
      return files;
    }
    entries = await readdir(directory, { withFileTypes: true });
  } catch {
    findings.push({ label: "scan root is missing or unreadable", path: directory, line: 1 });
    return files;
  }
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      findings.push({ label: "linked source entry is not scanned", path, line: 1 });
    } else if (entry.isDirectory() && ignoredDirectories.has(entry.name)) {
      continue;
    } else if (entry.isDirectory()) {
      files.push(...(await walk(path)));
    } else if (entry.isFile() && isTextCandidate(entry.name)) {
      files.push(path);
    }
  }
  return files;
}

const paths = new Set(rootFiles.map((file) => join(root, file)));
for (const directory of scanRoots) {
  for (const path of await walk(join(root, directory))) paths.add(path);
}

let scannedFileCount = 0;
for (const path of [...paths].sort()) {
  let metadata;
  try {
    metadata = await lstat(path);
  } catch {
    findings.push({ label: "required source file is missing or unreadable", path, line: 1 });
    continue;
  }
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    findings.push({ label: "source path is not a regular file", path, line: 1 });
    continue;
  }
  if (metadata.size > maximumFileBytes) {
    findings.push({ label: "text source exceeds scan size limit", path, line: 1 });
    continue;
  }
  let bytes;
  try {
    bytes = await readFile(path);
  } catch {
    findings.push({ label: "source file became unreadable", path, line: 1 });
    continue;
  }
  scannedFileCount += 1;
  let source;
  try {
    source = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    findings.push({ label: "invalid UTF-8 in text source", path, line: 1 });
    continue;
  }
  if (source.includes("\0")) {
    findings.push({ label: "NUL byte in text source", path, line: lineAt(source, source.indexOf("\0")) });
    continue;
  }
  for (const [label, pattern] of detectors) {
    pattern.lastIndex = 0;
    for (const match of source.matchAll(pattern)) {
      findings.push({ label, path, line: lineAt(source, match.index ?? 0) });
    }
  }
  assignmentDetector.lastIndex = 0;
  for (const match of source.matchAll(assignmentDetector)) {
    const value = match[1] ?? match[2] ?? match[3] ?? "";
    if (!isAllowedNonSecret(value)) {
      findings.push({
        label: "literal secret assignment",
        path,
        line: lineAt(source, match.index ?? 0),
      });
    }
  }
}

if (findings.length > 0) {
  console.error("Potential plaintext secrets found:");
  for (const finding of findings) {
    const displayPath = relative(root, finding.path);
    console.error(`- ${displayPath}:${finding.line}: ${finding.label}`);
  }
  process.exit(1);
}

console.log(`Secret scan passed for ${scannedFileCount} active first-party source files.`);
