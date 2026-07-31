import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, join, normalize, relative, resolve } from "node:path";

export function parseArgs(argv) {
  const options = { positional: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (!value.startsWith("--")) {
      options.positional.push(value);
      continue;
    }
    const name = value.slice(2);
    if (["check", "offline", "archive-mode"].includes(name)) {
      options[name] = true;
      continue;
    }
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      throw new Error(`missing value for --${name}`);
    }
    options[name] = next;
    index += 1;
  }
  return options;
}

export function rootFrom(options) {
  return resolve(options.root ?? process.cwd());
}

export function readText(path) {
  return readFileSync(path, "utf8").replace(/^\uFEFF/, "");
}

export function readJson(path) {
  return JSON.parse(readText(path));
}

export function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

export function writeGenerated(path, value, check, staleMessage) {
  const content = stableJson(value);
  if (check) {
    if (!existsSync(path) || readText(path) !== content) {
      throw new Error(staleMessage);
    }
    return;
  }
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, content, "utf8");
}

export function sha256Bytes(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export function sha256File(path) {
  return sha256Bytes(readFileSync(path));
}

function scalar(value) {
  const trimmed = value.trim();
  if (trimmed === "[]") return [];
  if (trimmed === "{}") return {};
  if (trimmed === "true") return true;
  if (trimmed === "false") return false;
  if (trimmed === "null") return null;
  if (/^-?\d+$/.test(trimmed)) return Number(trimmed);
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

export function parseFrontmatter(content, path = "<document>") {
  const normalizedContent = content.replace(/\r\n/g, "\n");
  if (!normalizedContent.startsWith("---\n")) {
    throw new Error(`${path}: missing YAML frontmatter`);
  }
  const end = normalizedContent.indexOf("\n---\n", 4);
  if (end < 0) throw new Error(`${path}: unterminated YAML frontmatter`);
  const raw = normalizedContent.slice(4, end);
  const data = {};
  let activeArray = null;
  for (const line of raw.split("\n")) {
    if (!line.trim() || line.trimStart().startsWith("#")) continue;
    const arrayMatch = line.match(/^\s+-\s+(.+)$/);
    if (arrayMatch && activeArray) {
      data[activeArray].push(scalar(arrayMatch[1]));
      continue;
    }
    const field = line.match(/^([A-Za-z0-9_]+):(?:\s*(.*))?$/);
    if (!field) throw new Error(`${path}: unsupported frontmatter line: ${line}`);
    const [, key, rawValue = ""] = field;
    if (rawValue.trim() === "") {
      data[key] = [];
      activeArray = key;
    } else {
      data[key] = scalar(rawValue);
      activeArray = null;
    }
  }
  return {
    data,
    body: normalizedContent.slice(end + 5),
  };
}

export function manifestDocumentPaths(manifest) {
  return (manifest.documents ?? []).map((entry) =>
    typeof entry === "string" ? entry : entry.path,
  );
}

export function resolveInside(root, path) {
  const absolute = isAbsolute(path) ? normalize(path) : resolve(root, path);
  const rel = relative(root, absolute);
  if (rel.startsWith("..") || isAbsolute(rel)) {
    throw new Error(`path escapes repository root: ${path}`);
  }
  return absolute;
}

export function ensureExists(root, path, message = "missing path") {
  const absolute = resolveInside(root, path);
  if (!existsSync(absolute)) throw new Error(`${message}: ${path}`);
  return absolute;
}

export function isFileOrDirectory(path) {
  return existsSync(path) && (statSync(path).isFile() || statSync(path).isDirectory());
}

export function fail(error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}

export function parseTomlValue(content, section, key) {
  let inSection = section === null;
  for (const line of content.replace(/\r\n/g, "\n").split("\n")) {
    const heading = line.match(/^\s*\[([^\]]+)\]\s*$/);
    if (heading) {
      inSection = heading[1] === section;
      continue;
    }
    if (!inSection) continue;
    const entry = line.match(new RegExp(`^\\s*${key}\\s*=\\s*["']([^"']+)["']`));
    if (entry) return entry[1];
  }
  return undefined;
}

export function todayIso() {
  return new Date().toISOString().slice(0, 10);
}

export function compareIsoDate(left, right) {
  return left.localeCompare(right);
}

export function normalizedRelative(path) {
  return path.replaceAll("\\", "/");
}

export function documentRecord(root, relativePath) {
  const absolute = ensureExists(root, relativePath, "maintained document missing");
  const content = readText(absolute);
  const { data, body } = parseFrontmatter(content, relativePath);
  return { path: normalizedRelative(relativePath), absolute, content, data, body };
}

export function pathJoin(...parts) {
  return join(...parts);
}

