import { createHash } from "node:crypto";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, extname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(packageRoot, "..", "..");
const allowedTreatments = new Set([
  "port",
  "sandbox",
  "fixture_only",
  "distribution_only",
  "reject",
]);
const codeExtensions = new Set([
  ".js",
  ".mjs",
  ".cjs",
  ".ts",
  ".tsx",
  ".py",
  ".sh",
  ".bash",
  ".zsh",
  ".ps1",
  ".bat",
  ".cmd",
  ".rb",
  ".pl",
  ".php",
]);
const textExtensions = new Set([
  ...codeExtensions,
  ".json",
  ".toml",
  ".yaml",
  ".yml",
  ".md",
  ".html",
  ".txt",
  ".csv",
  "",
]);

function parseArgs(argv) {
  const options = {
    strict: false,
    check: false,
    write: false,
    manifestPath: join(packageRoot, "source-manifest.json"),
    inventoryPath: join(packageRoot, "script-inventory.json"),
    sourceRoot: join(packageRoot, "source"),
    runtimeRoot: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--strict") options.strict = true;
    else if (argument === "--check") options.check = true;
    else if (argument === "--write") options.write = true;
    else if (argument === "--manifest") options.manifestPath = resolve(argv[++index]);
    else if (argument === "--inventory") options.inventoryPath = resolve(argv[++index]);
    else if (argument === "--source-root") options.sourceRoot = resolve(argv[++index]);
    else if (argument === "--runtime-root") options.runtimeRoot = resolve(argv[++index]);
    else throw new Error(`unknown argument: ${argument}`);
  }
  if (!options.check && !options.write) {
    throw new Error("one of --check or --write is required");
  }
  if (options.check && options.write) {
    throw new Error("--check and --write are mutually exclusive");
  }
  return options;
}

function normalizePath(value) {
  return value.split(sep).join("/");
}

function sha256Text(value) {
  return createHash("sha256").update(value).digest("hex");
}

