import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import {
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));

const runtimePaths = Object.freeze([
  "runtime/builder/2.1.0/agent-analyze.instructions.md",
  "runtime/builder/2.1.0/agent-create-rebuild.instructions.md",
  "runtime/builder/2.1.0/agent-edit.instructions.md",
  "runtime/builder/2.1.0/workflow-analyze.instructions.md",
  "runtime/builder/2.1.0/workflow-build-edit.instructions.md",
  "runtime/method/6.10.0/analyst-persona.instructions.md",
  "runtime/method/6.10.0/architect-persona.instructions.md",
  "runtime/method/6.10.0/architecture-create.instructions.md",
  "runtime/method/6.10.0/bmad-help.instructions.md",
  "runtime/method/6.10.0/brainstorming.instructions.md",
  "runtime/method/6.10.0/code-review.instructions.md",
  "runtime/method/6.10.0/correct-course.instructions.md",
  "runtime/method/6.10.0/create-epics-and-stories.instructions.md",
  "runtime/method/6.10.0/create-story.instructions.md",
  "runtime/method/6.10.0/dev-persona.instructions.md",
  "runtime/method/6.10.0/dev-story.instructions.md",
  "runtime/method/6.10.0/document-project.instructions.md",
  "runtime/method/6.10.0/domain-research.instructions.md",
  "runtime/method/6.10.0/explain-concept.instructions.md",
  "runtime/method/6.10.0/implementation-readiness.instructions.md",
  "runtime/method/6.10.0/market-research.instructions.md",
  "runtime/method/6.10.0/mermaid-gen.instructions.md",
  "runtime/method/6.10.0/pm-persona.instructions.md",
  "runtime/method/6.10.0/prd.instructions.md",
  "runtime/method/6.10.0/prfaq.instructions.md",
  "runtime/method/6.10.0/product-brief.instructions.md",
  "runtime/method/6.10.0/qa-tests.instructions.md",
  "runtime/method/6.10.0/quick-dev.instructions.md",
  "runtime/method/6.10.0/retrospective.instructions.md",
  "runtime/method/6.10.0/sprint-planning.instructions.md",
  "runtime/method/6.10.0/tech-writer-persona.instructions.md",
  "runtime/method/6.10.0/technical-research.instructions.md",
  "runtime/method/6.10.0/ux-design.instructions.md",
  "runtime/method/6.10.0/ux-designer-persona.instructions.md",
  "runtime/method/6.10.0/validate-doc.instructions.md",
  "runtime/method/6.10.0/write-document.instructions.md",
]);

const normalizedPaths = Object.freeze([
  "normalized/bmad-analyst.package.json",
  "normalized/bmad-architect.package.json",
  "normalized/bmad-architecture.package.json",
  "normalized/bmad-brainstorming.package.json",
  "normalized/bmad-check-implementation-readiness.package.json",
  "normalized/bmad-code-review.package.json",
  "normalized/bmad-correct-course.package.json",
  "normalized/bmad-create-epics-and-stories.package.json",
  "normalized/bmad-create-story.package.json",
  "normalized/bmad-dev-story.package.json",
  "normalized/bmad-dev.package.json",
  "normalized/bmad-document-project.package.json",
  "normalized/bmad-domain-research.package.json",
  "normalized/bmad-help-action-graph.json",
  "normalized/bmad-help.package.json",
  "normalized/bmad-market-research.package.json",
  "normalized/bmad-pm.package.json",
  "normalized/bmad-prd.package.json",
  "normalized/bmad-prfaq.package.json",
  "normalized/bmad-product-brief.package.json",
  "normalized/bmad-qa-generate-e2e-tests.package.json",
  "normalized/bmad-quick-dev.package.json",
  "normalized/bmad-retrospective.package.json",
  "normalized/bmad-sprint-planning.package.json",
  "normalized/bmad-tech-writer-explain-concept.package.json",
  "normalized/bmad-tech-writer-mermaid-gen.package.json",
  "normalized/bmad-tech-writer-validate-doc.package.json",
  "normalized/bmad-tech-writer-write-document.package.json",
  "normalized/bmad-tech-writer.package.json",
  "normalized/bmad-technical-research.package.json",
  "normalized/bmad-ux-designer.package.json",
  "normalized/bmad-ux.package.json",
  "normalized/bmm-agent-roster.json",
  "normalized/builder-agent.package.json",
  "normalized/builder-workflow.package.json",
]);

const managedOutputPaths = Object.freeze([
  "NOTICE.md",
  "adoption-ledger.json",
  "licenses/BMAD-BUILDER-MIT.txt",
  "licenses/BMAD-METHOD-MIT.txt",
  ...runtimePaths,
].sort());

const requiredPackagePaths = Object.freeze([
  "README.md",
  "runtime-manifest.json",
  "semantic-source-ledger.json",
  "scripts/verify.mjs",
  ...normalizedPaths,
  ...managedOutputPaths,
].sort());

const packageDistributionFiles = Object.freeze([
  "adoption-ledger.json",
  "capability-closure-ledger.json",
  "semantic-source-ledger.json",
  "NOTICE.md",
  "licenses",
  "normalized",
  "runtime",
  "runtime-manifest.json",
  "scripts",
  "tests",
  "README.md",
]);

const sourceFacts = Object.freeze({
  method: Object.freeze({
    packageName: "bmad-method",
    packageVersion: "6.10.0",
    sourceFormatVersion: null,
    sourceFormatVersionEvidence: "not_declared",
    runtimeCompatibility: Object.freeze({ node: ">=20.12.0" }),
    archiveSha256:
      "a7c049038099b99081fbd03d22c6a5180edd88dee656bb37c4276b1cc31b4a32",
  }),
  builder: Object.freeze({
    packageName: "bmad-builder",
    packageVersion: "2.1.0",
    moduleVersion: "1.0.0",
    sourceFormatVersion: null,
    sourceFormatVersionEvidence: "not_declared",
    runtimeCompatibility: Object.freeze({ node: ">=22.0.0" }),
    archiveSha256:
      "d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852",
  }),
});

