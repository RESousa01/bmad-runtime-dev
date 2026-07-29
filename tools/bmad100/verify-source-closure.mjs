#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, isAbsolute, join, posix, relative, resolve, sep } from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";

const repositoryRoot = resolve(import.meta.dirname, "../..");
const defaultFoundationRoot = resolve(repositoryRoot, "packages/bmad-foundation");
const knownTreatments = new Set([
  "adopt",
  "adapt",
  "shim",
  "fixture",
  "distribution",
  "reject_nonruntime",
]);
const builderGovernanceSidecars = new Set([
  "distribution-decision.json",
  "source-notes.json",
]);

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const args = {
    strict: false,
    foundationRoot: defaultFoundationRoot,
    sourceManifest: null,
    sourceAccounting: null,
    builderLedger: null,
    catalog: null,
    sourceOverlay: null,
    methodArchive: null,
    builderArchive: null,
    generatedOutputInventory: null,
    manifestDir: resolve(repositoryRoot, "tools/bmad100/manifests"),
    json: resolve(repositoryRoot, "artifacts/bmad100/source-closure.json"),
    report: resolve(repositoryRoot, "artifacts/bmad100/quality/qal-01/report.json"),
  };
  const pathFlags = new Map([
    ["--foundation-root", "foundationRoot"],
    ["--source-manifest", "sourceManifest"],
    ["--source-accounting", "sourceAccounting"],
    ["--builder-ledger", "builderLedger"],
    ["--catalog", "catalog"],
    ["--source-overlay", "sourceOverlay"],
    ["--method-archive", "methodArchive"],
    ["--builder-archive", "builderArchive"],
    ["--generated-output-inventory", "generatedOutputInventory"],
    ["--manifest-dir", "manifestDir"],
    ["--json", "json"],
    ["--report", "report"],
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--strict") {
      args.strict = true;
      continue;
    }
    const field = pathFlags.get(token);
    if (!field) fail(`unknown argument ${token}`);
    const value = argv[++index];
    if (!value) fail(`${token} requires a path`);
    args[field] = resolve(value);
  }
  if (!args.strict) fail("source closure verification requires --strict");
  args.sourceManifest ??= resolve(args.foundationRoot, "source-manifest.json");
  args.sourceAccounting ??= resolve(args.foundationRoot, "source-accounting.json");
  args.builderLedger ??= resolve(args.foundationRoot, "builder-source-ledger.json");
  args.catalog ??= resolve(args.foundationRoot, "capability-catalog.json");
  args.generatedOutputInventory ??= resolve(
    repositoryRoot,
    "tools/bmad100/manifests/generated-output-targets.json",
  );
  return args;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function parseCsv(text) {
  const rows = [];
  let row = [];
  let value = "";
  let quoted = false;
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (quoted) {
      if (character === '"' && text[index + 1] === '"') {
        value += '"';
        index += 1;
      } else if (character === '"') {
        quoted = false;
      } else {
        value += character;
      }
    } else if (character === '"') {
      quoted = true;
    } else if (character === ",") {
      row.push(value);
      value = "";
    } else if (character === "\n") {
      row.push(value.replace(/\r$/u, ""));
      rows.push(row);
      row = [];
      value = "";
    } else {
      value += character;
    }
  }
  if (value.length > 0 || row.length > 0) {
    row.push(value.replace(/\r$/u, ""));
    rows.push(row);
  }
  const [headers, ...records] = rows;
  return records
    .filter((record) => record.some((field) => field.length > 0))
    .map((record) =>
      Object.fromEntries(headers.map((header, index) => [header, record[index] ?? ""])),
    );
}

function readJson(path, label = path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    fail(`${label} is missing or invalid: ${error.message}`);
  }
}

function run(command, commandArgs, options = {}) {
  const result = spawnSync(command, commandArgs, {
    cwd: repositoryRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    ...options,
  });
  if (result.status !== 0) {
    fail(
      `${command} ${commandArgs.join(" ")} failed (${result.status}): ${String(result.stderr).trim()}`,
    );
  }
  return result.stdout;
}

function resolveArchivePath(args, source) {
  const explicit =
    source.id === "method" ? args.methodArchive : args.builderArchive;
  const environment =
    source.id === "method"
      ? process.env.BMAD_METHOD_ARCHIVE
      : process.env.BMAD_BUILDER_ARCHIVE;
  const path =
    explicit ??
    environment ??
    resolve(homedir(), "Downloads", source.acquired_filename);
  if (!existsSync(path)) {
    fail(
      `${source.id} pinned archive is unavailable at ${path}; set ${
        source.id === "method" ? "BMAD_METHOD_ARCHIVE" : "BMAD_BUILDER_ARCHIVE"
      }`,
    );
  }
  return realpathSync(path);
}

function archiveSymlinks(archivePath, archiveRoot) {
  const verbose = run("tar", ["-tvf", archivePath]);
  const links = new Map();
  for (const line of verbose.split(/\r?\n/u)) {
    if (!line.startsWith("l")) continue;
    const marker = `${archiveRoot}/`;
    const start = line.indexOf(marker);
    const arrow = line.lastIndexOf(" -> ");
    if (start < 0 || arrow < 0) fail(`cannot parse archived symbolic link: ${line}`);
    links.set(line.slice(start, arrow), line.slice(arrow + 4));
  }
  return links;
}

function sourceCategories(sourceId, path) {
  const categories = [];
  const methodPrimary =
    sourceId === "method" &&
    /^src\/(?:bmm-skills|core-skills)\/.+\/SKILL\.md$/u.test(path);
  const builderPrimary =
    sourceId === "builder" && /^skills\/[^/]+\/SKILL\.md$/u.test(path);
  if (methodPrimary || builderPrimary) categories.push("primary_skill");
  if (path.includes("/v6-shims/")) categories.push("compatibility_shim");
  if (path.startsWith("samples/")) categories.push("sample_fixture");
  if (
    /(?:^|\/)(?:test|tests|fixtures?|evals)(?:\/|$)/iu.test(path) ||
    path.startsWith("test/")
  ) {
    categories.push("test_fixture");
  }
  if (
    /^(?:\.augment|\.claude-plugin|\.github|website|web-bundles)\//u.test(path)
  ) {
    categories.push("distribution_asset");
  }
  if (path.startsWith("docs/") || /(?:^|\/)README(?:_[A-Z]+)?\.md$/u.test(path)) {
    categories.push("documentation");
  }
  if (
    /(?:^|\/)(?:scripts?|tools)(?:\/|$)/iu.test(path) ||
    /\.(?:[cm]?js|ts|py|ps1|sh)$/iu.test(path)
  ) {
    categories.push("script");
  }
  if (
    /(?:^|\/)(?:templates?|assets?)(?:\/|$)/iu.test(path) ||
    /template/iu.test(posix.basename(path))
  ) {
    categories.push("template_or_asset");
  }
  if (
    path.startsWith("skills/bmad-module-builder/assets/setup-skill-template/")
  ) {
    categories.push(
      path.endsWith("/SKILL.md") ? "nested_setup_skill" : "nested_setup_resource",
    );
  }
  if (path.endsWith("module-help.csv")) categories.push("module_help");
  if (categories.length === 0) categories.push("source_resource");
  return categories;
}

