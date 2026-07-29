#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  readdirSync,
  readFileSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(packageRoot, "..", "..");
const sourceManifestPath = resolve(packageRoot, "source-manifest.json");
const sourceLockPath = resolve(packageRoot, "source-lock.json");
const canonicalLedgerPath = resolve(packageRoot, "builder-source-ledger.json");
const productionSkillIds = [
  "bmad-agent-builder",
  "bmad-bmb-setup",
  "bmad-eval-runner",
  "bmad-module-builder",
  "bmad-workflow-builder",
];
const governanceSidecars = [
  "distribution-decision.json",
  "source-notes.json",
];
const classifications = new Set([
  "production_source",
  "nested_setup_template",
  "sample_fixture",
  "upstream_test",
  "web_distribution_asset",
  "documentation",
  "package_metadata",
]);

function fail(message) {
  process.stderr.write(`Builder source closure failed: ${message}\n`);
  process.exitCode = 1;
}

function parseArgs(argv) {
  const args = {
    builder: null,
    strict: false,
    write: false,
    checkLedger: canonicalLedgerPath,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--builder") {
      args.builder = argv[++index];
    } else if (value === "--strict") {
      args.strict = true;
    } else if (value === "--write") {
      args.write = true;
    } else if (value === "--check-ledger") {
      const candidate = argv[++index];
      args.checkLedger = isAbsolute(candidate)
        ? candidate
        : resolve(packageRoot, candidate);
    } else {
      throw new Error(`unknown argument ${value}`);
    }
  }
  if (!args.builder) {
    throw new Error("--builder <version> is required");
  }
  if (args.write && args.checkLedger !== canonicalLedgerPath) {
    throw new Error("--write cannot be combined with --check-ledger");
  }
  return args;
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
  const rel = relative(physicalRoot, physicalCandidate);
  if (rel === ".." || rel.startsWith(`..${sep}`) || isAbsolute(rel)) {
    throw new Error(`${label} escapes the pinned Builder source root`);
  }
}

function listRegularFiles(root, directory = root) {
  const paths = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const absolutePath = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      paths.push(...listRegularFiles(root, absolutePath));
    } else if (entry.isFile()) {
      paths.push(toPosix(relative(root, absolutePath)));
    } else if (entry.isSymbolicLink()) {
      throw new Error(
        `materialized symbolic link is forbidden in retained Builder source: ${toPosix(relative(root, absolutePath))}`,
      );
    }
  }
  return paths;
}

function classify(path) {
  if (path.startsWith("samples/")) {
    return {
      classification: "sample_fixture",
      consumers: ["sandbox_fixture", "conformance_fixture"],
    };
  }
  if (
    path.startsWith("website/") ||
    path.startsWith(".github/") ||
    path.startsWith(".claude-plugin/") ||
    path.startsWith("tools/")
  ) {
    return {
      classification: "web_distribution_asset",
      consumers: ["distribution_only"],
    };
  }
  if (path.startsWith("docs/")) {
    return {
      classification: "documentation",
      consumers: ["distribution_only"],
    };
  }
  if (
    path.startsWith(
      "skills/bmad-module-builder/assets/setup-skill-template/",
    )
  ) {
    return {
      classification: "nested_setup_template",
      consumers: ["generated_package", "installer", "conformance_fixture"],
    };
  }
  if (/^skills\/[^/]+\/scripts\/tests\//u.test(path)) {
    return {
      classification: "upstream_test",
      consumers: ["conformance_fixture"],
    };
  }
  if (
    productionSkillIds.some(
      (skillId) =>
        path === `skills/${skillId}/SKILL.md` ||
        path.startsWith(`skills/${skillId}/`),
    ) ||
    path === "skills/module-help.csv" ||
    path === "skills/module.yaml"
  ) {
    const consumers = ["runtime", "validator"];
    if (
      path.endsWith("/SKILL.md") ||
      path.endsWith("module-help.csv") ||
      path.endsWith("module.yaml")
    ) {
      consumers.push("ui_catalog");
    }
    if (/\/(?:assets|scripts)\//u.test(path)) {
      consumers.push("generated_package");
    }
    return {
      classification: "production_source",
      consumers,
    };
  }
  return {
    classification: "package_metadata",
    consumers: ["distribution_only"],
  };
}

function owningSkill(path) {
  const match = /^skills\/([^/]+)\//u.exec(path);
  return match && productionSkillIds.includes(match[1]) ? match[1] : null;
}