const expectedTreatmentDecisionSets = Object.freeze({
  "method-001": ["adopt", "adapt"],
  "method-002": ["adopt", "reject"],
  "method-003": ["adopt", "reject"],
  "method-004": ["adopt"],
  "method-005": ["adopt"],
  "method-006": ["adopt", "adapt"],
  "method-007": ["adapt", "reject"],
  "method-008": ["adopt", "adapt"],
  "method-009": ["adapt", "reject"],
  "method-010": ["adopt", "adapt"],
  "method-011": ["adopt", "adapt"],
  "method-012": ["adopt", "adapt"],
  "method-013": ["adopt", "adapt"],
  "method-014": ["adopt", "adapt"],
  "method-015": ["adapt", "reject"],
  "method-016": ["adopt", "adapt"],
  "method-017": ["adapt", "reject"],
  "method-018": ["adopt", "adapt"],
  "method-019": ["adopt", "adapt", "reject"],
  "method-020": ["adopt", "adapt"],
  "method-021": ["adapt", "reject"],
  "method-022": ["adapt"],
  "method-023": ["adopt", "adapt", "reject"],
  "method-024": ["adapt", "reject"],
  "method-025": ["adapt", "reject"],
  "method-026": ["adapt", "reject"],
  "method-027": ["adapt", "reject"],
  "method-028": ["adapt"],
  "method-029": ["reject"],
  "method-030": ["adopt", "adapt"],
  "method-031": ["adopt", "adapt"],
  "method-032": ["adopt", "adapt"],
  "method-033": ["adopt", "adapt"],
  "method-034": ["adopt", "adapt"],
  "method-035": ["adopt", "adapt"],
  "method-036": ["adopt", "adapt"],
  "method-037": ["adopt", "adapt"],
  "method-038": ["adopt", "adapt"],
  "method-039": ["adopt", "adapt"],
  "method-040": ["adopt", "adapt"],
  "method-041": ["adopt", "adapt"],
  "method-042": ["adopt", "adapt"],
  "method-043": ["adopt", "adapt"],
  "method-044": ["adopt", "adapt"],
  "method-045": ["adopt", "adapt"],
  "method-046": ["adopt", "adapt"],
  "method-047": ["adopt", "adapt"],
  "method-048": ["adopt", "adapt"],
  "method-049": ["adopt", "adapt"],
  "method-050": ["adopt", "adapt"],
  "method-051": ["adopt", "adapt"],
  "method-052": ["adopt", "adapt"],
  "method-053": ["adopt", "adapt"],
  "method-054": ["adopt", "adapt"],
  "method-055": ["adopt", "adapt"],
  "method-056": ["adopt", "adapt"],
  "method-057": ["adopt", "adapt"],
  "method-058": ["adopt", "adapt"],
  "method-059": ["adopt", "adapt"],
  "method-060": ["adopt", "adapt"],
  "method-061": ["adopt", "adapt"],
  "method-062": ["adopt", "adapt"],
  "method-063": ["adopt", "adapt"],
  "method-064": ["adopt", "adapt"],
  "method-065": ["adopt", "adapt"],
  "method-066": ["adopt", "adapt"],
  "method-067": ["adopt", "adapt"],
  "method-064": ["adopt", "adapt"],
  "method-065": ["adopt", "adapt"],
  "method-066": ["adopt", "adapt"],
  "method-067": ["adopt", "adapt"],
  "builder-001": ["adopt"],
  "builder-002": ["adopt", "reject"],
  "builder-003": ["adopt", "adapt"],
  "builder-004": ["adopt", "adapt", "reject"],
  "builder-005": ["adapt", "reject"],
  "builder-006": ["adopt", "adapt", "defer", "reject"],
  "builder-007": ["adapt", "reject"],
  "builder-008": ["adopt", "defer"],
  "builder-009": ["adopt"],
  "builder-010": ["adopt", "defer", "reject"],
  "builder-011": ["adopt", "reject"],
  "builder-012": ["adapt"],
  "builder-013": ["adapt", "reject"],
  "builder-014": ["adapt", "reject"],
  "builder-015": ["defer"],
  "builder-016": ["adapt", "reject"],
  "builder-017": ["adopt", "adapt", "reject"],
  "builder-018": ["adopt", "adapt", "reject"],
  "builder-019": ["adapt", "reject"],
  "builder-020": ["adapt", "reject"],
  "builder-021": ["adopt"],
  "builder-022": ["adopt", "reject"],
  "builder-023": ["adopt", "reject"],
  "builder-024": ["adapt"],
  "builder-025": ["adopt", "reject"],
  "builder-026": ["adapt", "reject"],
  "builder-027": ["adapt", "defer", "reject"],
  "builder-028": ["adapt", "defer", "reject"],
  "builder-029": ["adapt", "defer", "reject"],
  "builder-030": ["adapt", "defer", "reject"],
  "builder-031": ["adapt", "defer", "reject"],
  "builder-032": ["adapt", "defer", "reject"],
  "builder-033": ["adapt", "reject"],
  "builder-034": ["adapt", "reject"],
  "builder-035": ["adapt", "reject"],
  "builder-036": ["adapt", "reject"],
  "builder-037": ["adapt", "defer", "reject"],
  "builder-038": ["adapt", "defer"],
  "builder-039": ["adapt", "defer", "reject"],
  "builder-040": ["adapt", "reject"],
  "builder-041": ["adapt", "reject"],
  "builder-042": ["defer", "reject"],
  "builder-043": ["defer", "reject"],
  "builder-044": ["adapt", "defer", "reject"],
  "builder-045": ["adapt", "reject"],
  "builder-046": ["adapt", "reject"],
  "builder-047": ["adapt", "reject"],
});