function loadArchive(args, source) {
  const path = resolveArchivePath(args, source);
  const bytes = readFileSync(path);
  if (bytes.byteLength !== source.archive_byte_length) {
    fail(`${source.id} archive byte length drift`);
  }
  if (sha256(bytes) !== source.archive_sha256) {
    fail(`${source.id} archive SHA-256 drift`);
  }
  const entries = run("tar", ["-tf", path])
    .split(/\r?\n/u)
    .filter(Boolean);
  const members = entries.filter((entry) => !entry.endsWith("/"));
  if (members.length !== source.regular_file_count) {
    fail(
      `${source.id} archive member count drift: expected ${source.regular_file_count}, received ${members.length}`,
    );
  }
  const prefix = `${source.archive_root}/`;
  if (members.some((entry) => !entry.startsWith(prefix))) {
    fail(`${source.id} archive contains a member outside ${source.archive_root}`);
  }
  const symlinks = archiveSymlinks(path, source.archive_root);
  const temporaryRoot = mkdtempSync(join(tmpdir(), `bmad-${source.id}-archive-`));
  try {
    const extractArgs = ["-xf", path];
    for (const linkPath of symlinks.keys()) {
      extractArgs.push(`--exclude=${linkPath}`);
    }
    extractArgs.push("-C", temporaryRoot);
    run("tar", extractArgs);
    const archiveMembers = [];
    const contents = new Map();
    for (const archiveMember of members) {
      const memberPath = archiveMember.slice(prefix.length);
      if (
        memberPath.length === 0 ||
        memberPath.startsWith("../") ||
        posix.isAbsolute(memberPath)
      ) {
        fail(`${source.id} archive member escapes its root: ${archiveMember}`);
      }
      const linkTarget = symlinks.get(archiveMember) ?? null;
      const memberBytes =
        linkTarget === null
          ? readFileSync(resolve(temporaryRoot, archiveMember))
          : Buffer.from(linkTarget, "utf8");
      contents.set(memberPath, memberBytes);
      archiveMembers.push({
        path: memberPath,
        sha256: sha256(memberBytes),
        byte_length: memberBytes.byteLength,
        entry_type: linkTarget === null ? "file" : "symlink_metadata",
        link_target: linkTarget,
        categories: sourceCategories(source.id, memberPath),
      });
    }
    archiveMembers.sort((left, right) => left.path.localeCompare(right.path));
    const packageJson = JSON.parse(contents.get("package.json").toString("utf8"));
    if (
      packageJson.name !== source.package_name ||
      packageJson.version !== source.version ||
      packageJson.engines?.node !== source.runtime.node
    ) {
      fail(`${source.id} archived package identity/version/engine drift`);
    }
    const skillCount = archiveMembers.filter((member) =>
      member.path.endsWith("/SKILL.md"),
    ).length;
    if (skillCount !== source.skill_md_count) {
      fail(`${source.id} archived SKILL.md count drift`);
    }
    return {
      path,
      sha256: sha256(bytes),
      byte_length: bytes.byteLength,
      members: archiveMembers,
      contents,
      package: {
        name: packageJson.name,
        version: packageJson.version,
        node_engine: packageJson.engines.node,
      },
    };
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}

function requireUnique(items, key, label) {
  const values = items.map(key);
  const unique = new Set(values);
  if (unique.size !== values.length) {
    fail(`${label} contains a duplicate identity`);
  }
  return unique;
}

function countBy(items, key) {
  const counts = {};
  for (const item of items) {
    const value = key(item);
    counts[value] = (counts[value] ?? 0) + 1;
  }
  return Object.fromEntries(Object.entries(counts).sort(([left], [right]) => left.localeCompare(right)));
}

function toPosix(path) {
  return path.split(sep).join("/");
}

function listFiles(root, directory = root) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const absolute = resolve(directory, entry.name);
    const path = toPosix(relative(root, absolute));
    if (entry.isDirectory()) {
      files.push(...listFiles(root, absolute));
    } else if (entry.isFile()) {
      files.push(path);
    } else {
      fail(`retained source contains non-regular filesystem entry ${path}`);
    }
  }
  return files.sort();
}

function assertContained(root, candidate, label) {
  const physicalRoot = realpathSync(root);
  const physicalCandidate = realpathSync(candidate);
  const rel = relative(physicalRoot, physicalCandidate);
  if (rel === ".." || rel.startsWith(`..${sep}`) || isAbsolute(rel)) {
    fail(`${label} escapes its pinned source root`);
  }
}

function sourceBytes(args, source, member) {
  if (member.entry_type === "symlink_metadata") {
    return Buffer.from(member.link_target, "utf8");
  }
  const canonical = resolve(
    args.foundationRoot,
    "source",
    source.id,
    source.version,
    ...member.path.split("/"),
  );
  const overlay = args.sourceOverlay
    ? resolve(args.sourceOverlay, source.id, ...member.path.split("/"))
    : null;
  const path = overlay && existsSync(overlay) ? overlay : canonical;
  if (!existsSync(path)) fail(`missing retained source member ${source.id}:${member.path}`);
  assertContained(
    overlay && existsSync(overlay)
      ? resolve(args.sourceOverlay, source.id)
      : resolve(args.foundationRoot, "source", source.id, source.version),
    path,
    `${source.id}:${member.path}`,
  );
  if (!lstatSync(path).isFile()) fail(`source member is not a regular file ${source.id}:${member.path}`);
  return readFileSync(path);
}

function packageIdentity(args, source) {
  const path = resolve(
    args.foundationRoot,
    "source",
    source.id,
    source.version,
    "package.json",
  );
  const packageJson = readJson(path, `${source.id} package.json`);
  return {
    name: packageJson.name,
    version: packageJson.version,
    node_engine: packageJson.engines?.node ?? null,
  };
}

function discoveryFor(sourceId, members, actionDiscovery) {
  const primary = members.filter((member) =>
    member.categories.includes("primary_skill"),
  );
  return {
    primary_skills: primary.length,
    shims: primary.filter((member) =>
      member.categories.includes("compatibility_shim"),
    ).length,
    fixtures: members.filter((member) =>
      member.categories.some((category) =>
        ["sample_fixture", "test_fixture"].includes(category),
      ),
    ).length,
    samples: members.filter((member) =>
      member.categories.includes("sample_fixture"),
    ).length,
    web_or_distribution: members.filter((member) =>
      member.categories.includes("distribution_asset"),
    ).length,
    scripts: members.filter((member) => member.categories.includes("script")).length,
    templates: members.filter((member) =>
      member.categories.includes("template_or_asset"),
    ).length,
    nested_setup_skills: members.filter((member) =>
      member.categories.includes("nested_setup_skill"),
    ).length,
    public_actions: sourceId === "builder" ? actionDiscovery.public.length : 0,
    internal_actions: sourceId === "builder" ? actionDiscovery.internal.length : 0,
  };
}

function generatedManifest(source, archive, accountingByKey, actionDiscovery) {
  return {
    schema_version: "sapphirus.bmad100.pinned-source-manifest/v1",
    source: {
      id: source.id,
      version: source.version,
      archive_root: source.archive_root,
      archive_sha256: source.archive_sha256,
      regular_file_count: source.regular_file_count,
      skill_md_count: source.skill_md_count,
    },
    archive: {
      acquired_filename: source.acquired_filename,
      byte_length: archive.byte_length,
      sha256: archive.sha256,
    },
    discovery: discoveryFor(source.id, archive.members, actionDiscovery),
    categories: countBy(
      archive.members.flatMap((member) =>
        member.categories.map((category) => ({ category })),
      ),
      (row) => row.category,
    ),
    members: archive.members.map((member) => {
      const accounting = accountingByKey.get(`${source.id}:${member.path}`);
      return {
        path: member.path,
        sha256: member.sha256,
        byte_length: member.byte_length,
        categories: member.categories,
        entry_type: member.entry_type,
        link_target: member.link_target,
        treatment: accounting?.treatment ?? null,
        owner: accounting?.owner ?? null,
        runtime_denominator: accounting?.runtime_denominator ?? false,
        target_capability: accounting?.target_capability ?? null,
      };
    }),
  };
}