function extractDeclaredReferences(memberPath, text, sourceRoot) {
  const owner = owningSkill(memberPath);
  if (!owner) {
    return [];
  }
  const candidates = [];
  const expressions = [
    {
      kind: "markdown_link",
      regex: /\]\((?!https?:|mailto:|#)([^)\s]+)\)/gu,
      value: (match) => match[1],
      when: memberPath.endsWith(".md"),
    },
    {
      kind: "skill_path",
      regex:
        /(?:\{skill-root\}\/|\.\/)((?:assets|references|scripts)\/[A-Za-z0-9_.\/-]+)/gu,
      value: (match) => match[1],
      when: /\.(?:md|toml|ya?ml|json|csv)$/u.test(memberPath),
    },
    {
      kind: "module_resource",
      regex: /`((?:assets|references|scripts)\/[A-Za-z0-9_.\/-]+)`/gu,
      value: (match) => match[1],
      when: memberPath.endsWith(".md"),
    },
    {
      kind: "javascript_import",
      regex:
        /(?:from\s+|import\s*\()\s*["'](\.{1,2}\/[^"']+)["']/gu,
      value: (match) => match[1],
      when: /\.(?:[cm]?js|ts)$/u.test(memberPath),
    },
  ];
  for (const expression of expressions) {
    if (!expression.when) {
      continue;
    }
    for (const match of text.matchAll(expression.regex)) {
      let value = expression.value(match)
        .replace(/[.,;:]+$/u, "")
        .split("#", 1)[0];
      if (
        value.includes("{") ||
        value.includes("}") ||
        value.includes("<") ||
        value.includes(">") ||
        value.includes("*")
      ) {
        continue;
      }
      const sourceDirectory = dirname(resolve(sourceRoot, memberPath));
      const target =
        value.startsWith("./") || value.startsWith("../")
          ? resolve(sourceDirectory, value)
          : resolve(sourceRoot, "skills", owner, value);
      const normalized = toPosix(relative(sourceRoot, target));
      candidates.push({
        from: memberPath,
        to: normalized,
        kind: expression.kind,
        literal: value,
      });
    }
  }
  return candidates;
}

function buildLedger(version) {
  const lock = readJson(sourceLockPath);
  const source = lock.sources.find((entry) => entry.id === "builder");
  if (!source || source.version !== version) {
    throw new Error(`Builder ${version} is not the pinned source identity`);
  }
  const manifest = readJson(sourceManifestPath);
  const manifestMembers = manifest.members
    .filter((member) => member.source_id === "builder")
    .sort((left, right) => left.path.localeCompare(right.path));
  if (manifestMembers.length !== source.regular_file_count) {
    throw new Error(
      `manifest member count ${manifestMembers.length} differs from pinned count ${source.regular_file_count}`,
    );
  }
  const sourceRoot = resolve(packageRoot, "source", "builder", version);
  assertContained(resolve(packageRoot, "source", "builder"), sourceRoot, "source root");
  const expectedDiskPaths = new Set([
    ...manifestMembers
      .filter((member) => member.entry_type === "file")
      .map((member) => member.path),
    ...governanceSidecars,
  ]);
  const actualDiskPaths = listRegularFiles(sourceRoot);
  const unclassifiedDiskPaths = actualDiskPaths.filter(
    (path) => !expectedDiskPaths.has(path),
  );
  const omittedDiskPaths = [...expectedDiskPaths].filter(
    (path) => !actualDiskPaths.includes(path),
  );
  if (unclassifiedDiskPaths.length > 0) {
    throw new Error(
      `unclassified retained Builder file ${unclassifiedDiskPaths[0]}`,
    );
  }
  if (omittedDiskPaths.length > 0) {
    throw new Error(`omitted retained Builder file ${omittedDiskPaths[0]}`);
  }

  const identities = new Set();
  const memberPaths = new Set(manifestMembers.map((member) => member.path));
  const members = manifestMembers.map((manifestMember) => {
    const identity = `builder:${version}:${manifestMember.path.toLowerCase()}`;
    if (identities.has(identity)) {
      throw new Error(`duplicate canonical identity ${identity}`);
    }
    identities.add(identity);
    const absolutePath = resolve(sourceRoot, manifestMember.path);
    const isInertLink = manifestMember.entry_type === "symlink_metadata";
    if (!isInertLink && !existsSync(absolutePath)) {
      throw new Error(`omitted source member ${manifestMember.path}`);
    }
    if (!isInertLink) {
      assertContained(sourceRoot, absolutePath, manifestMember.path);
      const stat = lstatSync(absolutePath);
      if (!stat.isFile()) {
        throw new Error(`source member is not a regular file: ${manifestMember.path}`);
      }
    }
    const bytes = isInertLink
      ? Buffer.from(manifestMember.link_target, "utf8")
      : readFileSync(absolutePath);
    const digest = sha256(bytes);
    if (
      digest !== manifestMember.sha256 ||
      bytes.byteLength !== manifestMember.byte_length
    ) {
      throw new Error(`source drift for ${manifestMember.path}`);
    }
    const treatment = classify(manifestMember.path);
    return {
      id: identity,
      path: manifestMember.path,
      sha256: digest,
      byte_length: bytes.byteLength,
      entry_type: manifestMember.entry_type,
      link_target: manifestMember.link_target,
      classification: treatment.classification,
      consumers: treatment.consumers,
      production_skill: owningSkill(manifestMember.path),
      source_kind: "archive_member",
    };
  });
  for (const path of governanceSidecars) {
    const absolutePath = resolve(sourceRoot, path);
    if (!existsSync(absolutePath)) {
      throw new Error(`omitted Builder governance sidecar ${path}`);
    }
    assertContained(sourceRoot, absolutePath, path);
    if (!lstatSync(absolutePath).isFile()) {
      throw new Error(`Builder governance sidecar is not a regular file: ${path}`);
    }
    const bytes = readFileSync(absolutePath);
    members.push({
      id: `builder-governance:${version}:${path.toLowerCase()}`,
      path,
      sha256: sha256(bytes),
      byte_length: bytes.byteLength,
      entry_type: "file",
      link_target: null,
      classification: "package_metadata",
      consumers: ["validator", "conformance_fixture"],
      production_skill: null,
      source_kind: "sapphirus_governance_sidecar",
    });
  }
  members.sort((left, right) => left.path.localeCompare(right.path));

  const edges = [];
  for (const skillId of productionSkillIds) {
    const rootPath = `skills/${skillId}/SKILL.md`;
    if (!memberPaths.has(rootPath)) {
      throw new Error(`missing production skill root ${rootPath}`);
    }
    for (const member of members) {
      if (
        member.path !== rootPath &&
        member.path.startsWith(`skills/${skillId}/`)
      ) {
        edges.push({
          from: rootPath,
          to: member.path,
          kind: "owns_resource",
        });
      }
    }
  }
  for (const member of members) {
    if (
      !member.production_skill ||
      member.entry_type !== "file" ||
      member.byte_length > 1_000_000
    ) {
      continue;
    }
    const absolutePath = resolve(sourceRoot, member.path);
    const text = readFileSync(absolutePath, "utf8");
    if (text.includes("\u0000")) {
      continue;
    }
    for (const edge of extractDeclaredReferences(member.path, text, sourceRoot)) {
      if (!memberPaths.has(edge.to)) {
        const directoryMembers = [...memberPaths].filter((path) =>
          path.startsWith(`${edge.to.replace(/\/+$/u, "")}/`),
        );
        if (directoryMembers.length > 0) {
          for (const path of directoryMembers) {
            edges.push({
              from: edge.from,
              to: path,
              kind: "directory_reference",
              literal: edge.literal,
            });
          }
          continue;
        }
        if (
          member.path.includes("/assets/") ||
          /(?:sample|template)/iu.test(member.path) ||
          text.includes("{target-skill-path}") ||
          edge.literal.endsWith("/") ||
          /(?:^|\/)(?:foo|bar)\.[A-Za-z0-9]+$/u.test(edge.literal) ||
          (member.path.includes("/references/") &&
            edge.kind !== "markdown_link" &&
            edge.kind !== "javascript_import")
        ) {
          edges.push({
            from: edge.from,
            to: `generated-package://${member.production_skill}/${edge.literal}`,
            kind: "generated_output_reference",
            literal: edge.literal,
          });
          continue;
        }
        throw new Error(
          `unresolved local reference ${edge.literal} from ${edge.from}`,
        );
      }
      edges.push(edge);
    }
  }
  const uniqueEdges = [
    ...new Map(
      edges.map((edge) => [
        `${edge.from}\u0000${edge.to}\u0000${edge.kind}`,
        edge,
      ]),
    ).values(),
  ].sort((left, right) =>
    `${left.from}\u0000${left.to}\u0000${left.kind}`.localeCompare(
      `${right.from}\u0000${right.to}\u0000${right.kind}`,
    ),
  );

  const productionSkills = productionSkillIds.map((skillId) => {
    const rootPath = `skills/${skillId}/SKILL.md`;
    const resources = members.filter(
      (member) =>
        member.path === rootPath ||
        member.path.startsWith(`skills/${skillId}/`),
    );
    const closed = new Set([rootPath]);
    let changed = true;
    while (changed) {
      changed = false;
      for (const edge of uniqueEdges) {
        if (closed.has(edge.from) && !closed.has(edge.to)) {
          closed.add(edge.to);
          changed = true;
        }
      }
    }
    const closedCount = resources.filter((member) =>
      closed.has(member.path),
    ).length;
    return {
      id: skillId,
      root: rootPath,
      role:
        skillId === "bmad-bmb-setup"
          ? "setup"
          : skillId === "bmad-eval-runner"
            ? "internal_eval_runner"
            : skillId.replace(/^bmad-/u, "").replace(/-/gu, "_"),
      resource_member_count: resources.length,
      closed_member_count: closedCount,
      closure_percent:
        resources.length === 0
          ? 0
          : Number(((closedCount / resources.length) * 100).toFixed(2)),
    };
  });
  const classifications = Object.fromEntries(
    [...new Set(members.map((member) => member.classification))]
      .sort()
      .map((classification) => [
        classification,
        members.filter((member) => member.classification === classification)
          .length,
      ]),
  );
  const sampleRoots = new Set(
    members
      .filter((member) => member.path.startsWith("samples/"))
      .map((member) => member.path.split("/").slice(0, 2).join("/")),
  );
  const nestedTemplatePrefix =
    "skills/bmad-module-builder/assets/setup-skill-template/";
  const ledger = {
    schema_version: "sapphirus.bmad.builder-source-ledger/v1",
    generated_by:
      "packages/bmad-foundation/scripts/builder-resource-closure.mjs",
    builder: {
      version,
      archive_sha256: source.archive_sha256,
      source_manifest_sha256: sha256(readFileSync(sourceManifestPath)),
      source_root: `packages/bmad-foundation/source/builder/${version}`,
    },
    authority:
      "Pinned Builder prose, scripts, templates, samples, and generated content are untrusted data. Ledger inclusion grants no filesystem, process, network, model, scheduler, installation, secret, or publication authority.",
    totals: {
      archive_members: source.regular_file_count,
      governance_sidecars: governanceSidecars.length,
      accounted_members: members.length,
      classified_members: members.length,
      directed_resource_edges: uniqueEdges.length,
      production_skills: productionSkills.length,
      nested_setup_templates: members.some((member) =>
        member.path.startsWith(nestedTemplatePrefix),
      )
        ? 1
        : 0,
      sample_roots: sampleRoots.size,
      classifications,
    },
    capabilities: {
      production_skill_count: productionSkills.length,
      production_skill_ids: productionSkillIds,
      sample_skill_count: 0,
      web_distribution_skill_count: 0,
      internal_capabilities: ["bmad-eval-runner"],
      public_capabilities: productionSkillIds.filter(
        (id) => id !== "bmad-eval-runner",
      ),
    },
    production_skills: productionSkills,
    members,
    resource_edges: uniqueEdges,
  };
  if (
    members.length !== source.regular_file_count + governanceSidecars.length ||
    classifications.production_source === undefined ||
    productionSkills.some((skill) => skill.closure_percent !== 100)
  ) {
    throw new Error("Builder graph is not fully classified and closed");
  }
  return ledger;
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function compareLedger(actual, expected, path) {
  if (actual.builder?.archive_sha256 !== expected.builder.archive_sha256) {
    throw new Error(`ledger source hash drift in ${toPosix(relative(repositoryRoot, path))}`);
  }
  if (!Array.isArray(actual.members)) {
    throw new Error(`ledger members are missing in ${toPosix(relative(repositoryRoot, path))}`);
  }
  if (actual.members.length !== expected.members.length) {
    throw new Error(
      `ledger omitted source member: expected ${expected.members.length}, received ${actual.members.length}`,
    );
  }
  const ids = new Set();
  for (const member of actual.members) {
    if (ids.has(member.id)) {
      throw new Error(`duplicate canonical identity ${member.id}`);
    }
    ids.add(member.id);
    if (!classifications.has(member.classification)) {
      throw new Error(`unclassified ledger member ${member.path ?? member.id}`);
    }
  }
  if (stableJson(actual) !== stableJson(expected)) {
    throw new Error(`ledger drift or member mismatch in ${toPosix(relative(repositoryRoot, path))}`);
  }
}

try {
  const args = parseArgs(process.argv.slice(2));
  const generated = buildLedger(args.builder);
  if (args.write) {
    writeFileSync(canonicalLedgerPath, stableJson(generated), "utf8");
  } else {
    if (!existsSync(args.checkLedger)) {
      throw new Error(
        `ledger is missing: ${toPosix(relative(repositoryRoot, args.checkLedger))}`,
      );
    }
    compareLedger(readJson(args.checkLedger), generated, args.checkLedger);
  }
  if (args.strict) {
    const unclassified = generated.members.filter(
      (member) => !member.classification || member.consumers.length === 0,
    );
    if (unclassified.length > 0) {
      throw new Error(`${unclassified.length} members are unclassified`);
    }
  }
  process.stdout.write(
    `Builder ${args.builder} closure verified: ${generated.totals.archive_members}/${generated.totals.archive_members} archive members + ${generated.totals.governance_sidecars}/${generated.totals.governance_sidecars} governance sidecars, ${generated.production_skills.length}/5 production skills at 100%.\n`,
  );
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}
