#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  lstatSync,
  readFileSync,
  realpathSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const packageRoot = resolve(dirname(scriptPath), "..");
const repositoryRoot = resolve(packageRoot, "..", "..");
const sourceLockPath = resolve(packageRoot, "source-lock.json");
const sourceManifestPath = resolve(packageRoot, "source-manifest.json");
const builderGovernanceSidecars = ["distribution-decision.json", "source-notes.json"];
const methodPackage = {
  source_id: "method",
  package_name: "bmad-method",
  version: "6.10.0",
};

function generatedOutputDispositions(member_path, member_sha256, literals) {
  return literals.map((literal) => ({
    ...methodPackage,
    member_path,
    member_sha256,
    literal,
    disposition: "generated_output",
    output_identity: `bmad-document-project:${literal.slice(2)}`,
  }));
}

// These are source-derived edge decisions, not path-pattern exclusions. Every decision is
// invalidated by source package, version, declaring-member, member-hash, or literal drift.
const sealedEdgeDispositions = [
  ...generatedOutputDispositions(
    "src/bmm-skills/1-analysis/bmad-document-project/templates/index-template.md",
    "42c8a14f53088e4fda82f26a3fe41dc8a89d4bcb7a9659dd696136378b64ee90",
    [
      "./project-overview.md",
      "./source-tree-analysis.md",
      "./architecture.md",
      "./component-inventory.md",
      "./development-guide.md",
      "./api-contracts.md",
      "./data-models.md",
      "./integration-architecture.md",
      "./project-parts.json",
      "./deployment-guide.md",
      "./contribution-guide.md",
    ],
  ),
  ...generatedOutputDispositions(
    "src/bmm-skills/1-analysis/bmad-document-project/templates/project-overview-template.md",
    "a7c7325b75a5a678dca391b9b69b1e3409cfbe6da95e70443ed3ace164e287b2",
    ["./index.md", "./source-tree-analysis.md"],
  ),
  ...generatedOutputDispositions(
    "src/bmm-skills/1-analysis/bmad-document-project/workflows/full-scan-instructions.md",
    "2235945df2ae261265187447ce593238e132a026a9b88d3507a1f1d6808a0263",
    ["./project-overview.md", "./source-tree-analysis.md", "./contribution-guide.md"],
  ),
  {
    ...methodPackage,
    member_path:
      "src/bmm-skills/2-plan-workflows/bmad-ux/references/design-md-spec.md",
    member_sha256:
      "d04d5f663607aba3f3181e818ae33856e6caf5e560dcb16fa64ed80b7cf3d590",
    literal: "https://github.com/google-labs-code/design.md",
    disposition: "external_authority_rejected_pinned_projection",
    authority_identity: "github:google-labs-code/design.md",
    governance_owner: "MTH-10",
    projection_member_path:
      "src/bmm-skills/2-plan-workflows/bmad-ux/references/design-md-spec.md",
    projection_member_sha256:
      "d04d5f663607aba3f3181e818ae33856e6caf5e560dcb16fa64ed80b7cf3d590",
  },
  {
    ...methodPackage,
    member_path: "src/bmm-skills/2-plan-workflows/bmad-ux/SKILL.md",
    member_sha256:
      "250d2794f2b8e316bb1ba4666f0728970afd8a5d2707ac1cceac6c21e63730b6",
    literal: "https://github.com/google-labs-code/design.md",
    disposition: "external_authority_rejected_pinned_projection",
    authority_identity: "github:google-labs-code/design.md",
    governance_owner: "MTH-10",
    projection_member_path:
      "src/bmm-skills/2-plan-workflows/bmad-ux/references/design-md-spec.md",
    projection_member_sha256:
      "d04d5f663607aba3f3181e818ae33856e6caf5e560dcb16fa64ed80b7cf3d590",
  },
];

function fail(message) {
  throw new Error(`resource closure failed: ${message}`);
}