function verifyDiskClosure(args, source, members) {
  const root = resolve(args.foundationRoot, "source", source.id, source.version);
  if (!existsSync(root)) fail(`missing retained ${source.id} source root`);
  const expected = new Set(
    members
      .filter((member) => member.entry_type === "file")
      .map((member) => member.path),
  );
  if (source.id === "builder") {
    for (const sidecar of builderGovernanceSidecars) expected.add(sidecar);
  }
  const actual = listFiles(root);
  const extra = actual.filter((path) => !expected.has(path));
  const missing = [...expected].filter((path) => !actual.includes(path));
  if (extra.length > 0) fail(`unclassified retained ${source.id} file ${extra[0]}`);
  if (missing.length > 0) fail(`missing retained ${source.id} file ${missing[0]}`);
}

function verifySourceMembers(args, source, members) {
  if (members.length !== source.regular_file_count) {
    fail(
      `${source.id} source denominator count drift: expected ${source.regular_file_count}, received ${members.length}`,
    );
  }
  requireUnique(members, (member) => member.path.toLowerCase(), `${source.id} members`);
  let skillCount = 0;
  for (const member of members) {
    if (!member.class) fail(`unclassified source member ${source.id}:${member.path}`);
    const bytes = sourceBytes(args, source, member);
    if (bytes.byteLength !== member.byte_length) {
      fail(`source byte length drift ${source.id}:${member.path}`);
    }
    if (sha256(bytes) !== member.sha256) {
      fail(`source hash drift ${source.id}:${member.path}`);
    }
    if (member.path.endsWith("/SKILL.md")) skillCount += 1;
  }
  if (skillCount !== source.skill_md_count) {
    fail(`${source.id} SKILL.md denominator drift: expected ${source.skill_md_count}, received ${skillCount}`);
  }
  verifyDiskClosure(args, source, members);
}

function verifyArchiveManifestClosure(source, archive, manifestMembers) {
  const archiveByPath = new Map(archive.members.map((member) => [member.path, member]));
  const manifestByPath = new Map(manifestMembers.map((member) => [member.path, member]));
  requireUnique(
    archive.members,
    (member) => member.path.toLowerCase(),
    `${source.id} archive members`,
  );
  if (archive.members.length !== manifestMembers.length) {
    fail(
      `${source.id} archive/import denominator mismatch: archive=${archive.members.length} manifest=${manifestMembers.length}`,
    );
  }
  for (const member of archive.members) {
    const imported = manifestByPath.get(member.path);
    if (!imported) fail(`${source.id} archive member is missing from import ${member.path}`);
    if (
      imported.sha256 !== member.sha256 ||
      imported.byte_length !== member.byte_length ||
      imported.entry_type !== member.entry_type ||
      imported.link_target !== member.link_target
    ) {
      fail(`${source.id} archive/import member drift ${member.path}`);
    }
  }
  for (const member of manifestMembers) {
    if (!archiveByPath.has(member.path)) {
      fail(`${source.id} imported member is absent from archive ${member.path}`);
    }
  }
  const primary = archive.members.filter((member) =>
    member.categories.includes("primary_skill"),
  );
  if (primary.length !== source.primary_skill_count) {
    fail(
      `${source.id} structurally discovered primary skill count drift: expected ${source.primary_skill_count}, received ${primary.length}`,
    );
  }
  return primary;
}

function verifyAccounting(sourceManifest, accounting) {
  if (accounting.members.length !== sourceManifest.members.length) {
    fail(
      `source accounting member count drift: expected ${sourceManifest.members.length}, received ${accounting.members.length}`,
    );
  }
  const manifestByKey = new Map(
    sourceManifest.members.map((member) => [`${member.source_id}:${member.path}`, member]),
  );
  requireUnique(accounting.members, (member) => member.member_key, "source accounting members");
  for (const row of accounting.members) {
    const source = manifestByKey.get(row.member_key);
    if (!source) fail(`source accounting has an extra member ${row.member_key}`);
    if (source.sha256 !== row.sha256 || source.class !== row.class || source.entry_type !== row.entry_type) {
      fail(`source accounting drift for ${row.member_key}`);
    }
    if (!knownTreatments.has(row.treatment)) fail(`unclassified treatment for ${row.member_key}`);
    if (!row.owner || !row.retained_purpose) fail(`source member lacks a reviewed consumer or exclusion ${row.member_key}`);
    if (
      row.runtime_denominator &&
      !["primary_method_skill", "primary_builder_skill"].includes(row.class)
    ) {
      fail(`false production-counted source row ${row.member_key}`);
    }
    if (row.runtime_denominator && (!row.target_capability || !row.implementation_task)) {
      fail(`production source row is not connected to runtime ownership ${row.member_key}`);
    }
    if (!row.runtime_denominator && !row.verification_use) {
      fail(`non-production source row lacks reviewed verification use ${row.member_key}`);
    }
  }
  const primary = accounting.members.filter((member) => member.runtime_denominator);
  const methodPrimary = primary.filter((member) => member.class === "primary_method_skill").length;
  const builderPrimary = primary.filter((member) => member.class === "primary_builder_skill").length;
  if (methodPrimary !== 48 || builderPrimary !== 5) {
    fail(`primary source denominator drift method=${methodPrimary} builder=${builderPrimary}`);
  }
  const publicActions = accounting.builder_actions.filter((action) => action.visibility === "public");
  const internalActions = accounting.builder_actions.filter((action) => action.visibility === "internal");
  if (publicActions.length !== 9 || internalActions.length !== 1) {
    fail(`Builder action count drift public=${publicActions.length} internal=${internalActions.length}`);
  }
  requireUnique(accounting.builder_actions, (action) => action.id, "Builder actions");
  for (const action of accounting.builder_actions) {
    if (!action.owner || !action.treatment || !action.source_skill || !action.action) {
      fail(`Builder action is unclassified ${action.id ?? "<missing-id>"}`);
    }
  }
  return { methodPrimary, builderPrimary, publicActions, internalActions };
}

function discoverBuilderActions(archive, accounting) {
  const helpBytes = archive.contents.get("skills/module-help.csv");
  if (!helpBytes) fail("Builder archive omits skills/module-help.csv");
  const rows = parseCsv(helpBytes.toString("utf8")).filter(
    (row) => row.skill && row.skill !== "_meta" && row.action,
  );
  if (rows.length !== 9) {
    fail(`Builder module-help public action discovery drift: expected 9, received ${rows.length}`);
  }
  const discovered = new Set(
    rows.map((row) => `${row.skill}\0${row.action}\0${row["menu-code"]}`),
  );
  const publicActions = accounting.builder_actions.filter(
    (action) => action.visibility === "public",
  );
  for (const action of publicActions) {
    const key = `${action.source_skill}\0${action.action}\0${action.menu_code}`;
    if (!discovered.has(key)) {
      fail(`Builder public action is not source-derived ${action.id}`);
    }
  }
  const evalMember = archive.members.find(
    (member) =>
      member.path === "skills/bmad-eval-runner/SKILL.md" &&
      member.categories.includes("primary_skill"),
  );
  const evalText = archive.contents
    .get("skills/bmad-eval-runner/SKILL.md")
    ?.toString("utf8");
  const internalActions = accounting.builder_actions.filter(
    (action) => action.visibility === "internal",
  );
  if (
    !evalMember ||
    !/\bevals?\b/iu.test(evalText ?? "") ||
    internalActions.length !== 1 ||
    internalActions[0].source_skill !== "bmad-eval-runner"
  ) {
    fail("Builder internal Eval Runner action is not source-derived");
  }
  return {
    public: rows.map((row) => ({
      skill: row.skill,
      action: row.action,
      menu_code: row["menu-code"],
    })),
    internal: [
      {
        skill: "bmad-eval-runner",
        action: internalActions[0].action,
      },
    ],
  };
}