function readJson(path, label) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${path}: ${error.message}`);
  }
}

function sourceFilePath(sourceRoot, member) {
  return join(sourceRoot, member.source_id, ...member.path.split("/"));
}

function readSourceText(sourceRoot, member) {
  if (member.entry_type !== "file") return "";
  const path = sourceFilePath(sourceRoot, member);
  if (!existsSync(path)) return "";
  const extension = extname(member.path).toLowerCase();
  if (!textExtensions.has(extension)) return "";
  const buffer = readFileSync(path);
  if (buffer.includes(0)) return "";
  return buffer.toString("utf8");
}

function discoverySignals(member, content) {
  const name = basename(member.path);
  const extension = extname(member.path).toLowerCase();
  const signals = [];
  if (member.class === "script_source") signals.push("manifest_script_source");
  if (codeExtensions.has(extension)) signals.push("code_extension");
  if (member.is_executable) signals.push("archive_executable_bit");
  if (content.startsWith("#!")) signals.push("shebang");
  if (/^Dockerfile(?:\..+)?$/i.test(name)) signals.push("dockerfile");
  if (/^(?:docker-)?compose(?:\.[^.]+)?\.ya?ml$/i.test(name)) {
    signals.push("container_compose");
  }
  if (/template/i.test(member.path) && codeExtensions.has(extension)) {
    signals.push("code_emitting_template");
  }
  if (/(?:^|\/)(?:hooks?|scripts?)\//i.test(member.path)) {
    signals.push("script_or_hook_path");
  }
  return [...new Set(signals)].sort();
}

function isCandidate(member, content) {
  return member.entry_type === "file" && discoverySignals(member, content).length > 0;
}

function languageFor(path, content) {
  const extension = extname(path).toLowerCase();
  const languages = {
    ".js": "javascript",
    ".mjs": "javascript_module",
    ".cjs": "javascript_commonjs",
    ".ts": "typescript",
    ".tsx": "typescript_react",
    ".py": "python",
    ".sh": "posix_shell",
    ".bash": "bash",
    ".zsh": "zsh",
    ".ps1": "powershell",
    ".bat": "windows_batch",
    ".cmd": "windows_cmd",
    ".rb": "ruby",
    ".pl": "perl",
    ".php": "php",
  };
  if (languages[extension]) return languages[extension];
  if (/^#!.*python/m.test(content)) return "python";
  if (/^#!.*(?:bash|\/sh)/m.test(content)) return "shell";
  return /^Dockerfile/i.test(basename(path)) ? "dockerfile" : "unknown_executable";
}

function importsFor(content) {
  const imports = new Set();
  const patterns = [
    /(?:^|\n)\s*(?:from|import)\s+([A-Za-z_][\w.]*)/g,
    /(?:from\s+|require\s*\(|import\s*\()\s*["']([^"']+)["']/g,
    /(?:^|\n)\s*(?:source|\.)\s+["']?([^\s"']+)/g,
  ];
  for (const pattern of patterns) {
    for (const match of content.matchAll(pattern)) imports.add(match[1]);
  }
  return [...imports].sort();
}

function riskAnalysis(content, path) {
  const readsFilesystem =
    /\b(?:readFile|read_text|read_bytes|open\s*\(|readdir|glob|walk|existsSync|statSync)\b/.test(
      content,
    );
  const writesFilesystem =
    /\b(?:writeFile|write_text|write_bytes|appendFile|mkdir|unlink|remove|rename|copyFile|shutil\.|fs\.)\b/.test(
      content,
    );
  const requirements = {
    filesystem:
      readsFilesystem && writesFilesystem
        ? "read_write"
        : writesFilesystem
          ? "write"
          : readsFilesystem
            ? "read"
            : "none",
    network: /\b(?:fetch|https?|requests|urllib|socket|curl|wget|WebClient)\b/i.test(content),
    process: /\b(?:child_process|subprocess|spawn|execFile|execSync|os\.system|shell\s*=\s*True)\b/.test(
      content,
    ),
    environment: /\b(?:process\.env|os\.environ|getenv|dotenv)\b/.test(content),
    secrets: /\b(?:secret|password|credential|private[_-]?key|access[_-]?token|keychain|vault)\b/i.test(
      content,
    ),
    schedule: /\b(?:cron|schedule|scheduler|taskschd|schtasks|PULSE)\b/i.test(content),
    package_install:
      /\b(?:npm|pnpm|yarn|pip|uv|apt(?:-get)?|brew)\s+(?:add|install|i)\b/i.test(content),
    archive_extraction: /\b(?:extractall|unzip|tarfile|ZipFile|archive)\b/i.test(content),
    dynamic_code: /\b(?:eval|Function|importlib|exec)\s*\(/.test(content),
    path_traversal:
      /\b(?:realpath|resolve|relative|canonicalize|commonpath)\s*\(/.test(content) ||
      content.includes("../"),
  };
  const flags = Object.entries(requirements)
    .filter(([name, value]) => name !== "filesystem" && value)
    .map(([name]) => name);
  if (requirements.filesystem !== "none") flags.push("filesystem_access");
  if (/(?:^|\/)(?:hooks?|scripts?)\//i.test(path)) flags.push("runtime_hook_or_script");
  return { requirements, explicit_review_flags: [...new Set(flags)].sort() };
}

function expectedIo(content, requirements) {
  const inputs = [];
  const outputs = [];
  if (/\b(?:process\.argv|argparse|sys\.argv|commander|yargs)\b/.test(content)) {
    inputs.push("command_line_arguments");
  }
  if (/\b(?:stdin|input\s*\()\b/.test(content)) inputs.push("standard_input");
  if (requirements.environment) inputs.push("environment_variables");
  if (["read", "read_write"].includes(requirements.filesystem)) inputs.push("filesystem_content");
  if (/\b(?:console\.log|print\s*\(|stdout|stderr)\b/.test(content)) {
    outputs.push("standard_output_or_error");
  }
  if (["write", "read_write"].includes(requirements.filesystem)) outputs.push("filesystem_changes");
  if (requirements.process) outputs.push("subprocess_result");
  return {
    expected_inputs: inputs.length ? inputs : ["none_detected"],
    expected_outputs: outputs.length ? outputs : ["none_detected"],
  };
}

function ownerFor(member) {
  const parts = member.path.split("/");
  const skillIndex = parts.findIndex(
    (part) => part === "skills" || part.endsWith("-skills"),
  );
  if (skillIndex >= 0) {
    const candidate = parts
      .slice(skillIndex + 1)
      .find((part) => part.startsWith("bmad-"));
    if (candidate) return `${member.source_id}:${candidate}`;
  }
  const bmadSegment = parts.find((part) => part.startsWith("bmad-"));
  if (bmadSegment) return `${member.source_id}:${bmadSegment}`;
  if (member.path.startsWith("tools/installer/")) return `${member.source_id}:installer`;
  if (member.path.startsWith("website/")) return `${member.source_id}:website`;
  if (member.path.startsWith(".github/")) return `${member.source_id}:repository-automation`;
  return `${member.source_id}:source-distribution`;
}

const classificationRules = [
  {
    id: "fixture-or-upstream-test",
    treatment: "fixture_only",
    rationale:
      "Retained only as an upstream test or sample fixture; it is never installed or invoked by the product runtime.",
    matches: (member) =>
      member.class === "test_or_sample_fixture" ||
      /(?:^|\/)(?:test|tests)\//i.test(member.path),
  },
  {
    id: "rejected-upstream-installer",
    treatment: "reject",
    rationale:
      "Upstream installation and migration authority is rejected; Sapphirus owns installation through typed native plans, Airlock, validation, and receipts.",
    matches: (member) =>
      member.source_id === "method" &&
      (member.path.startsWith("tools/installer/") ||
        member.path === "tools/migrate-custom-module-paths.js"),
  },
  {
    id: "rejected-generated-autonomy-hook",
    treatment: "reject",
    rationale:
      "Generated wake or scheduling hooks cannot grant autonomy or process authority; autonomy remains explicit, bounded, revocable, and native-owned.",
    matches: (member) =>
      member.class === "template_or_asset" && /wake-template\.(?:py|js)$/i.test(member.path),
  },
  {
    id: "sandboxed-eval-process-orchestration",
    treatment: "sandbox",
    rationale:
      "Eval orchestration requires bounded subprocess or container execution and cannot be represented as a pure port; any future use must cross the governed sandbox boundary.",
    matches: (member) =>
      member.source_id === "builder" &&
      /skills\/bmad-eval-runner\/scripts\/(?:run_evals|run_triggers)\.py$/i.test(member.path),
  },
  {
    id: "ported-code-template",
    treatment: "port",
    rationale:
      "The template describes code-producing behavior that must be independently implemented behind typed native/runtime ports; the retained template itself never executes.",
    matches: (member) =>
      member.class === "template_or_asset" &&
      codeExtensions.has(extname(member.path).toLowerCase()) &&
      !/wake-template\.(?:py|js)$/i.test(member.path),
  },
  {
    id: "distribution-automation",
    treatment: "distribution_only",
    rationale:
      "Repository, documentation, validation, and website automation is retained for provenance and distribution comparison only and is excluded from runtime authority.",
    matches: (member) =>
      member.path.startsWith(".github/") ||
      member.path.startsWith(".husky/") ||
      member.path.startsWith("docs/") ||
      member.path.startsWith("website/") ||
      (member.path.startsWith("tools/") &&
        !member.path.startsWith("tools/installer/") &&
        member.path !== "tools/migrate-custom-module-paths.js") ||
      /(?:^|\/)(?:eslint|prettier|astro)\.config\.[cm]?[jt]s$/i.test(member.path),
  },
  {
    id: "ported-official-skill-behavior",
    treatment: "port",
    rationale:
      "Official skill behavior is semantic input for an independent typed host-code port; the retained upstream script never executes in the product.",
    matches: (member) =>
      member.class !== "test_or_sample_fixture" &&
      member.class !== "template_or_asset" &&
      !/(?:^|\/)(?:test|tests)\//i.test(member.path) &&
      !/skills\/bmad-eval-runner\/scripts\/(?:run_evals|run_triggers)\.py$/i.test(
        member.path,
      ) &&
      (member.path.startsWith("skills/") ||
        member.path.startsWith("src/") ||
        member.path.startsWith("samples/")),
  },
];

function ruleFor(member) {
  const matches = classificationRules.filter((rule) => rule.matches(member));
  if (matches.length !== 1) {
    throw new Error(
      `${member.source_id}:${member.path} matches ${matches.length} classification rules (${matches.map((rule) => rule.id).join(", ") || "none"})`,
    );
  }
  return matches[0];
}

function findIndirectReferences(members, contentByKey, candidate) {
  const name = basename(candidate.path);
  const references = [];
  for (const member of members) {
    const key = `${member.source_id}:${member.path}`;
    if (key === `${candidate.source_id}:${candidate.path}`) continue;
    const content = contentByKey.get(key) ?? "";
    if (!content) continue;
    if (content.includes(candidate.path) || content.includes(name)) references.push(key);
  }
  return references.sort();
}

function deterministicStatus(requirements, content) {
  if (requirements.network || requirements.process) return "external_state_dependent";
  if (
    requirements.environment ||
    /\b(?:Date\.now|new Date|time\.time|datetime\.|random\.|Math\.random)\b/.test(content)
  ) {
    return "environment_dependent";
  }
  return "deterministic_for_identical_inputs";
}

function platformAssumptions(content) {
  const values = [];
  if (/^#!\/usr\/bin\/env/m.test(content)) values.push("posix_env_shebang");
  if (/^#!\/bin\//m.test(content)) values.push("posix_absolute_shebang");
  if (/\b(?:win32|Windows|powershell|cmd\.exe)\b/i.test(content)) values.push("windows");
  if (/\b(?:darwin|macOS)\b/i.test(content)) values.push("macos");
  if (/\b(?:linux|\/proc\/|\/tmp\/)\b/i.test(content)) values.push("linux_or_posix");
  return values.length ? [...new Set(values)].sort() : ["platform_neutral_or_unspecified"];
}

function sourceDigest(manifest) {
  return sha256Text(`${JSON.stringify(manifest)}\n`);
}

function discover(manifest, sourceRoot) {
  const members = manifest.members ?? [];
  const contentByKey = new Map();
  for (const member of members) {
    contentByKey.set(
      `${member.source_id}:${member.path}`,
      readSourceText(sourceRoot, member),
    );
  }
  const candidates = members.filter((member) =>
    isCandidate(member, contentByKey.get(`${member.source_id}:${member.path}`) ?? ""),
  );
  return { members, candidates, contentByKey };
}

function buildInventory(manifest, sourceRoot) {
  const { members, candidates, contentByKey } = discover(manifest, sourceRoot);
  const entries = candidates
    .map((member) => {
      const memberKey = `${member.source_id}:${member.path}`;
      const content = contentByKey.get(memberKey) ?? "";
      const risk = riskAnalysis(content, member.path);
      const io = expectedIo(content, risk.requirements);
      const rule = ruleFor(member);
      return {
        member_key: memberKey,
        source_id: member.source_id,
        source_version:
          manifest.sources?.find((source) => source.id === member.source_id)?.version ?? null,
        path: member.path,
        sha256: member.sha256,
        source_class: member.class,
        language: languageFor(member.path, content),
        owner: ownerFor(member),
        discovery_signals: discoverySignals(member, content),
        imports: importsFor(content),
        ...io,
        requirements: risk.requirements,
        deterministic_status: deterministicStatus(risk.requirements, content),
        platform_assumptions: platformAssumptions(content),
        upstream_tests: findIndirectReferences(members, contentByKey, member).filter((key) =>
          /(?:^|\/)(?:test|tests)\//i.test(key),
        ),
        consumers: findIndirectReferences(members, contentByKey, member).filter(
          (key) => !/(?:^|\/)(?:test|tests)\//i.test(key),
        ),
        explicit_review_flags: risk.explicit_review_flags,
        treatment: rule.treatment,
        review: {
          status: "reviewed",
          owner: "Sapphirus Security Architecture",
          decision_rule: rule.id,
          rationale: rule.rationale,
        },
      };
    })
    .sort((left, right) => left.member_key.localeCompare(right.member_key));

  const counts = Object.fromEntries(
    [...allowedTreatments]
      .sort()
      .map((treatment) => [
        treatment,
        entries.filter((entry) => entry.treatment === treatment).length,
      ]),
  );
  return {
    schema_version: "sapphirus.bmad.script-inventory/v1",
    discovery_policy_version: "1.0.0",
    source_manifest_sha256: sourceDigest(manifest),
    source_archives: (manifest.sources ?? [])
      .map((source) => ({
        source_id: source.id,
        version: source.version,
        archive_sha256: source.archive_sha256,
      }))
      .sort((left, right) => left.source_id.localeCompare(right.source_id)),
    candidate_count: entries.length,
    treatment_counts: counts,
    authority_statement:
      "Retained source and generated instructions are untrusted data. Only typed plans approved by Airlock and executed through deterministic native ports can cause side effects.",
    entries,
  };
}

function listFiles(root) {
  if (!existsSync(root)) return [];
  if (statSync(root).isFile()) return [root];
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isSymbolicLink()) continue;
    if (entry.isDirectory()) {
      if (
        new Set([
          ".git",
          ".bmad100",
          "artifacts",
          "dist",
          "node_modules",
          "target",
          "test",
          "tests",
          "fixtures",
          "source",
        ]).has(entry.name)
      ) {
        continue;
      }
      files.push(...listFiles(path));
    } else if (entry.isFile() && textExtensions.has(extname(entry.name).toLowerCase())) {
      files.push(path);
    }
  }
  return files;
}

function runtimeScanRoots(runtimeRoot) {
  if (runtimeRoot) return [runtimeRoot];
  return [
    join(repositoryRoot, "packages"),
    join(repositoryRoot, "crates"),
    join(repositoryRoot, "apps"),
    join(repositoryRoot, "services"),
  ];
}

function validateRuntimeReferences(entries, runtimeRoot) {
  const prohibited = entries.filter(
    (entry) => entry.treatment === "reject" || !allowedTreatments.has(entry.treatment),
  );
  const errors = [];
  for (const root of runtimeScanRoots(runtimeRoot)) {
    for (const file of listFiles(root)) {
      const normalizedFile = normalizePath(relative(runtimeRoot ?? repositoryRoot, file));
      if (
        normalizedFile.endsWith("packages/bmad-foundation/script-inventory.json") ||
        normalizedFile.endsWith("packages/bmad-foundation/source-accounting.json") ||
        normalizedFile.endsWith("packages/bmad-foundation/source-manifest.json")
      ) {
        continue;
      }
      const content = readFileSync(file, "utf8").replaceAll("\\", "/");
      for (const entry of prohibited) {
        const full = `packages/bmad-foundation/source/${entry.source_id}/${entry.path}`;
        const local = `source/${entry.source_id}/${entry.path}`;
        if (content.includes(full) || content.includes(local)) {
          errors.push(
            `runtime references rejected script ${entry.member_key}: ${normalizedFile}`,
          );
        }
      }
    }
  }
  return errors;
}

function validateInventory(manifest, sourceRoot, inventory, runtimeRoot) {
  const errors = [];
  if (inventory.schema_version !== "sapphirus.bmad.script-inventory/v1") {
    errors.push(`unsupported script inventory schema: ${inventory.schema_version}`);
  }
  if (inventory.discovery_policy_version !== "1.0.0") {
    errors.push(`unsupported discovery policy: ${inventory.discovery_policy_version}`);
  }
  const { candidates } = discover(manifest, sourceRoot);
  const candidateByKey = new Map(
    candidates.map((member) => [`${member.source_id}:${member.path}`, member]),
  );
  const entries = inventory.entries ?? [];
  const seen = new Set();
  for (const entry of entries) {
    if (seen.has(entry.member_key)) {
      errors.push(`duplicate treatment decision: ${entry.member_key}`);
      continue;
    }
    seen.add(entry.member_key);
    const member = candidateByKey.get(entry.member_key);
    if (!member) {
      errors.push(`inventory entry is not a discovered executable: ${entry.member_key}`);
      continue;
    }
    if (entry.sha256 !== member.sha256) {
      errors.push(`source hash drift for ${entry.member_key}`);
    }
    if (!allowedTreatments.has(entry.treatment)) {
      errors.push(`invalid treatment for ${entry.member_key}: ${entry.treatment}`);
    }
    if (
      entry.review?.status !== "reviewed" ||
      !entry.review?.owner ||
      !entry.review?.rationale
    ) {
      errors.push(`unreviewed treatment decision: ${entry.member_key}`);
    }
  }
  for (const key of candidateByKey.keys()) {
    if (!seen.has(key)) {
      errors.push(`unclassified executable or code-generating member: ${key}`);
    }
  }
  errors.push(...validateRuntimeReferences(entries, runtimeRoot));
  return errors;
}

function compareCanonical(actual, expected) {
  return `${JSON.stringify(actual, null, 2)}\n` === `${JSON.stringify(expected, null, 2)}\n`;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const manifest = readJson(options.manifestPath, "source manifest");
  if (options.write) {
    const inventory = buildInventory(manifest, options.sourceRoot);
    writeFileSync(options.inventoryPath, `${JSON.stringify(inventory, null, 2)}\n`, "utf8");
    process.stdout.write(
      `wrote ${inventory.entries.length} reviewed script treatments to ${options.inventoryPath}\n`,
    );
    return;
  }

  const inventory = readJson(options.inventoryPath, "script inventory");
  const errors = validateInventory(
    manifest,
    options.sourceRoot,
    inventory,
    options.runtimeRoot,
  );
  if (
    options.strict &&
    options.manifestPath === join(packageRoot, "source-manifest.json") &&
    !compareCanonical(inventory, buildInventory(manifest, options.sourceRoot))
  ) {
    errors.push(
      "script inventory drift: regenerate with node packages/bmad-foundation/scripts/classify-scripts.mjs --write",
    );
  }
  if (errors.length) {
    for (const error of errors) process.stderr.write(`ERROR: ${error}\n`);
    process.exitCode = 1;
    return;
  }
  process.stdout.write(
    `script classification verified: ${inventory.entries.length} reviewed treatments; no prohibited runtime references\n`,
  );
}

try {
  main();
} catch (error) {
  process.stderr.write(`ERROR: ${error.message}\n`);
  process.exitCode = 1;
}