const expectedProjectionSourceMemberIds = Object.freeze({
  "runtime/method/6.10.0/document-project.instructions.md": [
    "method-004",
    "method-064",
    "method-065",
  ],
  "runtime/method/6.10.0/explain-concept.instructions.md": [
    "method-004",
    "method-013",
    "method-009",
  ],
  "runtime/method/6.10.0/mermaid-gen.instructions.md": [
    "method-004",
    "method-011",
    "method-009",
  ],
  "runtime/method/6.10.0/prd.instructions.md": [
    "method-004",
    "method-066",
    "method-067",
  ],
  "runtime/method/6.10.0/validate-doc.instructions.md": [
    "method-004",
    "method-012",
    "method-009",
  ],
  "runtime/method/6.10.0/write-document.instructions.md": [
    "method-004",
    "method-010",
    "method-009",
  ],
  "runtime/method/6.10.0/document-project.instructions.md": [
    "method-004",
    "method-064",
    "method-065",
  ],
  "runtime/method/6.10.0/prd.instructions.md": [
    "method-004",
    "method-066",
    "method-067",
  ],
  "runtime/method/6.10.0/code-review.instructions.md": [
    "method-004",
    "method-056",
    "method-057",
  ],
  "runtime/method/6.10.0/correct-course.instructions.md": [
    "method-004",
    "method-046",
    "method-047",
  ],
  "runtime/method/6.10.0/create-epics-and-stories.instructions.md": [
    "method-004",
    "method-042",
    "method-043",
  ],
  "runtime/method/6.10.0/create-story.instructions.md": [
    "method-004",
    "method-060",
    "method-061",
  ],
  "runtime/method/6.10.0/dev-story.instructions.md": [
    "method-004",
    "method-050",
    "method-051",
  ],
  "runtime/method/6.10.0/implementation-readiness.instructions.md": [
    "method-004",
    "method-044",
    "method-045",
  ],
  "runtime/method/6.10.0/qa-tests.instructions.md": [
    "method-004",
    "method-054",
    "method-055",
  ],
  "runtime/method/6.10.0/quick-dev.instructions.md": [
    "method-004",
    "method-052",
    "method-053",
  ],
  "runtime/method/6.10.0/retrospective.instructions.md": [
    "method-004",
    "method-062",
    "method-063",
  ],
  "runtime/method/6.10.0/sprint-planning.instructions.md": [
    "method-004",
    "method-058",
    "method-059",
  ],
  "runtime/method/6.10.0/ux-design.instructions.md": [
    "method-004",
    "method-048",
    "method-049",
  ],
  "runtime/method/6.10.0/brainstorming.instructions.md": [
    "method-004",
    "method-030",
    "method-031",
  ],
  "runtime/method/6.10.0/domain-research.instructions.md": [
    "method-004",
    "method-034",
    "method-035",
  ],
  "runtime/method/6.10.0/market-research.instructions.md": [
    "method-004",
    "method-032",
    "method-033",
  ],
  "runtime/method/6.10.0/prfaq.instructions.md": [
    "method-004",
    "method-040",
    "method-041",
  ],
  "runtime/method/6.10.0/product-brief.instructions.md": [
    "method-004",
    "method-038",
    "method-039",
  ],
  "runtime/method/6.10.0/technical-research.instructions.md": [
    "method-004",
    "method-036",
    "method-037",
  ],
  "runtime/method/6.10.0/bmad-help.instructions.md": [
    "method-001",
    "method-002",
    "method-003",
    "method-004",
    "method-005",
  ],
  "runtime/method/6.10.0/architect-persona.instructions.md": [
    "method-004",
    "method-018",
    "method-019",
  ],
  "runtime/method/6.10.0/analyst-persona.instructions.md": [
    "method-004",
    "method-006",
    "method-007",
  ],
  "runtime/method/6.10.0/tech-writer-persona.instructions.md": [
    "method-004",
    "method-008",
    "method-009",
  ],
  "runtime/method/6.10.0/pm-persona.instructions.md": [
    "method-004",
    "method-014",
    "method-015",
  ],
  "runtime/method/6.10.0/ux-designer-persona.instructions.md": [
    "method-004",
    "method-016",
    "method-017",
  ],
  "runtime/method/6.10.0/dev-persona.instructions.md": [
    "method-004",
    "method-020",
    "method-021",
  ],
  "runtime/method/6.10.0/architecture-create.instructions.md": [
    "method-018",
    "method-019",
    "method-022",
    "method-023",
    "method-024",
    "method-025",
    "method-026",
    "method-027",
    "method-028",
    "method-029",
  ],
  "runtime/builder/2.1.0/agent-create-rebuild.instructions.md": [
    "builder-003",
    "builder-004",
    "builder-005",
    "builder-008",
    "builder-009",
    "builder-010",
    "builder-011",
    "builder-013",
    "builder-014",
    "builder-015",
    "builder-016",
    "builder-038",
    "builder-039",
    "builder-040",
    "builder-041",
    "builder-042",
    "builder-043",
  ],
  "runtime/builder/2.1.0/agent-edit.instructions.md": [
    "builder-003",
    "builder-004",
    "builder-006",
    "builder-008",
    "builder-009",
    "builder-010",
    "builder-011",
    "builder-013",
    "builder-014",
    "builder-016",
  ],
  "runtime/builder/2.1.0/agent-analyze.instructions.md": [
    "builder-003",
    "builder-004",
    "builder-007",
    "builder-009",
    "builder-011",
    "builder-012",
    "builder-027",
    "builder-028",
    "builder-029",
    "builder-030",
    "builder-031",
    "builder-032",
    "builder-040",
    "builder-041",
  ],
  "runtime/builder/2.1.0/workflow-build-edit.instructions.md": [
    "builder-017",
    "builder-018",
    "builder-019",
    "builder-021",
    "builder-022",
    "builder-023",
    "builder-025",
    "builder-026",
    "builder-044",
    "builder-045",
    "builder-046",
  ],
  "runtime/builder/2.1.0/workflow-analyze.instructions.md": [
    "builder-017",
    "builder-018",
    "builder-020",
    "builder-021",
    "builder-023",
    "builder-024",
    "builder-033",
    "builder-034",
    "builder-035",
    "builder-036",
    "builder-037",
    "builder-046",
    "builder-047",
  ],
});

const criticalMembers = Object.freeze({
  "method:src/core-skills/bmad-help/SKILL.md":
    "718077d741e20d9c94f3c2b7827047f2d18a90b85c3cc2eecd449e28b7b0d642",
  "method:src/bmm-skills/1-analysis/bmad-agent-tech-writer/write-document.md":
    "c0ddfd981f765b82cba0921dad331cd1fa32bacdeea1f02320edfd60a0ae7e6f",
  "method:src/bmm-skills/1-analysis/bmad-agent-tech-writer/mermaid-gen.md":
    "1d83fcc5fa842bc31ecd9fd7e45fbf013fabcadf0022d3391fff5b53b48e4b5d",
  "method:src/bmm-skills/1-analysis/bmad-agent-tech-writer/validate-doc.md":
    "3b8d25f60be191716266726393f2d44b77262301b785a801631083b610d6acc5",
  "method:src/bmm-skills/1-analysis/bmad-agent-tech-writer/explain-concept.md":
    "6ea82dbe4e41d4bb8880cbaa62d936e40cef18f8c038be73ae6e09c462abafc9",
  "builder:skills/bmad-agent-builder/SKILL.md":
    "806ea0a5c3bd9d4ef5dfa2e0beb37490b0fb3faef848ac493db2db0e99f32dda",
  "builder:skills/bmad-agent-builder/assets/SKILL-template.md":
    "a4682113f512dccb2aa092ece9b27eabf57ce1ce1db13d2e3406c5b1fcd4234d",
  "builder:skills/bmad-agent-builder/assets/customize-template.toml":
    "e14b5f3e579b3f6286e9255aa5c6f31ac747bd9512075d30586c7d84ca55938d",
  "builder:skills/bmad-agent-builder/assets/capability-authoring-template.md":
    "c26b1cacea3f6c05c70984d4a8a96f7e44e7828962e19a79b2e9ebe54ebd6b4a",
  "builder:skills/bmad-agent-builder/assets/prompt-quality-canon.md":
    "a9c27fa16aa95a62503a9199397714a4a3b365e55235cab06143e73739d16acf",
  "builder:skills/bmad-workflow-builder/SKILL.md":
    "ed28d89b38b1821fce92e09845e94300a4b3d2ec94e8ce7e86e8fa6fe170a644",
});