function verifyGeneratedOutputInventory(args, ledger, inventory) {
  if (
    inventory.schema_version !==
      "sapphirus.bmad100.generated-output-target-inventory/v1" ||
    inventory.allowed_scheme !== "generated-package" ||
    inventory.disposition !== "reviewed_unmaterialized_authoring_target"
  ) {
    fail("generated-output target inventory contract is invalid");
  }
  const contract = inventory.consumer_contract;
  if (
    contract?.id !== "builder_generated_package_validator" ||
    !contract.promotion_rule ||
    !contract.exclusion_reason
  ) {
    fail("generated-output inventory lacks a precise consumer/exclusion contract");
  }
  const validator = resolve(repositoryRoot, contract.validator ?? "");
  if (!existsSync(validator) || !lstatSync(validator).isFile()) {
    fail("generated-output inventory consumer validator is missing");
  }
  requireUnique(
    inventory.targets,
    (target) => target.target_uri,
    "generated-output target inventory",
  );
  const inventoryByUri = new Map();
  for (const target of inventory.targets) {
    const uri = target.target_uri;
    if (
      typeof uri !== "string" ||
      !/^generated-package:\/\/[a-z0-9][a-z0-9-]*\/(?:assets|references|scripts)\/[A-Za-z0-9_.\/-]+$/u.test(
        uri,
      ) ||
      uri.includes("\\") ||
      uri.includes("/../") ||
      uri.includes("/./") ||
      uri.includes("//", "generated-package://".length) ||
      uri.includes("?") ||
      uri.includes("#")
    ) {
      fail(`unknown or malformed generated-package target ${String(uri)}`);
    }
    const directory = uri.endsWith("/");
    if (target.target_kind !== (directory ? "directory" : "file")) {
      fail(`generated-package target kind drift ${uri}`);
    }
    if (target.target_identity_sha256 !== sha256(uri)) {
      fail(`generated-package target identity hash drift ${uri}`);
    }
    inventoryByUri.set(uri, target);
  }
  const generatedEdges = ledger.resource_edges.filter(
    (edge) => edge.kind === "generated_output_reference",
  );
  const referenced = new Set();
  for (const edge of generatedEdges) {
    const target = inventoryByUri.get(edge.to);
    if (!target) {
      fail(`generated-package edge has no reviewed target contract ${edge.to}`);
    }
    const parsed = new URL(edge.to);
    const targetPath = parsed.pathname.slice(1);
    if (targetPath !== edge.literal) {
      fail(`generated-package edge literal/target mismatch ${edge.from} -> ${edge.to}`);
    }
    referenced.add(edge.to);
  }
  for (const uri of inventoryByUri.keys()) {
    if (!referenced.has(uri)) {
      fail(`generated-output target inventory contains an orphan ${uri}`);
    }
  }
  return {
    target_count: inventory.targets.length,
    edge_count: generatedEdges.length,
    inventory_sha256: sha256(readFileSync(args.generatedOutputInventory)),
    disposition: inventory.disposition,
    consumer: contract.id,
    package_bytes_asserted: false,
  };
}

function verifyBuilderLedger(sourceManifest, ledger, generatedOutputInventory) {
  const builderMembers = sourceManifest.members.filter((member) => member.source_id === "builder");
  if (ledger.totals?.archive_members !== builderMembers.length) {
    fail("Builder ledger archive member denominator drift");
  }
  if (ledger.members.length !== builderMembers.length + builderGovernanceSidecars.size) {
    fail(
      `Builder ledger member count drift: expected ${builderMembers.length + builderGovernanceSidecars.size}, received ${ledger.members.length}`,
    );
  }
  requireUnique(ledger.members, (member) => member.id, "Builder ledger members");
  const sourceByPath = new Map(builderMembers.map((member) => [member.path, member]));
  const ledgerPaths = new Set();
  for (const member of ledger.members) {
    ledgerPaths.add(member.path);
    if (!member.classification || !Array.isArray(member.consumers) || member.consumers.length === 0) {
      fail(`Builder ledger member lacks classification or consumer ${member.path}`);
    }
    const source = sourceByPath.get(member.path);
    if (source && source.sha256 !== member.sha256) fail(`Builder ledger hash drift ${member.path}`);
    if (!source && !builderGovernanceSidecars.has(member.path)) {
      fail(`Builder ledger has extra member ${member.path}`);
    }
  }
  for (const member of builderMembers) {
    if (!ledgerPaths.has(member.path)) fail(`Builder ledger omitted source member ${member.path}`);
  }
  const sampleRows = ledger.members.filter((member) => member.classification === "sample_fixture");
  if (sampleRows.length !== ledger.totals.classifications?.sample_fixture) {
    fail("Builder sample classification count drift");
  }
  const memberPaths = new Set(ledger.members.map((member) => member.path));
  const consumerAnchors = new Map([
    ["runtime", resolve(repositoryRoot, "packages/bmad-foundation/runtime-manifest.json")],
    ["validator", resolve(repositoryRoot, "packages/bmad-foundation/scripts/builder-resource-closure.mjs")],
    ["generated_package", resolve(repositoryRoot, "packages/bmad-foundation/normalized/builder-agent.package.json")],
    ["ui_catalog", resolve(repositoryRoot, "packages/bmad-foundation/capability-catalog.json")],
    ["installer", resolve(repositoryRoot, "packages/bmad-foundation/normalized")],
    ["conformance_fixture", resolve(repositoryRoot, "packages/bmad-foundation/tests/builder-source-closure.test.mjs")],
    ["sandbox_fixture", resolve(repositoryRoot, "packages/bmad-fixtures")],
    ["distribution_only", resolve(repositoryRoot, "packages/bmad-foundation/source/builder/2.1.0/distribution-decision.json")],
  ]);
  const minimumConsumers = new Map([
    ["production_source", ["runtime", "validator"]],
    ["nested_setup_template", ["generated_package", "installer", "conformance_fixture"]],
    ["sample_fixture", ["sandbox_fixture", "conformance_fixture"]],
    ["upstream_test", ["conformance_fixture"]],
    ["web_distribution_asset", ["distribution_only"]],
    ["documentation", ["distribution_only"]],
  ]);
  for (const member of ledger.members) {
    const minimum =
      member.classification === "package_metadata"
        ? member.source_kind === "sapphirus_governance_sidecar"
          ? ["validator", "conformance_fixture"]
          : ["distribution_only"]
        : minimumConsumers.get(member.classification);
    if (!minimum) fail(`Builder member has unknown classification ${member.path}`);
    for (const consumer of minimum) {
      if (!member.consumers.includes(consumer)) {
        fail(`Builder member lacks required ${consumer} consumer ${member.path}`);
      }
    }
    for (const consumer of member.consumers) {
      const anchor = consumerAnchors.get(consumer);
      if (!anchor || !existsSync(anchor)) {
        fail(`Builder member has unresolved consumer ${consumer}:${member.path}`);
      }
      if (
        consumer === "runtime" &&
        member.production_skill === null &&
        !["skills/module-help.csv", "skills/module.yaml"].includes(member.path)
      ) {
        fail(`Builder runtime consumer has no production skill owner ${member.path}`);
      }
      if (
        consumer === "sandbox_fixture" &&
        member.classification !== "sample_fixture"
      ) {
        fail(`Builder non-sample is falsely connected to sandbox ${member.path}`);
      }
      if (
        consumer === "ui_catalog" &&
        !["production_source"].includes(member.classification)
      ) {
        fail(`Builder non-production row is falsely UI-counted ${member.path}`);
      }
    }
  }
  requireUnique(
    ledger.resource_edges,
    (edge) => `${edge.from}\0${edge.to}\0${edge.kind}\0${edge.literal}`,
    "Builder resource edges",
  );
  for (const edge of ledger.resource_edges) {
    if (!memberPaths.has(edge.from)) fail(`Builder resource edge has missing source ${edge.from}`);
    if (
      !memberPaths.has(edge.to) &&
      edge.kind !== "generated_output_reference"
    ) {
      fail(`Builder resource edge is unresolved ${edge.from} -> ${edge.to}`);
    }
  }
  if (ledger.production_skills.length !== 5) fail("Builder production skill closure count drift");
  for (const skill of ledger.production_skills) {
    if (skill.closure_percent !== 100 || skill.resource_member_count !== skill.closed_member_count) {
      fail(`Builder production skill is not closed ${skill.id}`);
    }
  }
  return {
    members: ledger.members.length,
    resourceEdges: ledger.resource_edges.length,
    samples: sampleRows.length,
    classifications: ledger.totals.classifications,
    generatedOutputs: generatedOutputInventory,
  };
}