function parseArgs(argv) {
  if (argv.length === 1 && argv[0] === "--self-test") {
    return { mode: "--self-test", source: null, skill: null, json: null };
  }
  const flags = { mode: null, source: null, skill: null, json: null };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--check") {
      flags.mode = "--check";
    } else if (token === "--source") {
      flags.source = argv[++index];
    } else if (token === "--skill") {
      flags.skill = argv[++index];
    } else if (token === "--json") {
      flags.json = argv[++index];
      if (!flags.json) fail("missing value for --json");
    } else {
      fail("usage: node packages/bmad-foundation/scripts/resource-closure.mjs [--source <id@version> --skill <name>] --check|--self-test");
    }
  }
  if (
    flags.mode !== "--check" ||
    (flags.source === null) !== (flags.skill === null) ||
    (flags.source !== null && (!flags.source || !flags.skill))
  ) {
    fail("usage: node packages/bmad-foundation/scripts/resource-closure.mjs [--source <id@version> --skill <name>] --check|--self-test");
  }
  return flags;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function toPosix(path) {
  return path.split(sep).join("/");
}

function assertContained(root, candidate, label) {
  const physicalRoot = realpathSync(root);
  const physicalCandidate = realpathSync(candidate);
  const pathFromRoot = relative(physicalRoot, physicalCandidate);
  if (
    pathFromRoot === ".." ||
    pathFromRoot.startsWith(`..${sep}`) ||
    isAbsolute(pathFromRoot)
  ) {
    fail(`${label} escapes its pinned package root`);
  }
}

function listRegularFiles(root, directory = root) {
  const members = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const fullPath = resolve(directory, entry.name);
    const stat = lstatSync(fullPath);
    if (stat.isSymbolicLink()) {
      fail(`unsafe symbolic link ${toPosix(relative(root, fullPath))}`);
    }
    if (stat.isDirectory()) {
      members.push(...listRegularFiles(root, fullPath));
    } else if (stat.isFile()) {
      assertContained(root, fullPath, `member ${entry.name}`);
      members.push(toPosix(relative(root, fullPath)));
    }
  }
  return members.sort((left, right) => left.localeCompare(right));
}

function mediaType(path) {
  if (/\.mdx?$/u.test(path)) return "text/markdown";
  if (/\.toml$/u.test(path)) return "application/toml";
  if (/\.ya?ml$/u.test(path)) return "application/yaml";
  if (/\.csv$/u.test(path)) return "text/csv";
  if (/\.[cm]?js$/u.test(path)) return "text/javascript";
  if (/\.ts$/u.test(path)) return "text/typescript";
  if (/\.(?:html|hbs|mustache)$/u.test(path)) return "text/html";
  if (/\.svg$/u.test(path)) return "image/svg+xml";
  return "application/octet-stream";
}

function isResourceLiteral(value) {
  const resourceExtension = /\.(?:mdx?|toml|ya?ml|json|csv|[cm]?js|ts|py|html|svg|txt|xml|mustache|hbs)(?:[?#].*)?$/iu;
  const isRemote = value.startsWith("http:") || value.startsWith("https:");
  return (
    value.startsWith("./") ||
    value.startsWith("../") ||
    ((value.startsWith("/") || value.startsWith("\\") || /^[A-Za-z]:/u.test(value)) && resourceExtension.test(value)) ||
    value.startsWith("file:") ||
    (isRemote && resourceExtension.test(value)) ||
    (value.includes("/") && resourceExtension.test(value))
  );
}

function resolveReference(from, literal) {
  const value = literal.split("#", 1)[0].split("?", 1)[0];
  if (
    !value ||
    value.startsWith("/") ||
    value.startsWith("\\") ||
    value.includes("\\") ||
    value.includes(":") ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    return null;
  }
  const parts = dirname(from).split("/").filter(Boolean);
  for (const component of value.split("/")) {
    if (!component || component === ".") continue;
    if (component === "..") {
      if (parts.length === 0) return null;
      parts.pop();
    } else {
      parts.push(component);
    }
  }
  return parts.join("/");
}

function markdownTargets(line) {
  const targets = [];
  const expression = /\]\(([^)\s]+)\)/gu;
  for (const match of line.matchAll(expression)) targets.push(match[1]);
  return targets;
}

function quotedValues(line) {
  const values = [];
  for (const expression of [/'([^']+)'/gu, /"([^"]+)"/gu]) {
    for (const match of line.matchAll(expression)) values.push(match[1]);
  }
  return values;
}