const licenseSha256 =
  "0aa79baf6328b4a1e694ce10a12ffc36d7666554da128dff0e8fcda0fc536a66";
const contextMarkers = Object.freeze([
  ["bmad", "runtime", "lib"].join("-"),
  ["", "source", "review"].join("_"),
]);
const executableRuntimeName =
  /(?:\.(?:bat|cjs|cmd|dll|exe|js|mjs|ps1|py|ts)|(?:^|[-_.])(?:cleanup|eval|hook|install|render|setup|wake)(?:[-_.]|$))/iu;
const executableRuntimeContent =
  /(?:^#!|```\s*(?:bash|cmd|javascript|js|node|powershell|python|sh|typescript)|\b(?:child_process|npm\s+install|pnpm\s+install|python\s+-m|uv\s+run)\b)/imu;

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function assertFailureCode(result, expected) {
  assert.notEqual(result.code, 0);
  const actual = /^\[([a-z_]+)\]/u.exec(result.stderr)?.[1];
  assert.equal(actual, expected, result.stderr);
}

async function readJson(relativePath) {
  const bytes = await readFile(path.join(packageRoot, relativePath));
  return JSON.parse(bytes.toString("utf8"));
}

async function assertRegularFile(relativePath) {
  const absolutePath = path.join(packageRoot, relativePath);
  const metadata = await lstat(absolutePath);
  assert.equal(metadata.isFile(), true, `${relativePath} must be a regular file`);
  assert.equal(metadata.isSymbolicLink(), false, `${relativePath} must not be a link`);
  const resolved = await realpath(absolutePath);
  const relativeResolved = path.relative(packageRoot, resolved);
  assert.equal(
    relativeResolved.startsWith("..") || path.isAbsolute(relativeResolved),
    false,
    `${relativePath} must resolve inside the package`,
  );
}

async function runVerifier(cwd) {
  return await new Promise((resolve) => {
    const child = spawn(process.execPath, ["./scripts/verify.mjs"], {
      cwd,
      env: {},
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", (error) => {
      resolve({ code: null, stdout, stderr: `${stderr}${error.message}` });
    });
    child.on("close", (code) => {
      resolve({ code, stdout, stderr });
    });
  });
}

async function copyPackage() {
  const temporaryRoot = await mkdtemp(path.join(tmpdir(), "sapphirus-bmad-foundation-"));
  const destination = path.join(temporaryRoot, "package");
  await cp(packageRoot, destination, {
    recursive: true,
    dereference: false,
    filter(source) {
      return path.basename(source) !== "node_modules";
    },
  });
  return { destination, temporaryRoot };
}

test("the foundation declares every reviewed repository-owned output", async (t) => {
  for (const relativePath of requiredPackagePaths) {
    await t.test(relativePath, async () => {
      await assertRegularFile(relativePath);
    });
  }
});

test("the BMAD-04 package topology contains only reviewed normalized runtime data", async () => {
  const manifest = await readJson("package.json");
  assert.deepEqual(manifest.files, packageDistributionFiles);
  const rootEntries = await readdir(packageRoot, { withFileTypes: true });
  assert.deepEqual(
    rootEntries.map((entry) => entry.name).sort(),
    [
      "NOTICE.md",
      "README.md",
      "adoption-ledger.json",
      "capability-closure-ledger.json",
      "licenses",
      "normalized",
      "package.json",
      "runtime",
      "runtime-manifest.json",
      "scripts",
      "semantic-source-ledger.json",
      "tests",
    ],
  );
  assert.ok(rootEntries.every((entry) => !entry.isSymbolicLink()));
  assert.deepEqual(
    await readdir(path.join(packageRoot, "normalized")),
    normalizedPaths.map((relativePath) => path.posix.basename(relativePath)),
  );
  assert.deepEqual(await readdir(path.join(packageRoot, "scripts")), ["verify.mjs"]);
  assert.deepEqual(await readdir(path.join(packageRoot, "tests")), ["foundation.test.mjs"]);
});

test("the runtime manifest binds every bundled byte and excludes development-only files", async () => {
  const manifest = await readJson("runtime-manifest.json");
  assert.equal(manifest.schemaVersion, "sapphirus.bmad-foundation-runtime-manifest.v1");
  assert.equal(manifest.foundationVersion, "0.1.0-beta.1");
  assert.deepEqual(
    manifest.resources.map(({ path: resourcePath }) => resourcePath),
    [...manifest.resources.map(({ path: resourcePath }) => resourcePath)].sort(),
  );
  assert.ok(manifest.resources.every(({ path: resourcePath }) =>
    !resourcePath.startsWith("scripts/")
    && !resourcePath.startsWith("tests/")
    && !resourcePath.includes(["bmad", "runtime", "lib"].join("-"))));
  for (const resource of manifest.resources) {
    const bytes = await readFile(path.join(packageRoot, ...resource.path.split("/")));
    assert.equal(bytes.byteLength, resource.byteLength, resource.path);
    assert.equal(`sha256:${createHash("sha256").update(bytes).digest("hex")}`, resource.contentHash, resource.path);
  }
});

test("the semantic ledger locks source identity, licenses, and managed bytes", async () => {
  const ledger = await readJson("semantic-source-ledger.json");
  assert.equal(ledger.schemaVersion, "sapphirus.bmad.semantic-source-ledger/v1");

  const sources = new Map(ledger.sources.map((source) => [source.id, source]));
  assert.deepEqual([...sources.keys()].sort(), ["builder", "method"]);
  for (const [id, expected] of Object.entries(sourceFacts)) {
    const actual = sources.get(id);
    assert.ok(actual, `missing source ${id}`);
    for (const [key, value] of Object.entries(expected)) {
      assert.deepEqual(actual[key], value, `${id}.${key}`);
    }
    assert.equal(actual.gitIdentity, null, `${id} must not guess a Git identity`);
    assert.equal(actual.promotionEligibility, "blocked_provenance");
    assert.ok(Array.isArray(actual.missingImmutableIdentity));
    assert.ok(actual.missingImmutableIdentity.length > 0);
  }

  const members = new Map(
    ledger.sourceMembers.map((member) => [
      `${member.sourceId}:${member.member}`,
      member,
    ]),
  );
  assert.equal(members.size, ledger.sourceMembers.length, "source member identities must be unique");
  assert.equal(members.size, 114, "the reviewed source set must be exact and closed");
  assert.deepEqual(
    Object.fromEntries(
      ledger.sourceMembers.map((member) => [
        member.id,
        member.treatments.map((item) => item.decision),
      ]),
    ),
    expectedTreatmentDecisionSets,
  );
  for (const [identity, expectedHash] of Object.entries(criticalMembers)) {
    const member = members.get(identity);
    assert.ok(member, `missing source member ${identity}`);
    assert.equal(member.sha256, expectedHash, `${identity} digest`);
    assert.ok(Array.isArray(member.treatments), `${identity} treatments`);
    assert.ok(member.treatments.length > 0, `${identity} treatments`);
    for (const treatment of member.treatments) {
      assert.match(treatment.decision, /^(?:adopt|adapt|defer|reject)$/u);
      assert.match(treatment.rationale, /\S/u);
    }
  }
  // ADR-0005 + the 2026-07-20 owner approval superseded the ADR-0003
  // exclusion: the tech-writer action sources are first-party approved.
  for (const identity of Object.keys(criticalMembers).filter((value) =>
    value.includes("bmad-agent-tech-writer"),
  )) {
    const decisions = members.get(identity).treatments.map((t) => t.decision);
    assert.ok(
      !decisions.includes("defer"),
      `${identity} deferral was resolved by the reviewed approval`,
    );
  }

  assert.deepEqual(
    ledger.licenses,
    [
      {
        sourceId: "builder",
        path: "licenses/BMAD-BUILDER-MIT.txt",
        sha256: licenseSha256,
      },
      {
        sourceId: "method",
        path: "licenses/BMAD-METHOD-MIT.txt",
        sha256: licenseSha256,
      },
    ],
  );

  const outputs = [...ledger.managedOutputs].sort((left, right) =>
    left.path < right.path ? -1 : left.path > right.path ? 1 : 0,
  );
  assert.deepEqual(outputs.map((output) => output.path), managedOutputPaths);
  for (const output of outputs) {
    const bytes = await readFile(path.join(packageRoot, output.path));
    assert.equal(output.byteLength, bytes.byteLength, `${output.path} byte length`);
    assert.equal(output.sha256, sha256(bytes), `${output.path} digest`);
  }
});

test("the adoption ledger closes every citation and grants no runtime authority", async () => {
  const [semantic, adoption] = await Promise.all([
    readJson("semantic-source-ledger.json"),
    readJson("adoption-ledger.json"),
  ]);
  assert.equal(adoption.schemaVersion, "sapphirus.bmad.adoption-ledger/v1");
  assert.equal(adoption.operationalAuthority, "none");
  assert.equal(adoption.promotionEligibility, "blocked_provenance");
  assert.equal(adoption.trademarkDecision.status, "product_naming_not_approved");
  assert.match(adoption.trademarkDecision.rationale, /\S/u);

  const memberIds = new Set(semantic.sourceMembers.map((member) => member.id));
  const decisions = new Map(
    adoption.sourceDecisions.map((decision) => [decision.sourceMemberId, decision]),
  );
  assert.deepEqual([...decisions.keys()].sort(), [...memberIds].sort());
  for (const decision of decisions.values()) {
    assert.ok(Array.isArray(decision.treatments));
    assert.ok(decision.treatments.length > 0);
    for (const treatment of decision.treatments) {
      assert.match(treatment.decision, /^(?:adopt|adapt|defer|reject)$/u);
      assert.match(treatment.rationale, /\S/u);
    }
  }

  assert.deepEqual(
    adoption.methodRoster.map(({ code, name, title, state }) => ({ code, name, title, state })),
    [
      {
        code: "bmad-agent-analyst",
        name: "Mary",
        title: "Business Analyst",
        state: "managed_projection_inactive",
      },
      {
        code: "bmad-agent-tech-writer",
        name: "Paige",
        title: "Technical Writer",
        state: "managed_projection_inactive",
      },
      {
        code: "bmad-agent-pm",
        name: "John",
        title: "Product Manager",
        state: "managed_projection_inactive",
      },
      {
        code: "bmad-agent-ux-designer",
        name: "Sally",
        title: "UX Designer",
        state: "managed_projection_inactive",
      },
      {
        code: "bmad-agent-architect",
        name: "Winston",
        title: "System Architect",
        state: "managed_projection_inactive",
      },
      {
        code: "bmad-agent-dev",
        name: "Amelia",
        title: "Senior Software Engineer",
        state: "managed_projection_inactive",
      },
    ],
  );
  for (const rosterEntry of adoption.methodRoster) {
    assert.ok(Array.isArray(rosterEntry.sourceMemberIds));
    assert.ok(rosterEntry.sourceMemberIds.length > 0);
    assert.ok(rosterEntry.sourceMemberIds.every((id) => memberIds.has(id)));
  }
  assert.deepEqual(
    adoption.methodRoster.map((entry) => entry.sourceMemberIds),
    [
      ["method-004", "method-006", "method-007"],
      ["method-004", "method-008", "method-009"],
      ["method-004", "method-014", "method-015"],
      ["method-004", "method-016", "method-017"],
      ["method-004", "method-018", "method-019"],
      ["method-004", "method-020", "method-021"],
    ],
  );

  const projections = adoption.runtimeProjections;
  assert.deepEqual(
    [...projections].map((projection) => projection.path).sort(),
    runtimePaths,
  );
  for (const projection of projections) {
    assert.equal(projection.authority, "none", `${projection.path} authority`);
    assert.ok(Array.isArray(projection.sourceMemberIds));
    assert.ok(projection.sourceMemberIds.length > 0);
    for (const sourceMemberId of projection.sourceMemberIds) {
      assert.equal(memberIds.has(sourceMemberId), true, `${projection.path} source closure`);
    }
    assert.deepEqual(
      projection.sourceMemberIds,
      expectedProjectionSourceMemberIds[projection.path],
      `${projection.path} exact source closure`,
    );
    assert.equal(
      projection.classification,
      {
        BuilderAgentV2Stateless: "builder_agent",
        BuilderOutcomeSkillV2: "builder_workflow",
        MethodOfficialSkillV6: "method",
      }[projection.sourceIdentity.profile],
      `${projection.path} explicit source classification`,
    );
    assert.doesNotMatch(
      JSON.stringify(projection),
      /\b(?:activation|child_process|command|evaluation|network|promotion|registration|script)\b/iu,
      `${projection.path} must not contain an authority-bearing projection field`,
    );
  }

  const method = projections.filter((projection) => projection.classification === "method");
  assert.equal(method.length, 31);
  assert.ok(method.every((projection) => projection.state === "sealed_read_only"));

  const agentActions = projections
    .filter((projection) => projection.classification === "builder_agent")
    .flatMap((projection) => projection.actions);
  assert.deepEqual(agentActions, ["create_rebuild", "edit", "analyze"]);
  assert.ok(
    projections
      .filter((projection) => projection.classification === "builder_agent")
      .every((projection) => projection.state === "inactive_data"),
  );

  const workflowActions = projections
    .filter((projection) => projection.classification === "builder_workflow")
    .flatMap((projection) => projection.actions);
  assert.deepEqual(workflowActions, ["build", "edit", "analyze"]);
  assert.ok(
    projections
      .filter((projection) => projection.classification === "builder_workflow")
      .every((projection) => projection.state === "inactive_data"),
  );
});

test("Method help preserves all evidence-confidence classes without promoting inference", async () => {
  const help = await readFile(
    path.join(packageRoot, "runtime/method/6.10.0/bmad-help.instructions.md"),
    "utf8",
  );
  for (const confidence of [
    "authoritative",
    "user-asserted",
    "heuristic",
    "contextual",
    "unknown",
  ]) {
    assert.match(help, new RegExp(`\\b${confidence}\\b`, "u"), `${confidence} confidence`);
  }
  assert.match(help, /Never promote heuristic or contextual evidence to authoritative completion/u);
  assert.doesNotMatch(help, /Never infer completion/u);
});

test("runtime data has an exact non-executable allowlist", async () => {
  const runtimeRoot = path.join(packageRoot, "runtime");
  const found = [];
  async function walk(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const absolutePath = path.join(directory, entry.name);
      const relativePath = path.relative(packageRoot, absolutePath).replaceAll("\\", "/");
      assert.equal(entry.isSymbolicLink(), false, `${relativePath} must not be a link`);
      if (entry.isDirectory()) await walk(absolutePath);
      else {
        assert.equal(entry.isFile(), true, `${relativePath} must be a regular file`);
        found.push(relativePath);
        assert.doesNotMatch(entry.name, executableRuntimeName, relativePath);
        const source = await readFile(absolutePath, "utf8");
        assert.doesNotMatch(source, executableRuntimeContent, relativePath);
      }
    }
  }
  await walk(runtimeRoot);
  assert.deepEqual(found.sort(), runtimePaths);
});