function verifyFreshBuilderLedger(args) {
  const result = spawnSync(
    process.execPath,
    [
      resolve(args.foundationRoot, "scripts/builder-resource-closure.mjs"),
      "--builder",
      "2.1.0",
      "--strict",
      "--check-ledger",
      args.builderLedger,
    ],
    {
      cwd: repositoryRoot,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    },
  );
  if (result.status !== 0) {
    fail(
      `fresh Builder graph regeneration rejected the ledger: ${String(result.stderr || result.stdout).trim()}`,
    );
  }
  return String(result.stdout).trim();
}

function verifyScriptConsumers(args, archives, accounting) {
  const inventory = readJson(
    resolve(args.foundationRoot, "script-inventory.json"),
    "script inventory",
  );
  const inventoryByKey = new Map(
    inventory.entries.map((entry) => [entry.member_key, entry]),
  );
  requireUnique(inventory.entries, (entry) => entry.member_key, "script inventory");
  const accountingByKey = new Map(
    accounting.members.map((member) => [member.member_key, member]),
  );
  let discoveredScripts = 0;
  let inventoryScripts = 0;
  for (const [sourceId, archive] of archives) {
    for (const member of archive.members) {
      if (!member.categories.includes("script")) continue;
      discoveredScripts += 1;
      const key = `${sourceId}:${member.path}`;
      const accountingRow = accountingByKey.get(key);
      if (!accountingRow) fail(`orphaned script source ${key}`);
      const inventoryRow = inventoryByKey.get(key);
      if (!inventoryRow) {
        if (
          !["distribution", "reject_nonruntime"].includes(accountingRow.treatment) ||
          !accountingRow.verification_use
        ) {
          fail(`script has no inventory consumer or reviewed exclusion ${key}`);
        }
        continue;
      }
      inventoryScripts += 1;
      if (
        inventoryRow.sha256 !== member.sha256 ||
        !inventoryRow.treatment ||
        !Array.isArray(inventoryRow.consumers)
      ) {
        fail(`script inventory drift or unresolved consumer ${key}`);
      }
    }
  }
  for (const entry of inventory.entries) {
    const archive = archives.get(entry.source_id);
    const member = archive?.members.find((candidate) => candidate.path === entry.path);
    if (!member || member.sha256 !== entry.sha256) {
      fail(`script inventory contains orphaned or drifted row ${entry.member_key}`);
    }
  }
  return { discoveredScripts, inventoryScripts, reviewedExclusions: discoveredScripts - inventoryScripts };
}

