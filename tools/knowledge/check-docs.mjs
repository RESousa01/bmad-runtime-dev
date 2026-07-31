import { existsSync } from "node:fs";
import { dirname, extname, resolve } from "node:path";
import {
  documentRecord,
  fail,
  manifestDocumentPaths,
  parseArgs,
  readJson,
  rootFrom,
} from "./lib.mjs";

const DOC_TYPES = new Set(["current", "adr", "runbook", "plan", "review", "reference", "archive"]);
const STATUSES = new Set([
  "current",
  "accepted",
  "proposed",
  "active",
  "completed",
  "archived",
  "maintained",
  "blocked",
]);

function requiredMetadata(record) {
  const required = ["title", "doc_type", "status", "owner_role", "last_verified", "review_due"];
  for (const key of required) {
    if (record.data[key] === undefined || record.data[key] === "") {
      throw new Error(`${record.path}: missing required metadata ${key}`);
    }
  }
  if (!DOC_TYPES.has(record.data.doc_type)) {
    throw new Error(`${record.path}: invalid doc_type ${record.data.doc_type}`);
  }
  if (!STATUSES.has(record.data.status)) {
    throw new Error(`${record.path}: invalid status ${record.data.status}`);
  }
  if (["current", "review", "runbook"].includes(record.data.doc_type)) {
    if (!record.data.source_commit && !record.data.source_commit_from) {
      throw new Error(`${record.path}: source_commit or source_commit_from is required`);
    }
  }
  if (record.data.source_commit && !/^[0-9a-f]{40}$/.test(record.data.source_commit)) {
    throw new Error(`${record.path}: source_commit is not an exact commit`);
  }
  if (record.data.doc_type === "plan" && record.data.status === "current") {
    throw new Error(`${record.path}: plan cannot have status current`);
  }
}

function resolveCommitPointer(root, record) {
  if (!record.data.source_commit_from) return;
  const [jsonPath, fragment = ""] = record.data.source_commit_from.split("#", 2);
  if (!jsonPath || !fragment.startsWith("/")) {
    throw new Error(`${record.path}: source_commit_from is not a JSON Pointer`);
  }
  let value = readJson(resolve(root, jsonPath));
  for (const token of fragment
    .slice(1)
    .split("/")
    .map((part) => part.replaceAll("~1", "/").replaceAll("~0", "~"))) {
    value = value?.[token];
  }
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) {
    throw new Error(`${record.path}: source_commit_from does not resolve to an exact commit`);
  }
}

function checkLinks(record) {
  const markdown = /\[[^\]]+\]\(([^)]+)\)/g;
  for (const match of record.body.matchAll(markdown)) {
    let target = match[1].trim().replace(/^<|>$/g, "").split("#", 1)[0];
    if (!target || /^(?:https?:|mailto:)/i.test(target)) continue;
    target = decodeURIComponent(target);
    const resolved = resolve(dirname(record.absolute), target);
    if (!existsSync(resolved)) {
      throw new Error(`${record.path}: broken Markdown link ${match[1]}`);
    }
  }
  const wiki = /\[\[([^\]|#]+)(?:[|#][^\]]*)?\]\]/g;
  for (const match of record.body.matchAll(wiki)) {
    const candidate = match[1].trim();
    const local = resolve(dirname(record.absolute), extname(candidate) ? candidate : `${candidate}.md`);
    if (!existsSync(local)) {
      throw new Error(`${record.path}: broken wiki link ${candidate}`);
    }
  }
}

try {
  const options = parseArgs(process.argv.slice(2));
  const root = rootFrom(options);
  const manifest = readJson(resolve(root, "docs/_meta/knowledge-manifest.json"));
  const titles = new Map();
  for (const path of manifestDocumentPaths(manifest)) {
    const record = documentRecord(root, path);
    if (!record.body.trim()) throw new Error(`${record.path}: maintained document is empty`);
    if (/^(?:<{7}|={7}|>{7})/m.test(record.content)) {
      throw new Error(`${record.path}: merge marker found`);
    }
    const headings = record.body.match(/^#\s+.+$/gm) ?? [];
    if (headings.length !== 1) throw new Error(`${record.path}: expected exactly one H1`);
    requiredMetadata(record);
    resolveCommitPointer(root, record);
    checkLinks(record);
    for (const sourcePath of record.data.source_paths ?? []) {
      if (!existsSync(resolve(root, sourcePath))) {
        throw new Error(`${record.path}: broken source-path metadata ${sourcePath}`);
      }
    }
    for (const supersededPath of record.data.supersedes ?? []) {
      if (!existsSync(resolve(root, supersededPath))) {
        throw new Error(`${record.path}: broken supersedes path ${supersededPath}`);
      }
    }
    if (record.data.status === "current") {
      const prior = titles.get(record.data.title);
      if (prior) throw new Error(`duplicate current title: ${record.data.title}`);
      titles.set(record.data.title, record.path);
    }
  }
  for (const generated of manifest.generatedFiles ?? []) {
    if (!existsSync(resolve(root, generated))) {
      throw new Error(`generated file missing: ${generated}`);
    }
  }
  process.stdout.write(`Documentation structure verified (${manifestDocumentPaths(manifest).length} maintained documents)\n`);
} catch (error) {
  fail(error);
}