test("workflow classification uses explicit source identity rather than a path heuristic", async () => {
  const verifierUrl = pathToFileURL(path.join(packageRoot, "scripts", "verify.mjs"));
  verifierUrl.searchParams.set("test", String(Date.now()));
  const { classifyProjectionFromSourceIdentity } = await import(verifierUrl.href);
  assert.equal(typeof classifyProjectionFromSourceIdentity, "function");
  assert.equal(
    classifyProjectionFromSourceIdentity({
      sourceId: "builder",
      skill: "bmad-workflow-builder",
      profile: "BuilderOutcomeSkillV2",
      member: "misleading/agent-builder/path.md",
    }),
    "builder_workflow",
  );
  assert.equal(
    classifyProjectionFromSourceIdentity({
      sourceId: "builder",
      skill: "bmad-agent-builder",
      profile: "BuilderAgentV2Stateless",
      member: "misleading/workflow-builder/path.md",
    }),
    "builder_agent",
  );
  assert.throws(
    () =>
      classifyProjectionFromSourceIdentity({
        sourceId: "builder",
        skill: "unknown",
        profile: "unknown",
        member: "bmad-workflow-builder/SKILL.md",
      }),
    /source identity/iu,
  );
});

test("verification relocates with the package and fails closed on tampering", async (t) => {
  const { destination, temporaryRoot } = await copyPackage();
  try {
    await t.test("minimal checkout verifies without external context", async () => {
      const result = await runVerifier(destination);
      assert.equal(result.code, 0, `${result.stdout}\n${result.stderr}`);
      const files = await readdir(destination, { recursive: true });
      for (const relativePath of files) {
        const absolutePath = path.join(destination, relativePath);
        const metadata = await lstat(absolutePath);
        if (!metadata.isFile()) continue;
        const source = await readFile(absolutePath, "utf8");
        for (const marker of contextMarkers) {
          assert.equal(source.includes(marker), false, `${relativePath} contains an external marker`);
        }
      }
    });

    await t.test("managed-output byte drift is rejected", async () => {
      const target = path.join(destination, runtimePaths[0]);
      const original = await readFile(target);
      try {
        await writeFile(target, Buffer.concat([original, Buffer.from("\n")]), { flag: "w" });
        const result = await runVerifier(destination);
        assertFailureCode(result, "foundation_hash_mismatch");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("unexpected executable runtime files are quarantined", async () => {
      const target = path.join(destination, "runtime", "unexpected.ps1");
      try {
        await writeFile(target, "Write-Output 'not allowed'\n", { flag: "wx" });
        const result = await runVerifier(destination);
        assertFailureCode(result, "foundation_executable_content");
      } finally {
        await rm(target, { force: true });
      }
    });

    await t.test("executable content in an allowed instruction is quarantined", async () => {
      const target = path.join(destination, runtimePaths[0]);
      const original = await readFile(target);
      try {
        await writeFile(target, "```powershell\nWrite-Output 'not allowed'\n```\n", { flag: "w" });
        const result = await runVerifier(destination);
        assertFailureCode(result, "foundation_executable_content");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("external-context dependencies are rejected", async () => {
      const target = path.join(destination, "adoption-ledger.json");
      const original = await readFile(target);
      try {
        const adoption = JSON.parse(original.toString("utf8"));
        adoption.externalContext = contextMarkers[0];
        await writeFile(target, `${JSON.stringify(adoption, null, 2)}\n`, { flag: "w" });
        const result = await runVerifier(destination);
        assertFailureCode(result, "foundation_external_context_dependency");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("a missing license selects the license recovery path", async () => {
      const target = path.join(destination, "licenses", "BMAD-METHOD-MIT.txt");
      const original = await readFile(target);
      try {
        await rm(target);
        const result = await runVerifier(destination);
        assertFailureCode(result, "foundation_license_decision_missing");
      } finally {
        await writeFile(target, original, { flag: "wx" });
      }
    });

    await t.test("the package distribution allowlist cannot drift", async () => {
      const target = path.join(destination, "package.json");
      const original = await readFile(target);
      try {
        const manifest = JSON.parse(original.toString("utf8"));
        manifest.files = manifest.files.filter((entry) => entry !== "NOTICE.md");
        await writeFile(target, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "w" });
        const result = await runVerifier(destination);
        assertFailureCode(result, "foundation_hash_mismatch");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("manifest dependencies select the external-context recovery path", async () => {
      const target = path.join(destination, "package.json");
      const original = await readFile(target);
      try {
        const manifest = JSON.parse(original.toString("utf8"));
        manifest.dependencies = { unexpected: "1.0.0" };
        await writeFile(target, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "w" });
        assertFailureCode(await runVerifier(destination), "foundation_external_context_dependency");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("manifest traversal selects the reference-escape recovery path", async () => {
      const target = path.join(destination, "package.json");
      const original = await readFile(target);
      try {
        const manifest = JSON.parse(original.toString("utf8"));
        manifest.files[0] = "../outside";
        await writeFile(target, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "w" });
        assertFailureCode(await runVerifier(destination), "foundation_reference_escape");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("projection traversal selects the reference-escape recovery path", async () => {
      const target = path.join(destination, "adoption-ledger.json");
      const original = await readFile(target);
      try {
        const adoption = JSON.parse(original.toString("utf8"));
        adoption.runtimeProjections[0].path = "../outside.md";
        await writeFile(target, `${JSON.stringify(adoption, null, 2)}\n`, { flag: "w" });
        assertFailureCode(await runVerifier(destination), "foundation_reference_escape");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("missing adoption license evidence selects the license recovery path", async () => {
      const target = path.join(destination, "adoption-ledger.json");
      const original = await readFile(target);
      try {
        const adoption = JSON.parse(original.toString("utf8"));
        adoption.licenseDecisions.pop();
        await writeFile(target, `${JSON.stringify(adoption, null, 2)}\n`, { flag: "w" });
        assertFailureCode(await runVerifier(destination), "foundation_license_decision_missing");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("missing semantic license evidence selects the license recovery path", async () => {
      const target = path.join(destination, "semantic-source-ledger.json");
      const original = await readFile(target);
      try {
        const semantic = JSON.parse(original.toString("utf8"));
        semantic.licenses.pop();
        await writeFile(target, `${JSON.stringify(semantic, null, 2)}\n`, { flag: "w" });
        assertFailureCode(await runVerifier(destination), "foundation_license_decision_missing");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("malformed ledger records retain stable recovery codes", async (shapeTest) => {
      const cases = [
        {
          name: "source identity record",
          file: "semantic-source-ledger.json",
          mutate: (value) => { value.sources[0] = null; },
          code: "foundation_source_identity_incomplete",
        },
        {
          name: "identity evidence record",
          file: "semantic-source-ledger.json",
          mutate: (value) => { value.identityEvidence[0] = null; },
          code: "foundation_source_identity_incomplete",
        },
        {
          name: "source member record",
          file: "semantic-source-ledger.json",
          mutate: (value) => { value.sourceMembers[0] = null; },
          code: "foundation_source_identity_incomplete",
        },
        {
          name: "legal evidence record",
          file: "semantic-source-ledger.json",
          mutate: (value) => { value.legalEvidence[0] = null; },
          code: "foundation_license_decision_missing",
        },
        {
          name: "semantic license record",
          file: "semantic-source-ledger.json",
          mutate: (value) => { value.licenses[0] = null; },
          code: "foundation_license_decision_missing",
        },
        {
          name: "managed output record",
          file: "semantic-source-ledger.json",
          mutate: (value) => { value.managedOutputs[0] = null; },
          code: "foundation_hash_mismatch",
        },
        {
          name: "license decision record",
          file: "adoption-ledger.json",
          mutate: (value) => { value.licenseDecisions[0] = null; },
          code: "foundation_license_decision_missing",
        },
        {
          name: "runtime projection record",
          file: "adoption-ledger.json",
          mutate: (value) => { value.runtimeProjections[0] = null; },
          code: "foundation_hash_mismatch",
        },
      ];
      for (const testCase of cases) {
        await shapeTest.test(testCase.name, async () => {
          const target = path.join(destination, testCase.file);
          const original = await readFile(target);
          try {
            const value = JSON.parse(original.toString("utf8"));
            testCase.mutate(value);
            await writeFile(target, `${JSON.stringify(value, null, 2)}\n`, { flag: "w" });
            assertFailureCode(await runVerifier(destination), testCase.code);
          } finally {
            await writeFile(target, original, { flag: "w" });
          }
        });
      }
    });

    await t.test("duplicate decoded foundation JSON keys fail closed", async (duplicateTest) => {
      const target = path.join(destination, "package.json");
      const original = await readFile(target);
      for (const duplicateKey of ["private", "priva\\u0074e"]) {
        await duplicateTest.test(duplicateKey, async () => {
          try {
            const source = original.toString("utf8");
            const needle = '  "private": true,';
            const replacement = `${needle}\n  "${duplicateKey}": true,`;
            assert.notEqual(source.replace(needle, replacement), source);
            await writeFile(target, source.replace(needle, replacement), { flag: "w" });
            assertFailureCode(await runVerifier(destination), "foundation_hash_mismatch");
          } finally {
            await writeFile(target, original, { flag: "w" });
          }
        });
      }
    });

    await t.test("guessed Git identity keeps promotion on the source-identity recovery path", async () => {
      const target = path.join(destination, "semantic-source-ledger.json");
      const original = await readFile(target);
      try {
        const semantic = JSON.parse(original.toString("utf8"));
        semantic.sources[0].gitIdentity = "guessed-tag";
        semantic.sources[0].promotionEligibility = "eligible";
        await writeFile(target, `${JSON.stringify(semantic, null, 2)}\n`, { flag: "w" });
        assertFailureCode(await runVerifier(destination), "foundation_source_identity_incomplete");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("authority-bearing projection fields are quarantined", async () => {
      const target = path.join(destination, "adoption-ledger.json");
      const original = await readFile(target);
      try {
        const adoption = JSON.parse(original.toString("utf8"));
        adoption.runtimeProjections[0].command = "node payload.mjs";
        await writeFile(target, `${JSON.stringify(adoption, null, 2)}\n`, { flag: "w" });
        assertFailureCode(await runVerifier(destination), "foundation_executable_content");
      } finally {
        await writeFile(target, original, { flag: "w" });
      }
    });

    await t.test("linked package directories select the reference-escape recovery path", async (subtest) => {
      const outside = path.join(temporaryRoot, "outside");
      const target = path.join(destination, "runtime", "linked");
      await mkdir(outside);
      try {
        try {
          await symlink(outside, target, process.platform === "win32" ? "junction" : "dir");
        } catch (error) {
          if (["EACCES", "ENOTSUP", "EPERM"].includes(error?.code)) {
            subtest.skip(`link creation unavailable: ${error.code}`);
            return;
          }
          throw error;
        }
        assertFailureCode(await runVerifier(destination), "foundation_reference_escape");
      } finally {
        await rm(target, { force: true });
      }
    });
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

// Readiness Task 4: the complete BMAD capability denominator (ADR-0005).
// 26 roster menu paths plus the five Builder authoring operations, each
// bound to exactly one closure-ledger record. The denominator may grow,
// never shrink.
const expectedMenuPaths = Object.freeze([
  ["bmad-agent-analyst", "BP", "bmm:bmad-brainstorming"],
  ["bmad-agent-analyst", "MR", "bmm:bmad-market-research"],
  ["bmad-agent-analyst", "DR", "bmm:bmad-domain-research"],
  ["bmad-agent-analyst", "TR", "bmm:bmad-technical-research"],
  ["bmad-agent-analyst", "CB", "bmm:bmad-product-brief"],
  ["bmad-agent-analyst", "WB", "bmm:bmad-prfaq"],
  ["bmad-agent-analyst", "DP", "bmm:bmad-document-project"],
  ["bmad-agent-tech-writer", "DP", "bmm:bmad-document-project"],
  ["bmad-agent-tech-writer", "WD", "bmm:tech-writer-write-document"],
  ["bmad-agent-tech-writer", "MG", "bmm:tech-writer-mermaid-gen"],
  ["bmad-agent-tech-writer", "VD", "bmm:tech-writer-validate-doc"],
  ["bmad-agent-tech-writer", "EC", "bmm:tech-writer-explain-concept"],
  ["bmad-agent-pm", "PRD", "bmm:bmad-prd"],
  ["bmad-agent-pm", "CE", "bmm:bmad-create-epics-and-stories"],
  ["bmad-agent-pm", "IR", "bmm:bmad-check-implementation-readiness"],
  ["bmad-agent-pm", "CC", "bmm:bmad-correct-course"],
  ["bmad-agent-ux-designer", "CU", "bmm:bmad-ux"],
  ["bmad-agent-architect", "CA", "bmm:bmad-architecture"],
  ["bmad-agent-architect", "IR", "bmm:bmad-check-implementation-readiness"],
  ["bmad-agent-dev", "DS", "bmm:bmad-dev-story"],
  ["bmad-agent-dev", "QD", "bmm:bmad-quick-dev"],
  ["bmad-agent-dev", "QA", "bmm:bmad-qa-generate-e2e-tests"],
  ["bmad-agent-dev", "CR", "bmm:bmad-code-review"],
  ["bmad-agent-dev", "SP", "bmm:bmad-sprint-planning"],
  ["bmad-agent-dev", "CS", "bmm:bmad-create-story"],
  ["bmad-agent-dev", "ER", "bmm:bmad-retrospective"],
]);

const expectedBuilderOperations = Object.freeze([
  "builder:agent.analyze",
  "builder:agent.create_rebuild",
  "builder:agent.edit",
  "builder:workflow.analyze",
  "builder:workflow.build_edit",
]);

const capabilityArchetypes = Object.freeze({
  document_artifact: "sapphirus.bmad-document-artifact.v1",
  governed_change_set: "sapphirus.bmad-governed-change-set.v1",
  inactive_builder_draft: "sapphirus.bmad-inactive-builder-draft.v1",
});

test("capability closure ledger covers every roster menu path exactly once", async () => {
  const ledger = JSON.parse(
    await readFile(path.join(packageRoot, "capability-closure-ledger.json"), "utf8"),
  );
  assert.equal(ledger.schemaVersion, "sapphirus.bmad-capability-closure.v1");
  const records = ledger.capabilities;
  assert.ok(Array.isArray(records));

  const byId = new Map();
  for (const record of records) {
    assert.equal(byId.has(record.capabilityId), false, `duplicate ${record.capabilityId}`);
    byId.set(record.capabilityId, record);
    assert.ok(
      Object.hasOwn(capabilityArchetypes, record.outputArchetype),
      `${record.capabilityId}: unknown archetype ${record.outputArchetype}`,
    );
    assert.equal(
      record.outputSchema,
      capabilityArchetypes[record.outputArchetype],
      `${record.capabilityId}: schema must match its archetype`,
    );
    assert.ok(
      ["planned", "active"].includes(record.activationStatus),
      `${record.capabilityId}: invalid activation status`,
    );
    assert.equal(record.agentCodes.length, record.menuCodes.length);
  }

  const seenPaths = new Set();
  for (const record of records) {
    record.agentCodes.forEach((agentCode, index) => {
      const key = `${agentCode}/${record.menuCodes[index]}`;
      assert.equal(seenPaths.has(key), false, `duplicate menu path ${agentCode}/${record.menuCodes[index]}`);
      seenPaths.add(key);
    });
  }

  for (const [agentCode, menuCode, capabilityId] of expectedMenuPaths) {
    const record = byId.get(capabilityId);
    assert.ok(record, `missing capability ${capabilityId}`);
    const index = record.agentCodes.findIndex(
      (code, position) => code === agentCode && record.menuCodes[position] === menuCode,
    );
    assert.notEqual(index, -1, `capability ${capabilityId} missing path ${agentCode}/${menuCode}`);
  }
  assert.equal(seenPaths.size, expectedMenuPaths.length);

  for (const capabilityId of expectedBuilderOperations) {
    const record = byId.get(capabilityId);
    assert.ok(record, `missing builder operation ${capabilityId}`);
    assert.equal(record.outputArchetype, "inactive_builder_draft");
  }

  // Tasks 7-8 activation: every reviewed capability (26 menu paths and
  // the five Builder authoring operations) was proven through the
  // generic lifecycle matrix. Builder outputs stay inactive drafts
  // (ADR-0006): activation here means the authoring flow runs, never
  // that a draft can execute.
  for (const record of records) {
    assert.equal(
      record.activationStatus,
      "active",
      `${record.capabilityId} activation status`,
    );
    assert.ok(
      record.managedProjection === null
        || record.managedProjection.startsWith("runtime/"),
      `${record.capabilityId} projection binding`,
    );
    if (record.activationStatus === "active") {
      assert.notEqual(record.managedProjection, null,
        `${record.capabilityId} must bind a sealed projection to activate`);
    }
  }

  const menuCapabilityIds = new Set(expectedMenuPaths.map(([, , id]) => id));
  assert.equal(
    records.length,
    menuCapabilityIds.size + expectedBuilderOperations.length,
    "denominator drift: unexpected extra or missing capability records",
  );
});