function verifyMethodResourceGraph(archive, accounting, catalog) {
  const memberByPath = new Map(archive.members.map((member) => [member.path, member]));
  const accountingByKey = new Map(
    accounting.members
      .filter((member) => member.source_id === "method")
      .map((member) => [member.path, member]),
  );
  const catalogIds = new Set(catalog.capabilities.map((row) => row.canonicalId));
  const roots = archive.members
    .filter((member) => member.categories.includes("primary_skill"))
    .map((member) => posix.dirname(member.path))
    .sort((left, right) => right.length - left.length);
  const edges = [];
  for (const member of archive.members) {
    const ownerRoot = roots.find(
      (root) => member.path === `${root}/SKILL.md` || member.path.startsWith(`${root}/`),
    );
    if (!ownerRoot) continue;
    if (member.path !== `${ownerRoot}/SKILL.md`) {
      edges.push({
        from: `${ownerRoot}/SKILL.md`,
        to: member.path,
        kind: "recursive_skill_ownership",
        target_sha256: member.sha256,
      });
    }
    if (
      !/\.(?:md|[cm]?js|ts|py|toml|ya?ml|json|csv)$/iu.test(member.path) ||
      member.byte_length > 1024 * 1024
    ) {
      continue;
    }
    const text = archive.contents.get(member.path).toString("utf8");
    for (const line of text.split(/\r?\n/u)) {
      const references = [];
      for (const match of line.matchAll(
        /\{skill-root\}\/((?:assets|references|scripts)\/[A-Za-z0-9_.\/-]+)/gu,
      )) {
        references.push(match[1]);
      }
      if (!/\be\.g\./iu.test(line) && !/Bare paths/iu.test(line)) {
        for (const match of line.matchAll(
          /`((?:assets|references|scripts)\/[A-Za-z0-9_.\/-]+)`/gu,
        )) {
          references.push(match[1]);
        }
      }
      if (member.path.endsWith("SKILL.md")) {
        for (const match of line.matchAll(
          /\]\((?!https?:|mailto:|#|\{)([^)\s]+)\)/gu,
        )) {
          references.push(match[1].split("#", 1)[0]);
        }
      }
      for (const literal of references) {
        const cleaned = literal.replace(/[.,;:]+$/u, "");
        if (!cleaned || /[{}<>*]/u.test(cleaned)) continue;
        const target = posix.normalize(posix.join(ownerRoot, cleaned));
        const targetMember = memberByPath.get(target);
        const directoryMembers = archive.members.filter((candidate) =>
          candidate.path.startsWith(`${target.replace(/\/$/u, "")}/`),
        );
        if (!targetMember && directoryMembers.length === 0) {
          fail(`Method local resource edge is unresolved ${member.path} -> ${target}`);
        }
        for (const resolved of targetMember ? [targetMember] : directoryMembers) {
          const consumer = accountingByKey.get(resolved.path);
          if (!consumer) fail(`Method resource target is orphaned ${resolved.path}`);
          if (
            !consumer.target_capability &&
            !consumer.verification_use
          ) {
            fail(`Method resource target lacks a consumer or reviewed exclusion ${resolved.path}`);
          }
          if (
            consumer.target_capability &&
            !consumer.target_capability.endsWith(":shared-runtime-source") &&
            !catalogIds.has(consumer.target_capability)
          ) {
            fail(`Method resource target has unresolved runtime consumer ${resolved.path}`);
          }
          edges.push({
            from: member.path,
            to: resolved.path,
            kind: "declared_local_reference",
            target_sha256: resolved.sha256,
          });
        }
      }
    }
  }
  const uniqueEdges = [
    ...new Map(
      edges.map((edge) => [`${edge.from}\0${edge.to}\0${edge.kind}`, edge]),
    ).values(),
  ];
  const ownedMembers = new Set(
    uniqueEdges
      .filter((edge) => edge.kind === "recursive_skill_ownership")
      .map((edge) => edge.to),
  );
  for (const member of archive.members) {
    const root = roots.find((candidate) => member.path.startsWith(`${candidate}/`));
    if (
      root &&
      member.path !== `${root}/SKILL.md` &&
      !ownedMembers.has(member.path)
    ) {
      fail(`Method primary resource is orphaned ${member.path}`);
    }
  }
  return {
    primaryRoots: roots.length,
    edges: uniqueEdges.length,
    declaredLocalEdges: uniqueEdges.filter((edge) => edge.kind === "declared_local_reference").length,
    ownershipEdges: uniqueEdges.filter((edge) => edge.kind === "recursive_skill_ownership").length,
  };
}

function verifyCatalog(accounting, catalog) {
  if (
    catalog.counts?.method !== 48 ||
    catalog.counts?.builderPublic !== 9 ||
    catalog.counts?.builderInternal !== 1 ||
    catalog.counts?.total !== 58
  ) {
    fail(`capability catalog count drift ${JSON.stringify(catalog.counts)}`);
  }
  if (catalog.capabilities.length !== 58) fail("capability catalog row count drift");
  requireUnique(catalog.capabilities, (row) => row.rowId, "capability catalog rows");
  const rowsByCanonical = new Map();
  for (const row of catalog.capabilities) {
    const rows = rowsByCanonical.get(row.canonicalId) ?? [];
    rows.push(row);
    rowsByCanonical.set(row.canonicalId, rows);
  }
  for (const [canonicalId, rows] of rowsByCanonical) {
    if (
      rows.length > 1 &&
      rows.filter((row) => !row.source.path.includes("/v6-shims/")).length > 1
    ) {
      fail(`capability catalog has duplicate non-shim canonical id ${canonicalId}`);
    }
  }
  const accountingByKey = new Map(accounting.members.map((member) => [member.member_key, member]));
  const actionById = new Map(accounting.builder_actions.map((action) => [action.id, action]));
  for (const row of catalog.capabilities) {
    const member = accountingByKey.get(`${row.source.id}:${row.source.path}`);
    if (!member || member.sha256 !== row.source.sha256 || member.class !== row.source.class) {
      fail(`catalog source row is missing or drifted ${row.rowId}`);
    }
    if (row.source.id === "builder") {
      const action = actionById.get(row.rowId);
      if (!action || action.action !== row.action || action.visibility !== row.visibility) {
        fail(`catalog Builder action is missing or drifted ${row.rowId}`);
      }
    }
  }
}

function verifyNormalizedPackages(args, sourceLock) {
  const normalizedRoot = resolve(args.foundationRoot, "normalized");
  const files = existsSync(normalizedRoot)
    ? listFiles(normalizedRoot).filter((path) => path.endsWith(".json"))
    : [];
  const expected = new Map(
    sourceLock.sources.map((source) => [
      source.id,
      {
        version: source.version,
        archive: source.archive_sha256,
      },
    ]),
  );
  const semanticLedger = readJson(
    resolve(args.foundationRoot, "semantic-source-ledger.json"),
    "semantic source ledger",
  );
  const semanticIds = new Set(semanticLedger.sourceMembers.map((member) => member.id));
  const semanticBySourcePath = new Map(
    semanticLedger.sourceMembers.map((member) => [
      `${member.sourceId}:${member.member}`,
      member.sha256,
    ]),
  );
  let identityClaims = 0;
  let sourceMemberReferences = 0;
  let sourcePathReferences = 0;
  for (const path of files) {
    const value = readJson(resolve(normalizedRoot, path), `normalized package ${path}`);
    const identity = value.sourceIdentity;
    if (identity) {
      identityClaims += 1;
      const facts = expected.get(identity.sourceId);
      if (!facts) fail(`normalized package claims unknown source ${identity.sourceId}`);
      if (
        identity.packageVersion !== facts.version ||
        identity.archiveSha256 !== `sha256:${facts.archive}`
      ) {
        fail(`normalized package source identity drift ${path}`);
      }
    }
    const sourceId = identity?.sourceId ?? (value.packageName === "bmad-builder" ? "builder" : null);
    const visit = (node) => {
      if (Array.isArray(node)) {
        for (const item of node) visit(item);
        return;
      }
      if (!node || typeof node !== "object") return;
      if (Array.isArray(node.sourceMemberIds)) {
        for (const id of node.sourceMemberIds) {
          sourceMemberReferences += 1;
          if (!semanticIds.has(id)) {
            fail(`normalized package ${path} references unknown source member ${id}`);
          }
        }
      }
      if (
        sourceId &&
        typeof node.path === "string" &&
        typeof node.contentHash === "string" &&
        /^(?:src|skills)\//u.test(node.path)
      ) {
        sourcePathReferences += 1;
        const expectedHash = semanticBySourcePath.get(`${sourceId}:${node.path}`);
        if (!expectedHash || node.contentHash !== `sha256:${expectedHash}`) {
          fail(`normalized package source resource drift ${path}:${node.path}`);
        }
      }
      for (const child of Object.values(node)) visit(child);
    };
    visit(value);
  }
  return {
    files: files.length,
    identityClaims,
    sourceMemberReferences,
    sourcePathReferences,
  };
}

function verifyFixturesAndDocs(args, sourceLock) {
  const fixturePath = resolve(repositoryRoot, "packages/bmad-fixtures/fixtures/current/source-and-catalog.json");
  const fixture = readJson(fixturePath, "current source/catalog characterization fixture");
  if (!fixture.sourceLedger?.contentHash || !Array.isArray(fixture.closureLedger?.capabilityIds)) {
    fail("current source/catalog characterization fixture is incomplete");
  }
  const semanticPath = resolve(args.foundationRoot, "semantic-source-ledger.json");
  const closurePath = resolve(args.foundationRoot, "capability-closure-ledger.json");
  const closure = readJson(closurePath, "capability closure ledger");
  if (fixture.sourceLedger.contentHash !== `sha256:${sha256(readFileSync(semanticPath))}`) {
    fail("current source characterization fixture hash is stale");
  }
  if (
    stableJson(fixture.closureLedger.capabilityIds) !==
    stableJson(closure.capabilities.map((capability) => capability.capabilityId))
  ) {
    fail("current catalog characterization fixture is stale");
  }
  const provenancePath = resolve(repositoryRoot, "docs/bmad/source-provenance.md");
  const builderAccountingPath = resolve(repositoryRoot, "docs/bmad/builder/source-accounting.md");
  const provenance = readFileSync(provenancePath, "utf8");
  const builderAccounting = readFileSync(builderAccountingPath, "utf8");
  for (const source of sourceLock.sources) {
    if (
      !provenance.includes(source.version) ||
      !provenance.includes(source.archive_sha256) ||
      !provenance.includes(String(source.regular_file_count))
    ) {
      fail(`source provenance documentation drift for ${source.id}`);
    }
  }
  if (
    !builderAccounting.includes("299/299") ||
    !builderAccounting.includes("production capability count remains exactly five")
  ) {
    fail("Builder source accounting documentation is stale");
  }
  return {
    fixture: toPosix(relative(repositoryRoot, fixturePath)),
    documentation: [
      toPosix(relative(repositoryRoot, provenancePath)),
      toPosix(relative(repositoryRoot, builderAccountingPath)),
    ],
  };
}

function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, stableJson(value), "utf8");
}