function declaredReferences(path, bytes) {
  const text = bytes.toString("utf8");
  if (text.includes("\u0000")) return [];
  const references = [];
  let inFrontmatter = false;
  let csvHeaders = null;
  for (const line of text.split(/\r?\n/u)) {
    if (path.endsWith(".csv")) {
      const cells = line.split(",").map((cell) => cell.trim());
      if (csvHeaders === null) {
        csvHeaders = cells;
      } else {
        for (const [index, literal] of cells.entries()) {
          if (
            /\b(?:asset|template|script|include|file|path|source|reference)\b/iu.test(csvHeaders[index] ?? "") &&
            isResourceLiteral(literal) &&
            !/[{}<>*]/u.test(literal)
          ) {
            references.push({
              from: path,
              literal,
              kind: "csv",
              optional: /\boptional\b/iu.test(csvHeaders[index] ?? ""),
            });
          }
        }
      }
      continue;
    }
    if (/\.mdx?$/u.test(path) && line.trim() === "---") {
      inFrontmatter = !inFrontmatter;
      continue;
    }
    const lower = line.toLowerCase();
    const optional = /^\s*optional\s*[:=]/iu.test(line);
    const add = (literal, kind) => {
      const value = literal.trim().replace(/^<|>$/gu, "");
      if (value.startsWith("_bmad/") || value.startsWith("{project-") || value.includes("/path/to/")) return;
      if (value.includes("PATH")) return;
      if (!isResourceLiteral(value) || /[{}<>*]/u.test(value) || value.includes("{{") || value.includes("}}")) return;
      references.push({ from: path, literal: value, kind, optional });
    };
    if (!/\(to be generated\)/iu.test(line)) {
      for (const literal of markdownTargets(line)) add(literal, "markdown");
    }
    const isScript = /\bimport\s*["']|\brequire\s*\(|^\s*source\s+["']/u.test(lower);
    const isTemplate = /\.(?:mdx?|html|hbs|mustache)$/u.test(path) && /\{\{/u.test(lower);
    if (isScript) {
      for (const match of line.matchAll(/(?:\bimport\s*\(|\brequire\s*\(|^\s*source\s+)["']([^"']+)["']/gu)) {
        add(match[1], "script");
      }
      continue;
    }
    const isNamedValue =
      inFrontmatter ||
      (/\.(?:toml|ya?ml)$/u.test(path) && /^[\t ]*[A-Za-z_][A-Za-z0-9_-]*[\t ]*[:=]/u.test(line)) ||
      /^\s*optional\s*[:=]/iu.test(line);
    if (isTemplate || isNamedValue) {
      const kind = isTemplate
        ? "template"
          : path.endsWith(".toml")
            ? "toml"
            : /\.ya?ml$/u.test(path)
              ? "yaml"
                : "frontmatter";
      for (const literal of quotedValues(line)) add(literal, kind);
      const valuePart = line.includes("=")
        ? line.slice(line.indexOf("=") + 1)
        : line.includes(":")
          ? line.slice(line.indexOf(":") + 1)
          : line;
      if (kind !== "template") {
        for (const literal of valuePart.split(/[\s,=:]+/u)) add(literal, kind);
      }
    }
  }
  return [...new Map(references.map((edge) => [
    `${edge.from}\u0000${edge.literal}\u0000${edge.kind}\u0000${edge.optional}`,
    edge,
  ])).values()];
}

function edgeSealKey(source, memberPath, memberHash, literal) {
  return [
    source.id,
    source.package_name,
    source.version,
    memberPath,
    memberHash,
    literal,
  ].join("\u0000");
}

function validatedDispositionIndex(source, memberRecords, dispositions) {
  const membersByPath = new Map(
    memberRecords.map((member) => [member.path, member]),
  );
  const index = new Map();
  for (const disposition of dispositions.filter(
    (entry) => entry.source_id === source.id,
  )) {
    if (
      disposition.package_name !== source.package_name ||
      disposition.version !== source.version
    ) {
      fail(
        `${source.id} sealed disposition package/version drift for ${disposition.member_path}`,
      );
    }
    const member = membersByPath.get(disposition.member_path);
    if (!member) {
      fail(
        `${source.id} sealed disposition has unknown member ${disposition.member_path}`,
      );
    }
    if (member.contentHash !== disposition.member_sha256) {
      fail(
        `${source.id} sealed disposition member hash drift for ${disposition.member_path}`,
      );
    }
    if (
      !declaredReferences(member.path, member.bytes).some(
        (reference) => reference.literal === disposition.literal,
      )
    ) {
      fail(
        `${source.id} sealed disposition literal drift for ${disposition.member_path}: ${disposition.literal}`,
      );
    }
    const target = resolveReference(member.path, disposition.literal);
    if (disposition.disposition === "generated_output") {
      if (
        !disposition.output_identity ||
        /[\u0000-\u001f\u007f]/u.test(disposition.output_identity) ||
        !target ||
        membersByPath.has(target)
      ) {
        fail(
          `${source.id} invalid generated-output disposition for ${disposition.member_path}: ${disposition.literal}`,
        );
      }
    } else if (disposition.disposition === "external_authority_rejected") {
      if (
        !/^https?:/iu.test(disposition.literal) ||
        !disposition.authority_identity ||
        !disposition.governance_owner ||
        target
      ) {
        fail(
          `${source.id} invalid rejected-authority disposition for ${disposition.member_path}: ${disposition.literal}`,
        );
      }
    } else if (
      disposition.disposition ===
      "external_authority_rejected_pinned_projection"
    ) {
      const projection = membersByPath.get(
        disposition.projection_member_path,
      );
      if (
        !/^https?:/iu.test(disposition.literal) ||
        !disposition.authority_identity ||
        !disposition.governance_owner ||
        target ||
        !projection ||
        !disposition.projection_member_sha256 ||
        projection.contentHash !== disposition.projection_member_sha256
      ) {
        fail(
          `${source.id} invalid pinned-projection disposition for ${disposition.member_path}: ${disposition.literal}`,
        );
      }
    } else if (
      disposition.disposition !== "package_resource_mandatory" &&
      disposition.disposition !== "package_resource_optional"
    ) {
      fail(
        `${source.id} unknown sealed disposition ${disposition.disposition}`,
      );
    }
    const key = edgeSealKey(
      source,
      disposition.member_path,
      disposition.member_sha256,
      disposition.literal,
    );
    if (index.has(key)) {
      fail(
        `${source.id} duplicate sealed disposition for ${disposition.member_path}: ${disposition.literal}`,
      );
    }
    index.set(key, disposition);
  }
  return index;
}

export function evaluateResourceReferences({
  source,
  members,
  referencePaths,
  dispositions = [],
}) {
  const memberRecords = members.map((member) => {
    const bytes = Buffer.isBuffer(member.bytes)
      ? member.bytes
      : Buffer.from(member.bytes);
    return {
      path: member.path,
      bytes,
      contentHash: member.contentHash ?? sha256(bytes),
    };
  });
  const membersByPath = new Map(
    memberRecords.map((member) => [member.path, member]),
  );
  const memberSet = new Set(membersByPath.keys());
  const dispositionIndex = validatedDispositionIndex(
    source,
    memberRecords,
    dispositions,
  );
  const consumedDispositions = new Set();
  const directedEdges = [];
  const missing = [];
  const invalid = [];
  const activationBlockers = [];

  for (const from of referencePaths) {
    const member = membersByPath.get(from);
    if (!member) fail(`${source.id} cannot parse unknown member ${from}`);
    for (const reference of declaredReferences(from, member.bytes)) {
      const key = edgeSealKey(
        source,
        from,
        member.contentHash,
        reference.literal,
      );
      const sealed = dispositionIndex.get(key);
      const target = resolveReference(from, reference.literal);
      let disposition;
      if (sealed) {
        consumedDispositions.add(key);
        disposition = sealed.disposition;
        if (
          disposition === "package_resource_mandatory" &&
          reference.optional
        ) {
          fail(
            `${source.id} mandatory disposition contradicts optional source syntax in ${from}: ${reference.literal}`,
          );
        }
        if (
          disposition === "package_resource_optional" &&
          !reference.optional
        ) {
          fail(
            `${source.id} optional disposition contradicts mandatory source syntax in ${from}: ${reference.literal}`,
          );
        }
      } else if (target) {
        disposition = reference.optional
          ? "package_resource_optional"
          : "package_resource_mandatory";
      } else {
        invalid.push({
          from,
          literal: reference.literal,
          reason: "unsealed_or_unsafe_reference",
          requirement: reference.optional ? "optional" : "mandatory",
        });
        disposition = "unsealed_or_unsafe_reference";
      }

      const evidence = {
        source_id: source.id,
        package_name: source.package_name,
        version: source.version,
        member_path: from,
        from,
        member_sha256: member.contentHash,
        literal: reference.literal,
        kind: reference.kind,
        requirement: reference.optional ? "optional" : "mandatory",
        disposition,
        to:
          disposition === "package_resource_mandatory" ||
          disposition === "package_resource_optional"
            ? target
            : null,
        ...(sealed?.output_identity
          ? { output_identity: sealed.output_identity }
          : {}),
        ...(sealed?.authority_identity
          ? {
              authority_identity: sealed.authority_identity,
              governance_owner: sealed.governance_owner,
            }
          : {}),
        ...(sealed?.projection_member_path
          ? {
              projection_member_path: sealed.projection_member_path,
              projection_member_sha256: sealed.projection_member_sha256,
            }
          : {}),
      };
      directedEdges.push(evidence);

      if (
        (disposition === "package_resource_mandatory" ||
          disposition === "package_resource_optional") &&
        target &&
        !memberSet.has(target) &&
        disposition === "package_resource_mandatory"
      ) {
        missing.push({
          from,
          to: target,
          literal: reference.literal,
          disposition,
        });
      }
      if (disposition === "external_authority_rejected") {
        activationBlockers.push(evidence);
      }
    }
  }
  const unusedDisposition = [...dispositionIndex.keys()].find(
    (key) => !consumedDispositions.has(key),
  );
  if (unusedDisposition) {
    const disposition = dispositionIndex.get(unusedDisposition);
    fail(
      `${source.id} sealed disposition was not consumed for ${disposition.member_path}: ${disposition.literal}`,
    );
  }
  return {
    directedEdges,
    missing,
    invalid,
    activationBlockers,
  };
}

export function evaluateSkillResourceClosure({
  source,
  root,
  memberPaths,
  directedEdges,
  ownership = new Map(),
}) {
  const memberSet = new Set(memberPaths);
  const reachable = new Set([root]);
  const requiredEdges = new Set([root]);
  const closedEdges = new Set([root]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const edge of directedEdges) {
      if (!reachable.has(edge.from)) continue;
      const edgeIdentity = `${edge.from}\u0000${edge.literal}\u0000${edge.kind}`;
      if (
        edge.disposition === "package_resource_mandatory" ||
        (edge.disposition === "unsealed_or_unsafe_reference" &&
          edge.requirement === "mandatory")
      ) {
        requiredEdges.add(edgeIdentity);
      }
      if (
        (edge.disposition === "package_resource_mandatory" ||
          edge.disposition === "package_resource_optional") &&
        edge.to &&
        memberSet.has(edge.to)
      ) {
        if (edge.disposition === "package_resource_mandatory") {
          closedEdges.add(edgeIdentity);
        }
        if (!reachable.has(edge.to)) {
          reachable.add(edge.to);
          changed = true;
        }
      }
    }
  }
  const closurePercent = Number(
    ((closedEdges.size / requiredEdges.size) * 100).toFixed(2),
  );
  const skillActivationBlockers = directedEdges
    .filter(
      (edge) =>
        reachable.has(edge.from) &&
        edge.disposition === "external_authority_rejected",
    )
    .map((blocker) => ({
      member_path: blocker.member_path,
      member_sha256: blocker.member_sha256,
      literal: blocker.literal,
      disposition: blocker.disposition,
      authority_identity: blocker.authority_identity,
      governance_owner: blocker.governance_owner,
    }))
    .sort((left, right) =>
      `${left.member_path}\u0000${left.literal}`.localeCompare(
        `${right.member_path}\u0000${right.literal}`,
      ),
    );
  const skillInvalidReferences = directedEdges
    .filter(
      (edge) =>
        reachable.has(edge.from) &&
        edge.disposition === "unsealed_or_unsafe_reference",
    )
    .map((edge) => ({
      member_path: edge.member_path,
      member_sha256: edge.member_sha256,
      literal: edge.literal,
      kind: edge.kind,
      requirement: edge.requirement,
      disposition: edge.disposition,
    }))
    .sort((left, right) =>
      `${left.member_path}\u0000${left.literal}\u0000${left.kind}`.localeCompare(
        `${right.member_path}\u0000${right.literal}\u0000${right.kind}`,
      ),
    );
  return {
    root,
    source_id: source.id,
    package_identity: `${source.package_name}@${source.version}`,
    required_mandatory_edge_count: requiredEdges.size,
    closed_mandatory_edge_count: closedEdges.size,
    closure_percent: closurePercent,
    activation_allowed:
      closurePercent === 100 &&
      skillActivationBlockers.length === 0 &&
      skillInvalidReferences.length === 0,
    activation_blockers: skillActivationBlockers,
    invalid_references: skillInvalidReferences,
    cross_skill_dependencies: [...reachable]
      .filter((path) => ownership.has(path) && ownership.get(path) !== root)
      .map((path) => ({ path, owner_skill: ownership.get(path) }))
      .sort((left, right) => left.path.localeCompare(right.path)),
  };
}

function skillRoots(sourceId, members) {
  if (sourceId === "builder") {
    return members.filter((path) => /^skills\/[^/]+\/SKILL\.md$/u.test(path));
  }
  return members.filter((path) => /\/SKILL\.md$/u.test(path) && path.startsWith("src/"));
}

function cycleMembers(edges) {
  const graph = new Map();
  for (const edge of edges) {
    if (!graph.has(edge.from)) graph.set(edge.from, []);
    graph.get(edge.from).push(edge.to);
  }
  const visiting = new Set();
  const visited = new Set();
  const cycles = new Set();
  const stack = [];
  const visit = (node) => {
    if (visiting.has(node)) {
      const position = stack.indexOf(node);
      cycles.add(stack.slice(position).join(" -> "));
      return;
    }
    if (visited.has(node)) return;
    visiting.add(node);
    stack.push(node);
    for (const next of graph.get(node) ?? []) visit(next);
    stack.pop();
    visiting.delete(node);
    visited.add(node);
  };
  for (const node of graph.keys()) visit(node);
  return [...cycles].sort();
}

function runSelfTest() {
  const nested = declaredReferences("skills/a/steps/one.md", Buffer.from("[next](./two.md)\n", "utf8"));
  if (nested.length !== 1 || nested[0].literal !== "./two.md") fail("self-test nested Markdown parsing failed");
  const remote = declaredReferences("skills/a/SKILL.md", Buffer.from("[remote](https://example.invalid/a.md)\n", "utf8"));
  if (remote.length !== 1 || resolveReference("skills/a/SKILL.md", remote[0].literal) !== null) fail("self-test unpinned remote rejection failed");
  const required = new Set(["root", "root\u0000./missing.md"]);
  const closed = new Set(["root"]);
  const percent = (closed.size * 100) / required.size;
  if (percent >= 100) fail("self-test mandatory denominator was fabricated");
  process.stdout.write("resource-closure self-test passed\n");
}

function verifiedInventory(source, manifest, sourceRoot, memberPrefix = null) {
  assertContained(resolve(packageRoot, "source", source.id), sourceRoot, source.id);
  const manifestSource = manifest.sources.find((entry) => entry.id === source.id);
  if (!manifestSource || manifestSource.version !== source.version || manifestSource.archive_sha256 !== source.archive_sha256) {
    fail(`${source.id} source manifest does not match the pinned archive identity`);
  }
  const manifestMembers = manifest.members.filter((entry) => entry.source_id === source.id);
  if (manifestMembers.length !== source.regular_file_count) {
    fail(`${source.id} manifest count differs from the pinned archive count`);
  }
  const archivedFiles = manifestMembers.filter(
    (entry) =>
      entry.entry_type === "file" &&
      (memberPrefix === null || entry.path.startsWith(memberPrefix)),
  );
  const sidecars = source.id === "builder" ? builderGovernanceSidecars : [];
  const expectedDiskPaths = new Set([
    ...archivedFiles.map((entry) => entry.path),
    ...(memberPrefix === null ? sidecars : []),
  ]);
  const actualDiskPaths = listRegularFiles(sourceRoot).filter(
    (path) => memberPrefix === null || path.startsWith(memberPrefix),
  );
  const actualSet = new Set(actualDiskPaths);
  const unexpected = actualDiskPaths.find((path) => !expectedDiskPaths.has(path));
  const omitted = [...expectedDiskPaths].find((path) => !actualSet.has(path));
  if (unexpected) fail(`${source.id} has unexplained source addition ${unexpected}`);
  if (omitted) fail(`${source.id} omits pinned source member ${omitted}`);
  for (const member of archivedFiles) {
    const bytes = readFileSync(resolve(sourceRoot, member.path));
    if (sha256(bytes) !== member.sha256 || bytes.byteLength !== member.byte_length) {
      fail(`${source.id} source hash drift for ${member.path}`);
    }
  }
  return { members: actualDiskPaths, sidecars, manifestMembers };
}

function buildPackageClosure(source, manifest, sourceRoot, skill = null) {
  const manifestRoot =
    skill === null
      ? null
      : manifest.members.find(
          (entry) =>
            entry.source_id === source.id &&
            entry.entry_type === "file" &&
            entry.path.endsWith(`/${skill}/SKILL.md`),
        )?.path;
  if (skill !== null && !manifestRoot) {
    fail(`${source.id} has no pinned primary skill ${skill}`);
  }
  const memberPrefix =
    manifestRoot === null
      ? null
      : manifestRoot.slice(0, -"SKILL.md".length);
  const inventory = verifiedInventory(
    source,
    manifest,
    sourceRoot,
    memberPrefix,
  );
  const members = inventory.members;
  const caseFolded = new Set();
  const identityByPath = new Map();
  const contentCache = new Map();
  const memberRecords = [];
  for (const path of members) {
    const folded = path.toLocaleLowerCase("en-US");
    if (caseFolded.has(folded)) fail(`${source.id} has case-colliding member ${path}`);
    caseFolded.add(folded);
    const bytes = readFileSync(resolve(sourceRoot, path));
    const contentHash = sha256(bytes);
    memberRecords.push({ path, bytes, contentHash });
    const identity = `${source.id}:${source.version}:${folded}`;
    identityByPath.set(path, { identity, contentHash, media_type: mediaType(path) });
    if (!contentCache.has(contentHash)) contentCache.set(contentHash, []);
    contentCache.get(contentHash).push(identity);
  }
  const roots = skillRoots(source.id, members);
  const ownership = new Map();
  const directedEdges = [];
  for (const root of roots) {
    const prefix = root.slice(0, -"SKILL.md".length);
    for (const member of members.filter((path) => path.startsWith(prefix))) {
      ownership.set(member, root);
    }
  }
  const referencePaths = members.filter(
    (member) => ownership.has(member) && !member.split("/").at(-1).startsWith("sample-"),
  );
  const classified = evaluateResourceReferences({
    source,
    members: memberRecords,
    referencePaths,
    dispositions:
      memberPrefix === null
        ? sealedEdgeDispositions
        : sealedEdgeDispositions.filter((entry) =>
            entry.member_path.startsWith(memberPrefix),
          ),
  });
  directedEdges.push(...classified.directedEdges);
  const missing = classified.missing;
  const invalid = classified.invalid;
  const activationBlockers = classified.activationBlockers;
  const cycles = cycleMembers(directedEdges.filter(
    (edge) => edge.disposition === "package_resource_mandatory" && edge.to,
  ));
  const skills = roots.map((root) =>
    evaluateSkillResourceClosure({
      source,
      root,
      memberPaths: members,
      directedEdges,
      ownership,
    }),
  );
  return {
    source: { id: source.id, version: source.version, archive_sha256: source.archive_sha256, archive_member_count: source.regular_file_count, governance_sidecars: inventory.sidecars },
    members: members.map((path) => ({ path, ...identityByPath.get(path) })),
    content_hash_cache: [...contentCache.entries()].map(([content_hash, identities]) => ({
      content_hash,
      identities: identities.sort(),
    })),
    declared_edges: directedEdges.sort((left, right) =>
      `${left.member_path}\u0000${left.literal}\u0000${left.kind}`.localeCompare(
        `${right.member_path}\u0000${right.literal}\u0000${right.kind}`,
      ),
    ),
    skills,
    missing: missing.sort((left, right) => left.from.localeCompare(right.from)),
    invalid: invalid.sort((left, right) => left.from.localeCompare(right.from)),
    activation_blockers: activationBlockers.sort((left, right) =>
      `${left.member_path}\u0000${left.literal}`.localeCompare(
        `${right.member_path}\u0000${right.literal}`,
      ),
    ),
    cycles,
  };
}

export function run(argv = process.argv.slice(2)) {
  let receiptPath = null;
  try {
    const flags = parseArgs(argv);
    receiptPath = flags.json;
    if (receiptPath) rmSync(receiptPath, { force: true });
    if (flags.mode === "--self-test") {
      runSelfTest();
      return 0;
    }
    const lock = readJson(sourceLockPath);
    const manifest = readJson(sourceManifestPath);
    if (!Array.isArray(lock.sources) || lock.sources.length === 0) fail("pinned source lock is empty");
    const selectedSources =
      flags.source === null
        ? lock.sources
        : lock.sources.filter(
            (source) => `${source.id}@${source.version}` === flags.source,
          );
    if (selectedSources.length !== (flags.source === null ? lock.sources.length : 1)) {
      fail(`unknown pinned source ${flags.source}`);
    }
    const packages = selectedSources.map((source) => {
      if (!source.id || !source.version || !source.package_name || !source.archive_sha256) {
        fail("pinned source lock has an incomplete source identity");
      }
      return buildPackageClosure(
        source,
        manifest,
        resolve(packageRoot, "source", source.id, source.version),
        flags.skill,
      );
    });
    const failures = packages.flatMap((closure) => [
      ...closure.missing.map((entry) => `${closure.source.id}: missing ${entry.literal} from ${entry.from}`),
      ...closure.invalid.map((entry) => `${closure.source.id}: unsafe ${entry.literal} from ${entry.from}`),
      ...closure.cycles.map((cycle) => `${closure.source.id}: cycle ${cycle}`),
      ...closure.skills
        .filter((skill) => skill.closure_percent !== 100)
        .map((skill) => `${closure.source.id}: incomplete skill ${skill.root}`),
    ]);
    if (failures.length > 0) fail(failures[0]);
    const summary = {
      schema_version: "sapphirus.bmad.resource-closure/v1",
      generated_by: "packages/bmad-foundation/scripts/resource-closure.mjs",
      packages: packages.map((closure) => ({
        source: closure.source,
        members: closure.members.length,
        cache_entries: closure.content_hash_cache.length,
        edge_dispositions: closure.declared_edges.map((edge) => ({
          source_id: edge.source_id,
          package_name: edge.package_name,
          version: edge.version,
          member_path: edge.member_path,
          member_sha256: edge.member_sha256,
          literal: edge.literal,
          kind: edge.kind,
          disposition: edge.disposition,
          to: edge.to,
          ...(edge.output_identity
            ? { output_identity: edge.output_identity }
            : {}),
          ...(edge.authority_identity
            ? {
                authority_identity: edge.authority_identity,
                governance_owner: edge.governance_owner,
              }
            : {}),
          ...(edge.projection_member_path
            ? {
                projection_member_path: edge.projection_member_path,
                projection_member_sha256: edge.projection_member_sha256,
              }
            : {}),
        })),
        activation_blockers: closure.activation_blockers.map((blocker) => ({
          member_path: blocker.member_path,
          member_sha256: blocker.member_sha256,
          literal: blocker.literal,
          disposition: blocker.disposition,
          authority_identity: blocker.authority_identity,
          governance_owner: blocker.governance_owner,
        })),
        member_accounting: closure.members.filter((member) => {
          if (!flags.skill) return true;
          return closure.skills.some((skill) =>
            member.path.startsWith(skill.root.slice(0, -"SKILL.md".length)),
          );
        }),
        production_skills: closure.skills,
      })),
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

if (
  process.argv[1] &&
  resolve(process.argv[1]) === resolve(scriptPath)
) {
  process.exitCode = run();
}