function invalidatePriorReceipt(args) {
  for (const staleEvidence of [
    "logs/meta-tests.json",
    "logs/meta-test-results.json",
    "logs/meta-tests.tap",
  ]) {
    rmSync(resolve(dirname(args.report), staleEvidence), { force: true });
  }
  if (existsSync(args.report)) {
    const report = readJson(args.report, "prior QAL-01 report");
    report.status = "verification_in_progress";
    report.complete = false;
    delete report.meta_tests;
    delete report.command_results;
    writeJson(args.report, report);
  }
  const verificationLog = resolve(dirname(args.report), "logs/verification.json");
  if (existsSync(verificationLog)) {
    const log = readJson(verificationLog, "prior QAL-01 verification log");
    log.task_complete = false;
    log.pending_required_command = "pnpm verify:bmad:source-meta-tests";
    writeJson(verificationLog, log);
  }
}

function gitIdentity() {
  const headCommit = run("git", ["rev-parse", "HEAD"]).trim();
  const headTree = run("git", ["rev-parse", "HEAD^{tree}"]).trim();
  const status = run("git", ["status", "--porcelain=v1", "--untracked-files=all"]);
  return {
    head_commit: headCommit,
    head_tree: headTree,
    worktree_status_sha256: sha256(status),
    worktree_clean: status.length === 0,
  };
}

function toolchainIdentity() {
  const node = process.version.replace(/^v/u, "");
  const pnpm =
    process.platform === "win32"
      ? run("cmd.exe", ["/d", "/s", "/c", "pnpm --version"]).trim()
      : run("pnpm", ["--version"]).trim();
  if (node !== "24.18.0") {
    fail(`QAL-01 requires Node 24.18.0, received ${node}`);
  }
  if (pnpm !== "11.15.1") {
    fail(`QAL-01 requires pnpm 11.15.1, received ${pnpm}`);
  }
  return { node, pnpm };
}

function verificationInputIdentity(paths, archives) {
  const rows = [];
  for (const path of [...new Set(paths)].sort()) {
    if (!existsSync(path) || !lstatSync(path).isFile()) {
      fail(`verification input is missing ${path}`);
    }
    rows.push({
      path: toPosix(relative(repositoryRoot, path)),
      sha256: sha256(readFileSync(path)),
    });
  }
  for (const [sourceId, archive] of [...archives.entries()].sort()) {
    rows.push({
      path: `archive:${sourceId}`,
      sha256: archive.sha256,
    });
  }
  return {
    sha256: sha256(stableJson(rows)),
    inputs: rows,
  };
}

function main() {
  const startedAt = new Date().toISOString();
  const startedNanoseconds = process.hrtime.bigint();
  const args = parseArgs(process.argv.slice(2));
  invalidatePriorReceipt(args);
  const toolchain = toolchainIdentity();
  const sourceLock = readJson(resolve(args.foundationRoot, "source-lock.json"), "source lock");
  const sourceManifestBytes = readFileSync(args.sourceManifest);
  const sourceManifest = JSON.parse(sourceManifestBytes);
  const accountingBytes = readFileSync(args.sourceAccounting);
  const accounting = JSON.parse(accountingBytes);
  const builderLedger = readJson(args.builderLedger, "Builder source ledger");
  const catalog = readJson(args.catalog, "capability catalog");
  const generatedOutputInventory = readJson(
    args.generatedOutputInventory,
    "generated-output target inventory",
  );

  if (sourceLock.sources.length !== 2 || sourceManifest.sources.length !== 2) {
    fail("source denominator must contain exactly Method and Builder");
  }
  requireUnique(sourceManifest.members, (member) => `${member.source_id}:${member.path}`, "source manifest");
  const lockById = new Map(sourceLock.sources.map((source) => [source.id, source]));
  const manifestSources = new Map(sourceManifest.sources.map((source) => [source.id, source]));
  const accountingByKey = new Map(accounting.members.map((member) => [member.member_key, member]));
  const archives = new Map();
  const discoveredPrimary = new Map();

  for (const id of ["method", "builder"]) {
    const locked = lockById.get(id);
    const source = manifestSources.get(id);
    if (!locked || !source) fail(`missing pinned ${id} source identity`);
    for (const field of [
      "version",
      "archive_sha256",
      "archive_root",
      "regular_file_count",
      "skill_md_count",
    ]) {
      if (locked[field] !== source[field]) fail(`${id} source manifest identity drift for ${field}`);
    }
    const archive = loadArchive(args, locked);
    archives.set(id, archive);
    const members = sourceManifest.members.filter((member) => member.source_id === id);
    discoveredPrimary.set(
      id,
      verifyArchiveManifestClosure(locked, archive, members),
    );
    verifySourceMembers(args, locked, members);
  }

  const builderActionDiscovery = discoverBuilderActions(
    archives.get("builder"),
    accounting,
  );
  if (
    builderActionDiscovery.public.length !== lockById.get("builder").public_action_count ||
    builderActionDiscovery.internal.length !== lockById.get("builder").internal_action_count
  ) {
    fail("archive-derived Builder action denominator drift");
  }
  const emptyActions = { public: [], internal: [] };
  for (const id of ["method", "builder"]) {
    const locked = lockById.get(id);
    const manifest = generatedManifest(
      locked,
      archives.get(id),
      accountingByKey,
      id === "builder" ? builderActionDiscovery : emptyActions,
    );
    writeJson(resolve(args.manifestDir, `${id}-${locked.version}.json`), manifest);
  }

  const accountingResult = verifyAccounting(sourceManifest, accounting);
  if (
    accounting.source_manifest?.sha256 !== sha256(sourceManifestBytes) ||
    accounting.source_manifest?.member_count !== sourceManifest.members.length
  ) {
    fail("source accounting is not hash-bound to the current source manifest");
  }
  if (catalog.sourceAccountingSha256 !== sha256(accountingBytes)) {
    fail("capability catalog is not hash-bound to the current source accounting");
  }
  const { catalogSha256, ...catalogBody } = catalog;
  if (catalogSha256 !== sha256(stableJson(catalogBody))) {
    fail("capability catalog self-hash drift");
  }
  const freshBuilderGraph = verifyFreshBuilderLedger(args);
  const generatedOutputClosure = verifyGeneratedOutputInventory(
    args,
    builderLedger,
    generatedOutputInventory,
  );
  const builderResult = verifyBuilderLedger(
    sourceManifest,
    builderLedger,
    generatedOutputClosure,
  );
  if (
    builderLedger.builder?.source_manifest_sha256 !== sha256(sourceManifestBytes) ||
    builderLedger.builder?.archive_sha256 !== lockById.get("builder").archive_sha256
  ) {
    fail("Builder resource ledger is not hash-bound to the pinned source manifest");
  }
  const scriptClosure = verifyScriptConsumers(args, archives, accounting);
  verifyCatalog(accounting, catalog);
  const methodGraph = verifyMethodResourceGraph(
    archives.get("method"),
    accounting,
    catalog,
  );
  const normalized = verifyNormalizedPackages(args, sourceLock);
  const references = verifyFixturesAndDocs(args, sourceLock);
  const git = gitIdentity();

  const denominators = {
    archive_members:
      archives.get("method").members.length + archives.get("builder").members.length,
    method_archive_members: archives.get("method").members.length,
    builder_archive_members: archives.get("builder").members.length,
    method_primary_skills: discoveredPrimary.get("method").length,
    builder_primary_skills: discoveredPrimary.get("builder").length,
    builder_public_actions: builderActionDiscovery.public.length,
    builder_internal_actions: builderActionDiscovery.internal.length,
  };
  if (
    accountingResult.methodPrimary !== denominators.method_primary_skills ||
    accountingResult.builderPrimary !== denominators.builder_primary_skills ||
    accountingResult.publicActions.length !== denominators.builder_public_actions ||
    accountingResult.internalActions.length !== denominators.builder_internal_actions
  ) {
    fail("accounting denominators differ from archive-derived discovery");
  }
  const inputs = verificationInputIdentity(
    [
      resolve(args.foundationRoot, "source-lock.json"),
      args.sourceManifest,
      args.sourceAccounting,
      args.builderLedger,
      args.catalog,
      resolve(args.foundationRoot, "script-inventory.json"),
      args.generatedOutputInventory,
      resolve(args.foundationRoot, "semantic-source-ledger.json"),
      resolve(args.foundationRoot, "capability-closure-ledger.json"),
      resolve(repositoryRoot, "packages/bmad-fixtures/fixtures/current/source-and-catalog.json"),
      resolve(repositoryRoot, "docs/bmad/source-provenance.md"),
      resolve(repositoryRoot, "docs/bmad/builder/source-accounting.md"),
      resolve(import.meta.filename),
      resolve(repositoryRoot, "tools/bmad100/tests/source-closure-meta.test.mjs"),
      resolve(repositoryRoot, "tools/bmad100/lib/qal-01-meta-contract.mjs"),
      resolve(repositoryRoot, "tools/bmad100/run-source-closure-meta.mjs"),
      resolve(repositoryRoot, "tools/bmad100/publish-source-closure-receipt.mjs"),
      resolve(args.manifestDir, "method-6.10.0.json"),
      resolve(args.manifestDir, "builder-2.1.0.json"),
    ],
    archives,
  );
  const endedAt = new Date().toISOString();
  const durationMs = Number(process.hrtime.bigint() - startedNanoseconds) / 1_000_000;
  const receiptCandidateBody = {
    schema_version: "sapphirus.bmad100.qal-01-receipt-candidate/v1",
    head_commit: git.head_commit,
    qal_input_tree_sha256: inputs.sha256,
    toolchain,
    archives: Object.fromEntries(
      [...archives]
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([sourceId, archive]) => [sourceId, archive.sha256]),
    ),
    archive_manifests: {
      method: sha256(
        readFileSync(resolve(args.manifestDir, "method-6.10.0.json")),
      ),
      builder: sha256(
        readFileSync(resolve(args.manifestDir, "builder-2.1.0.json")),
      ),
    },
    verifier_sha256: sha256(readFileSync(resolve(import.meta.filename))),
    meta_test_sha256: sha256(
      readFileSync(
        resolve(repositoryRoot, "tools/bmad100/tests/source-closure-meta.test.mjs"),
      ),
    ),
  };
  const receiptCandidate = {
    ...receiptCandidateBody,
    candidate_sha256: sha256(stableJson(receiptCandidateBody)),
  };
  const result = {
    schema_version: "sapphirus.bmad100.source-closure/v1",
    complete: true,
    generated_at: endedAt,
    freshness: {
      started_at: startedAt,
      ended_at: endedAt,
      duration_ms: Math.round(durationMs),
      git,
      toolchain,
      verification_input_tree: inputs,
    },
    receipt_candidate: receiptCandidate,
    source_manifest: {
      path: toPosix(relative(repositoryRoot, args.sourceManifest)),
      sha256: sha256(sourceManifestBytes),
    },
    pinned_sources: sourceLock.sources.map((source) => ({
      id: source.id,
      version: source.version,
      archive_path: archives.get(source.id).path,
      archive_sha256: archives.get(source.id).sha256,
      archive_byte_length: archives.get(source.id).byte_length,
      archived_package: archives.get(source.id).package,
    })),
    denominators,
    classifications: {
      archive_categories: countBy(
        [...archives].flatMap(([sourceId, archive]) =>
          archive.members.flatMap((member) =>
            member.categories.map((category) => ({ sourceId, category })),
          ),
        ),
        (row) => `${row.sourceId}:${row.category}`,
      ),
      treatments: countBy(accounting.members, (member) => member.treatment),
      builder: builderResult.classifications,
    },
    closure: {
      imported_source_bytes: "pass",
      pinned_archive_bytes: "pass",
      accounting: "pass",
      builder_resources: "pass",
      catalog: "pass",
      normalized_packages: "pass",
      fixtures: "pass",
      documentation: "pass",
      unresolved_edges: 0,
      unclassified_rows: 0,
      orphaned_rows: 0,
      false_production_rows: 0,
    },
    builder_graph: {
      accounted_members: builderResult.members,
      directed_resource_edges: builderResult.resourceEdges,
      sample_rows: builderResult.samples,
      public_actions_discovered_from_module_help: builderActionDiscovery.public.length,
      internal_action_discovered_from_skill: builderActionDiscovery.internal[0].skill,
      fresh_regeneration: freshBuilderGraph,
      generated_output_targets: builderResult.generatedOutputs,
    },
    method_graph: methodGraph,
    script_closure: scriptClosure,
    normalized_packages: normalized,
    references,
    generated_manifests: [
      toPosix(relative(repositoryRoot, resolve(args.manifestDir, "method-6.10.0.json"))),
      toPosix(relative(repositoryRoot, resolve(args.manifestDir, "builder-2.1.0.json"))),
    ],
  };
  writeJson(args.json, result);
  writeJson(args.report, {
    schema_version: "sapphirus.bmad100.quality-report/v1",
    task_id: "QAL-01",
    status: "pending_meta_tests",
    complete: false,
    generated_at: endedAt,
    freshness: result.freshness,
    denominators,
    checks: result.closure,
    evidence: [toPosix(relative(repositoryRoot, args.json)), ...result.generated_manifests],
  });
  writeJson(resolve(dirname(args.report), "logs/verification.json"), {
    schema_version: "sapphirus.bmad100.verification-log/v1",
    task_id: "QAL-01",
    command:
      "node tools/bmad100/verify-source-closure.mjs --strict --json artifacts/bmad100/source-closure.json",
    exit_code: 0,
    started_at: startedAt,
    ended_at: endedAt,
    duration_ms: Math.round(durationMs),
    git,
    toolchain,
    verification_input_tree_sha256: inputs.sha256,
    receipt_candidate: receiptCandidate,
    command_results: [
      {
        command: "verify pinned Method archive payload",
        exit_code: 0,
        archive_sha256: archives.get("method").sha256,
      },
      {
        command: "verify pinned Builder archive payload",
        exit_code: 0,
        archive_sha256: archives.get("builder").sha256,
      },
      {
        command:
          "node packages/bmad-foundation/scripts/builder-resource-closure.mjs --builder 2.1.0 --strict --check-ledger packages/bmad-foundation/builder-source-ledger.json",
        exit_code: 0,
      },
      {
        command:
          "node tools/bmad100/verify-source-closure.mjs --strict --json artifacts/bmad100/source-closure.json",
        exit_code: 0,
      },
    ],
    task_complete: false,
    pending_required_command: "pnpm verify:bmad:source-meta-tests",
    source_manifest_sha256: result.source_manifest.sha256,
    pinned_sources: result.pinned_sources,
    denominators,
    checks: result.closure,
  });
  process.stdout.write(
    `BMAD source closure verified: ${denominators.archive_members}/${denominators.archive_members} members, Method ${denominators.method_primary_skills}/48, Builder ${denominators.builder_primary_skills}/5 skills and ${denominators.builder_public_actions + denominators.builder_internal_actions}/10 actions.\n`,
  );
}

try {
  main();
} catch (error) {
  process.stderr.write(`BMAD source closure failed: ${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
