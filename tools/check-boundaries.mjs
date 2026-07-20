import { lstat, readFile, readdir } from "node:fs/promises";
import { extname, join, relative } from "node:path";
import process from "node:process";

const root = process.cwd();
const violations = [];

const adapterCrates = new Set([
  "desktop-workspace",
  "desktop-airlock",
  "desktop-execution",
  "desktop-store",
  "desktop-cloud",
  "desktop-update",
]);
const requiredCrates = new Set([
  "desktop-app",
  "desktop-ipc",
  "desktop-runtime",
  ...adapterCrates,
]);
// The reviewed ready-command catalog: the D1 read-only set, the D2
// Help/model set, and the D3 governed-edits set. Must stay byte-identical across READY_COMMANDS in
// crates/desktop-app/src/commands.rs, is_known_command in
// crates/desktop-ipc/src/envelope.rs, and desktopHostCommands in
// apps/desktop-ui/src/lib/hostClient/contracts.ts.
const reviewedReadyCommands = [
  "app.get_boot_state",
  "workspace.select_folder",
  "workspace.list",
  "workspace.revoke",
  "workspace.list_entries",
  "workspace.read_text",
  "workspace.search",
  "bmad.scan",
  "bmad.library.snapshot",
  "bmad.persona.view",
  "model.auth.status",
  "model.auth.sign_in",
  "model.auth.sign_out",
  "bmad.help.prepare",
  "bmad.help.approve",
  "bmad.help.cancel",
  "bmad.help.submit",
  "bmad.help.latest",
  "run.create",
  "context.preview",
  "workspace.enable_edits",
  "changes.propose",
  "approval.decide",
  "rollback.request",
  "changes.history",
  "changes.recovery.prepare",
  "changes.recovery.decide",
  "app.preferences.get",
  "app.preferences.set",
  "app.about",
  "app.offboarding.inspect",
  "app.offboarding.erase",
  "workspace.pick_files",
];
const recoveryCommands = ["app.get_boot_state", "workspace.list"];
const updateBlockingRecoveryStates = ["recovery_required", "restoring", "manual_review"];
const boundedProcessAdapterPaths = new Set([
  "crates/desktop-cloud/src/windows_broker.rs",
]);
const exactToolchain = Object.freeze({
  node: "24.18.0",
  pnpm: "11.12.0",
  rust: "1.97.0",
  typescript: "7.0.2",
});
const referenceVaultPattern = /(?:bmad-runtime-lib|_source_review)/iu;
const referenceVaultVerificationPattern =
  /\b(?:pnpm(?:\.cmd)?\s+(?:run\s+)?vault:check|node(?:\.exe)?\s+(?:\.?[\\/])?tools[\\/]verify-reference-vault\.mjs)\b/iu;
const referenceVaultAllowlist = new Set([
  ".gitignore",
  "README.md",
  "docs/provenance/vault-validation.json",
  "tools/check-boundaries.mjs",
  "tools/check-secrets.mjs",
  "tools/verify-reference-vault.mjs",
]);
const expectedProductionCsp = Object.freeze(new Map([
  ["default-src", Object.freeze(["'self'"])],
  ["base-uri", Object.freeze(["'none'"])],
  ["object-src", Object.freeze(["'none'"])],
  ["frame-src", Object.freeze(["'none'"])],
  ["frame-ancestors", Object.freeze(["'none'"])],
  ["form-action", Object.freeze(["'none'"])],
  ["script-src", Object.freeze(["'self'"])],
  ["style-src", Object.freeze(["'self'"])],
  ["font-src", Object.freeze(["'self'", "data:"])],
  ["img-src", Object.freeze(["'self'", "data:"])],
  ["connect-src", Object.freeze(["ipc:", "http://ipc.localhost"])],
]));

async function requiredText(path) {
  try {
    const metadata = await lstat(path);
    if (!metadata.isFile() || metadata.isSymbolicLink()) {
      violations.push(`${relative(root, path)}: required file must be a regular file, not a link`);
      return undefined;
    }
    return await readFile(path, "utf8");
  } catch (error) {
    violations.push(`${relative(root, path)}: required file is missing or unreadable`);
    return undefined;
  }
}

async function requiredJson(path) {
  const source = await requiredText(path);
  if (source === undefined) return undefined;
  try {
    return JSON.parse(source);
  } catch {
    violations.push(`${relative(root, path)}: invalid JSON`);
    return undefined;
  }
}

function parseLockedPackages(source) {
  const packages = [];
  for (const block of source.split(/^\[\[package\]\]\s*$/m).slice(1)) {
    const name = /^name\s*=\s*"([^"]+)"\s*$/m.exec(block)?.[1];
    if (!name) continue;
    const dependencyBlock = /^dependencies\s*=\s*\[([\s\S]*?)^\]\s*$/m.exec(block)?.[1] ?? "";
    const dependencies = [...dependencyBlock.matchAll(/^\s*"([^"]+)"\s*,?\s*$/gm)].map(
      (match) => match[1].split(" ")[0],
    );
    packages.push({ name, dependencies });
  }
  return packages;
}

function quotedStrings(source) {
  return [
    ...source.matchAll(
      /"((?:\\[\s\S]|[^"\\])*)"|'((?:\\[\s\S]|[^'\\])*)'|`((?:\\[\s\S]|[^`\\])*)`/gu,
    ),
  ].map((match) => match[1] ?? match[2] ?? match[3]);
}

function sameOrderedValues(actual, expected) {
  return Array.isArray(actual)
    && actual.length === expected.length
    && actual.every((value, index) => value === expected[index]);
}

function expandBmadModelCommandAliases(source) {
  return source
    .replaceAll("bmadModelCommands.authStatus", '"model.auth.status"')
    .replaceAll("bmadModelCommands.authSignIn", '"model.auth.sign_in"')
    .replaceAll("bmadModelCommands.authSignOut", '"model.auth.sign_out"')
    .replaceAll("bmadModelCommands.prepare", '"bmad.help.prepare"')
    .replaceAll("bmadModelCommands.approve", '"bmad.help.approve"')
    .replaceAll("bmadModelCommands.cancel", '"bmad.help.cancel"')
    .replaceAll("bmadModelCommands.submit", '"bmad.help.submit"');
}

const commandLiteralRegression = ["lower.command", "UPPER_COMMAND", "punctuation:/command"];
if (
  !sameOrderedValues(
    quotedStrings(`"lower.command", 'UPPER_COMMAND', \`punctuation:/command\``),
    commandLiteralRegression,
  )
) {
  violations.push("tools/check-boundaries.mjs: exact command-literal parser regression");
}

function isExactExternalVersion(value) {
  return /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.test(value);
}

function isExactCargoVersion(value) {
  return /^=(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.test(value);
}

function tomlSection(source, name) {
  const lines = source.split(/\r\n|\r|\n/u);
  const header = `[${name}]`;
  const start = lines.findIndex((line) => line.trim() === header);
  if (start === -1) return undefined;
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    const candidate = lines[index].trim();
    if (candidate.startsWith("[") && candidate.endsWith("]")) {
      end = index;
      break;
    }
  }
  return lines.slice(start + 1, end).join("\n");
}

function tomlAssignments(section) {
  return [...section.matchAll(/^(?:"([^"]+)"|([A-Za-z0-9_-]+))\s*=\s*(.+)$/gmu)]
    .map((match) => ({ name: match[1] ?? match[2], value: match[3].trim() }));
}

function containsReferenceVault(value) {
  if (typeof value === "string") return referenceVaultPattern.test(value);
  if (Array.isArray(value)) return value.some(containsReferenceVault);
  if (value !== null && typeof value === "object") {
    return Object.values(value).some(containsReferenceVault);
  }
  return false;
}

function normalizedRepositoryPath(path) {
  return relative(root, path).replaceAll("\\", "/");
}

function hasForbiddenReferenceVaultDependency(path, source) {
  const repositoryPath = normalizedRepositoryPath(path);
  return referenceVaultPattern.test(source)
    && !referenceVaultAllowlist.has(repositoryPath);
}

function invokesReferenceVaultVerification(source) {
  return typeof source === "string" && referenceVaultVerificationPattern.test(source);
}

const referenceVaultGuardRegressionPath = join(root, "packages", "example", "package.json");
if (
  !hasForbiddenReferenceVaultDependency(
    referenceVaultGuardRegressionPath,
    '{"source":"../../bmad-runtime-lib/example"}',
  )
) {
  violations.push("tools/check-boundaries.mjs: reference-vault dependency guard regression");
}
if (hasForbiddenReferenceVaultDependency(join(root, "README.md"), "bmad-runtime-lib")) {
  violations.push("tools/check-boundaries.mjs: reference-vault provenance allowlist regression");
}
for (const invocation of [
  "pnpm vault:check",
  "pnpm run vault:check",
  "node tools/verify-reference-vault.mjs",
  "node .\\tools\\verify-reference-vault.mjs",
]) {
  if (!invokesReferenceVaultVerification(invocation)) {
    violations.push("tools/check-boundaries.mjs: reference-vault command guard regression");
  }
}
if (invokesReferenceVaultVerification("pnpm bmad:foundation:verify")) {
  violations.push("tools/check-boundaries.mjs: reference-vault command false positive");
}

function validateProductionCsp(source, displayPath) {
  const actual = new Map();
  for (const rawDirective of source.split(";")) {
    const directive = rawDirective.trim();
    if (directive === "") continue;
    const [name, ...values] = directive.split(/\s+/u);
    if (actual.has(name)) {
      violations.push(`${displayPath}: production CSP repeats ${name}`);
      continue;
    }
    actual.set(name, values);
  }
  for (const [name, expectedValues] of expectedProductionCsp) {
    const actualValues = actual.get(name);
    if (actualValues === undefined) {
      violations.push(`${displayPath}: production CSP is missing ${name}`);
    } else if (!sameOrderedValues(actualValues, expectedValues)) {
      violations.push(
        `${displayPath}: production CSP ${name} must be exactly ${expectedValues.join(" ")}`,
      );
    }
  }
  for (const name of actual.keys()) {
    if (!expectedProductionCsp.has(name)) {
      violations.push(`${displayPath}: production CSP contains unexpected directive ${name}`);
    }
  }
}

async function packageManifestPaths() {
  const paths = [join(root, "package.json")];
  for (const container of ["apps", "packages"]) {
    let entries;
    const containerPath = join(root, container);
    try {
      const metadata = await lstat(containerPath);
      if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
        violations.push(`${container}: first-party package container must be a regular directory`);
        continue;
      }
      entries = await readdir(containerPath, { withFileTypes: true });
    } catch {
      violations.push(`${container}: first-party package container is missing or unreadable`);
      continue;
    }
    for (const entry of entries) {
      if (entry.isSymbolicLink()) {
        violations.push(`${container}/${entry.name}: linked first-party package entries are forbidden`);
        continue;
      }
      if (entry.isDirectory()) paths.push(join(root, container, entry.name, "package.json"));
    }
  }
  return paths;
}

const firstPartyManifests = [];
for (const manifestPath of await packageManifestPaths()) {
  const manifest = await requiredJson(manifestPath);
  if (manifest === undefined) continue;
  const displayPath = relative(root, manifestPath);
  firstPartyManifests.push({ displayPath, manifest });
}
const firstPartyPackageNames = new Set();
for (const { displayPath, manifest } of firstPartyManifests) {
  if (typeof manifest.name !== "string" || manifest.name.length === 0) {
    violations.push(`${displayPath}: internal first-party package must have a name`);
  } else if (firstPartyPackageNames.has(manifest.name)) {
    violations.push(`${displayPath}: duplicate first-party package name ${manifest.name}`);
  } else {
    firstPartyPackageNames.add(manifest.name);
  }
}
for (const requiredPackageName of ["@sapphirus/bmad-foundation"]) {
  if (!firstPartyPackageNames.has(requiredPackageName)) {
    violations.push(`required first-party package is missing: ${requiredPackageName}`);
  }
}
for (const { displayPath, manifest } of firstPartyManifests) {
  if (manifest.private !== true) {
    violations.push(`${displayPath}: internal first-party package must set private to true`);
  }
  for (const field of [
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
  ]) {
    const dependencies = manifest[field];
    if (dependencies === undefined) continue;
    if (typeof dependencies !== "object" || dependencies === null || Array.isArray(dependencies)) {
      violations.push(`${displayPath}: ${field} must be an object`);
      continue;
    }
    for (const [name, version] of Object.entries(dependencies)) {
      if (typeof version !== "string") {
        violations.push(`${displayPath}: ${field}.${name} is not an exact or workspace-local version`);
      } else if (version === "workspace:*") {
        if (!firstPartyPackageNames.has(name)) {
          violations.push(`${displayPath}: ${field}.${name} references an unknown workspace package`);
        }
      } else if (!isExactExternalVersion(version)) {
        violations.push(`${displayPath}: ${field}.${name} is not an exact or workspace-local version`);
      }
      if (name === "typescript" && version !== exactToolchain.typescript) {
        violations.push(`${displayPath}: TypeScript must be exactly ${exactToolchain.typescript}`);
      }
    }
  }
}

const rootPackage = await requiredJson(join(root, "package.json"));
if (rootPackage) {
  if (rootPackage.packageManager !== `pnpm@${exactToolchain.pnpm}`) {
    violations.push(`package.json: packageManager must be pnpm@${exactToolchain.pnpm}`);
  }
  if (
    rootPackage.engines?.node !== exactToolchain.node
    || rootPackage.engines?.pnpm !== exactToolchain.pnpm
  ) {
    violations.push("package.json: Node and pnpm engines drifted from the reviewed toolchain");
  }
  if (rootPackage.scripts?.verify !== "pnpm verify:source") {
    violations.push("package.json: default verify must remain an alias for verify:source");
  }
  if (
    rootPackage.scripts?.["bmad:foundation:verify"]
    !== "pnpm --filter @sapphirus/bmad-foundation run verify"
  ) {
    violations.push("package.json: BMAD foundation verification gate is missing or drifted");
  }
  if (
    rootPackage.scripts?.["desktop:build"]
    !== "pnpm exec tauri build --config crates/desktop-app/tauri.conf.json --features deterministic-help"
  ) {
    violations.push(
      "package.json: the offline desktop installer must include deterministic local Help",
    );
  }
  const sourceVerification = rootPackage.scripts?.["verify:source"];
  if (typeof sourceVerification !== "string") {
    violations.push("package.json: verify:source is missing");
  } else if (invokesReferenceVaultVerification(sourceVerification)) {
    violations.push("package.json: verify:source must not depend on the optional reference vault");
  } else if (
    /\b(?:cargo|rustc|dotnet|msbuild|cl(?:\.exe)?|desktop:|rust:|verify:deferred-full|cross-language)\b/iu
      .test(sourceVerification)
  ) {
    violations.push("package.json: verify:source references a frozen native or cross-language lane");
  } else if (
    !sourceVerification
      .split("&&")
      .map((step) => step.trim())
      .includes("pnpm bmad:foundation:verify")
  ) {
    violations.push("package.json: verify:source does not run the BMAD foundation gate");
  }
  if (invokesReferenceVaultVerification(rootPackage.scripts?.["verify:deferred-full"] ?? "")) {
    violations.push("package.json: verify:deferred-full must not depend on the optional reference vault");
  }
}

for (const workflowPath of [
  ".github/workflows/contracts.yml",
  ".github/workflows/source.yml",
]) {
  const workflow = await requiredText(join(root, ...workflowPath.split("/")));
  if (workflow !== undefined && invokesReferenceVaultVerification(workflow)) {
    violations.push(`${workflowPath}: product CI must not depend on the optional reference vault`);
  }
}

const frozenToolPattern = /(?:^|[\s&|])(?:cargo|rustc|dotnet|msbuild|cl(?:\.exe)?)(?=[\s&|]|$)/iu;
for (const { displayPath, manifest } of firstPartyManifests) {
  for (const scriptName of ["build", "lint", "test", "typecheck"]) {
    const script = manifest.scripts?.[scriptName];
    if (typeof script === "string" && frozenToolPattern.test(script)) {
      violations.push(`${displayPath}: source-lane script ${scriptName} invokes a frozen native tool`);
    }
  }
}

for (const versionFile of [".node-version", ".nvmrc"]) {
  const source = await requiredText(join(root, versionFile));
  if (source !== undefined && source.trim() !== exactToolchain.node) {
    violations.push(`${versionFile}: Node must be exactly ${exactToolchain.node}`);
  }
}

const pnpmLockSource = await requiredText(join(root, "pnpm-lock.yaml"));
if (pnpmLockSource !== undefined) {
  const lockedTypeScriptVersions = [
    ...pnpmLockSource.matchAll(/^[ \t]{2}typescript@([^:\r\n]+):[ \t]*$/gmu),
  ].map((match) => match[1]);
  if (lockedTypeScriptVersions.length === 0) {
    violations.push("pnpm-lock.yaml: the reviewed TypeScript compiler is missing");
  }
  for (const version of new Set(lockedTypeScriptVersions)) {
    if (version !== exactToolchain.typescript) {
      violations.push(
        `pnpm-lock.yaml: TypeScript lock entry ${version} is forbidden; expected only ${exactToolchain.typescript}`,
      );
    }
  }
  const lockedTypeScriptNativeVersions = [
    ...pnpmLockSource.matchAll(
      /^[ \t]{2}'@typescript\/typescript-[^']+@([^']+)':[ \t]*$/gmu,
    ),
  ].map((match) => match[1]);
  if (lockedTypeScriptNativeVersions.length === 0) {
    violations.push("pnpm-lock.yaml: TypeScript 7 native compiler packages are missing");
  }
  for (const version of new Set(lockedTypeScriptNativeVersions)) {
    if (version !== exactToolchain.typescript) {
      violations.push(
        `pnpm-lock.yaml: TypeScript native lock entry ${version} is forbidden; expected only ${exactToolchain.typescript}`,
      );
    }
  }
}

for (const workflowName of [
  "desktop.yml",
  "security-nightly.yml",
  "release-dry-run.yml",
]) {
  const workflowPath = join(root, ".github", "workflows", workflowName);
  const workflowSource = await requiredText(workflowPath);
  if (
    workflowSource !== undefined
    && !/^ {4}if: \$\{\{ vars\.SAPPHIRUS_NATIVE_LANE_ENABLED == 'true' \}\}\s*$/mu
      .test(workflowSource)
  ) {
    violations.push(
      `${relative(root, workflowPath)}: native workflow is missing the organization-controlled freeze gate`,
    );
  }
}

{
  const workflowsRoot = join(root, ".github", "workflows");
  const workflowFiles = (await readdir(workflowsRoot)).filter((name) => name.endsWith(".yml"));
  for (const workflowFile of workflowFiles) {
    const workflowSource = await readFile(join(workflowsRoot, workflowFile), "utf8");
    for (const match of workflowSource.matchAll(/^\s*(?:- )?uses:\s*([^\s#]+)/gmu)) {
      if (!/^[^@\s]+@[0-9a-f]{40}$/u.test(match[1])) {
        violations.push(
          `.github/workflows/${workflowFile}: action reference must use a reviewed full commit SHA (${match[1]})`,
        );
      }
    }
  }
}

const signedReleasePath = join(root, ".github", "workflows", "release-windows-signed.yml");
const signedReleaseSource = await requiredText(signedReleasePath);
if (signedReleaseSource !== undefined) {
  for (const match of signedReleaseSource.matchAll(/^\s*- uses: ([^\s#]+)/gmu)) {
    if (!/^[^@]+@[0-9a-f]{40}$/u.test(match[1])) {
      violations.push(
        `${relative(root, signedReleasePath)}: action reference must use a reviewed full commit SHA (${match[1]})`,
      );
    }
  }
  for (const [pattern, message] of [
    [
      /github\.ref == 'refs\/heads\/main'/u,
      "must only sign the protected main branch",
    ],
    [
      /vars\.SAPPHIRUS_NATIVE_LANE_ENABLED == 'true'/u,
      "must remain behind the organization native-lane gate",
    ],
    [
      /vars\.SAPPHIRUS_SIGNING_LANE_ENABLED == 'true'/u,
      "must remain behind the organization signing-lane gate",
    ],
    [
      /^ {4}environment: windows-signing\s*$/mu,
      "must use the protected organization signing environment",
    ],
    [
      /^ {4}runs-on: \[self-hosted, windows, x64, sapphirus-signing\]\s*$/mu,
      "must use an organization-managed signing runner",
    ],
    [
      /^ {4}runs-on: \[self-hosted, windows, x64, sapphirus-qualification\]\s*$/mu,
      "must execute installers on a separate qualification runner",
    ],
    [
      /uses: actions\/checkout@[0-9a-f]{40}/u,
      "must pin checkout to an immutable commit",
    ],
    [
      /uses: dtolnay\/rust-toolchain@[0-9a-f]{40}/u,
      "must pin the Rust setup action to an immutable commit",
    ],
    [
      /\.\/tools\/build-signed-windows-installer\.ps1\s*$/mu,
      "must execute the repository-owned signed build",
    ],
    [
      /-RequireValidSignature\s*$/mu,
      "must fail closed unless the installer and installed application signatures are valid",
    ],
    [
      /-PriorInstallerPath "\$env:SAPPHIRUS_PRIOR_INSTALLER"\s*$/mu,
      "must exercise an upgrade from an explicit prior installer",
    ],
    [
      /-ExpectedPriorVersion "\$env:SAPPHIRUS_PRIOR_VERSION"\s*$/mu,
      "must pass dispatcher input through the environment rather than shell interpolation",
    ],
    [
      /sapphirus-signed-build-evidence\.json\s*$/mu,
      "must retain signed-build evidence",
    ],
    [
      /sapphirus-signed-lifecycle-evidence\.json\s*$/mu,
      "must retain signed lifecycle evidence",
    ],
    [
      /buildEvidence\.installer\.sha256 -ne \$lifecycleEvidence\.artifact\.sha256/u,
      "must bind lifecycle evidence to the exact signed installer",
    ],
    [
      /buildEvidence\.application\.sha256 -ne \$lifecycleEvidence\.lifecycle\.installedExecutableSha256/u,
      "must bind the installed executable to the exact signed application",
    ],
    [
      /node tools\/resolve-release-metadata\.mjs --github-output/u,
      "must resolve workflow paths and versions from the repository release authority",
    ],
    [
      /node tools\/generate-release-sbom\.mjs/u,
      "must generate the deterministic release SBOM",
    ],
    [
      /\$buildEvidence\.sbom\.sha256 -ne \$sbomHash/u,
      "must bind the SBOM to signed-build evidence before qualification",
    ],
    [
      /^  attest-qualified-installer:\s*$/mu,
      "must attest only after signed lifecycle qualification",
    ],
    [
      /uses: actions\/attest-build-provenance@e8998f949152b193b063cb0ec769d69d929409be/u,
      "must create immutable build provenance",
    ],
    [
      /uses: actions\/attest@[0-9a-f]{40}/u,
      "must bind the SBOM to the signed application and installer",
    ],
    [
      /node tools\/create-release-attestation-predicate\.mjs/u,
      "must revalidate exact qualification evidence before attestation",
    ],
    [
      /predicate-type: https:\/\/sapphirus\.dev\/attestations\/release-qualification\/v1/u,
      "must attest the release-specific toolchain, lock, and lifecycle predicate",
    ],
    [
      /SAPPHIRUS_EXPECTED_REVISION: \$\{\{ github\.sha \}\}[\s\S]*--expected-revision "\$SAPPHIRUS_EXPECTED_REVISION"/u,
      "must bind qualification evidence to the dispatched source revision",
    ],
    [
      /SAPPHIRUS_PRIOR_VERSION: \$\{\{ inputs\.prior_version \}\}[\s\S]*--expected-prior-version "\$SAPPHIRUS_PRIOR_VERSION"/u,
      "must route caller-controlled prior version through the environment",
    ],
    [
      /signed-installer-qualification-\$\{\{ github\.sha \}\}/u,
      "must consume the immutable qualification artifact before attestation",
    ],
    [
      /^      attestations: write\s*$/mu,
      "must grant attestation write authority only to the attestation job",
    ],
    [
      /^      id\x2dtoken: write\s*$/mu,
      "must grant OIDC authority only to the attestation job",
    ],
  ]) {
    if (!pattern.test(signedReleaseSource)) {
      violations.push(`${relative(root, signedReleasePath)}: ${message}`);
    }
  }
  const attestationJob = signedReleaseSource.match(/^  attest-qualified-installer:\s*$[\s\S]*$/mu)?.[0] ?? "";
  for (const runBlock of attestationJob.matchAll(/^      - name:.*\n(?:^        .*\n)*?^        run: [|>]\s*\n((?:^          .*\n?)*)/gmu)) {
    if (/\$\{\{\s*inputs\./u.test(runBlock[1])) {
      violations.push(`${relative(root, signedReleasePath)}: privileged attestation shell must not interpolate workflow inputs directly`);
    }
  }
}

const releaseDryRunPath = join(root, ".github", "workflows", "release-dry-run.yml");
const releaseDryRunSource = await requiredText(releaseDryRunPath);
if (releaseDryRunSource !== undefined) {
  for (const match of releaseDryRunSource.matchAll(/^\s*- uses: ([^\s#]+)/gmu)) {
    if (!/@[0-9a-f]{40}$/u.test(match[1])) {
      violations.push(`${relative(root, releaseDryRunPath)}: action reference must use a full immutable commit SHA: ${match[1]}`);
    }
  }
  for (const [pattern, message] of [
    [
      /\.\/tools\/qualify-windows-installer\.ps1\s*$/mu,
      "must execute the repository-owned Windows installer lifecycle verifier",
    ],
    [
      /node tools\/resolve-release-metadata\.mjs --github-output/u,
      "must resolve the installer identity from the repository release authority",
    ],
    [
      /-InstallerPath "target\/release\/bundle\/nsis\/\$\{\{ steps\.release-metadata\.outputs\.installer_name \}\}"\s*$/mu,
      "must qualify the resolver-selected NSIS artifact",
    ],
    [
      /-ExpectedVersion "\$\{\{ steps\.release-metadata\.outputs\.product_version \}\}"\s*$/mu,
      "must bind installer qualification to the resolver-selected product version",
    ],
    [
      /\$\{\{ runner\.temp \}\}\/sapphirus-installer-qualification\.json\s*$/mu,
      "must upload the installer lifecycle evidence",
    ],
  ]) {
    if (!pattern.test(releaseDryRunSource)) {
      violations.push(`${relative(root, releaseDryRunPath)}: ${message}`);
    }
  }
}

const installerQualificationPath = join(root, "tools", "qualify-windows-installer.ps1");
const installerQualificationSource = await requiredText(installerQualificationPath);
if (installerQualificationSource !== undefined) {
  for (const [pattern, message] of [
    [/Get-AuthenticodeSignature/u, "must record Authenticode status"],
    [/Assert-ExactFoundationPayload/u, "must verify the exact bundled BMAD foundation"],
    [/Assert-CleanQualificationAccount/u, "must refuse to overwrite an existing Sapphirus installation"],
    [/RequireValidSignature/u, "must expose a fail-closed signed-release gate"],
    [/Prior and current installers use different publishers/u, "must reject a prior installer from another publisher"],
    [/Compare-CanonicalSemVer[\s\S]*prior installer version must precede/u, "must reject same-version reinstall and downgrade qualification"],
    [/-not \[string\]::IsNullOrWhiteSpace\(\$PriorInstallerPath\)[\s\S]*Compare-CanonicalSemVer/u, "must preserve fresh-install qualification without prior-version comparison"],
    [/Assert-CleanQualificationAccount\s*\n\s*\$lifecycleComplete/u, "must recheck uninstall registration after removal"],
    [/Wait-ForPathState/u, "must verify install and uninstall lifecycle state"],
  ]) {
    if (!pattern.test(installerQualificationSource)) {
      violations.push(`${relative(root, installerQualificationPath)}: ${message}`);
    }
  }
}

const signedBuildPath = join(root, "tools", "build-signed-windows-installer.ps1");
const signedBuildSource = await requiredText(signedBuildPath);
if (signedBuildSource !== undefined) {
  for (const [pattern, message] of [
    [/\[string\] \$EvidencePath/u, "must require a durable signed-build evidence path"],
    [/sourceRevision = \$sourceRevision\.ToLowerInvariant\(\)/u, "must bind evidence to the exact source revision"],
    [/sourceTreeState = 'clean'/u, "must record a clean source tree"],
    [/Signed release builds require a clean source worktree/u, "must reject dirty source builds"],
    [/Assert-TimestampedPublisherSignature/u, "must verify publisher signatures and timestamps"],
    [/\[string\] \$SbomPath/u, "must require the release SBOM"],
    [/releaseMetadata = \$releaseMetadata/u, "must embed the centralized release metadata"],
    [/sha256 = \$sbomHash/u, "must bind the SBOM hash into signed-build evidence"],
    [/Runtime release toolchain disagrees with the reviewed metadata authority/u, "must reject runtime toolchain drift"],
    [/The signed build mutated the source worktree/u, "must reject post-build source drift"],
    [/Release metadata or lock identity changed during the signed build/u, "must reject post-build metadata and lock drift"],
    [/& \$node\.Source \$sbomScript --verify \$sbom/u, "must verify exact post-build SBOM bytes"],
  ]) {
    if (!pattern.test(signedBuildSource)) {
      violations.push(`${relative(root, signedBuildPath)}: ${message}`);
    }
  }
}

for (const releaseTool of [
  "resolve-release-metadata.mjs",
  "generate-release-sbom.mjs",
  "release-metadata.test.mjs",
  "release-sbom.test.mjs",
  "create-release-attestation-predicate.mjs",
  "release-attestation-predicate.test.mjs",
]) {
  await requiredText(join(root, "tools", releaseTool));
}

if (rootPackage?.scripts?.["release:test"] !== "node --test tools/release-metadata.test.mjs tools/release-sbom.test.mjs tools/release-attestation-predicate.test.mjs") {
  violations.push("package.json: release:test must run the exact metadata, SBOM, and attestation regression suite");
}
if (!String(rootPackage?.scripts?.["verify:source"] ?? "").includes("pnpm release:test")) {
  violations.push("package.json: verify:source must include the release metadata and SBOM regression suite");
}

const rustToolchainSource = await requiredText(join(root, "rust-toolchain.toml"));
if (
  rustToolchainSource !== undefined
  && !new RegExp(`^channel\\s*=\\s*"${exactToolchain.rust.replaceAll(".", "\\.")}"\\s*$`, "m")
    .test(rustToolchainSource)
) {
  violations.push(`rust-toolchain.toml: Rust must be exactly ${exactToolchain.rust}`);
}

const lockSource = await requiredText(join(root, "Cargo.lock"));
const packages = lockSource === undefined ? [] : parseLockedPackages(lockSource);
const packageByName = new Map(packages.map((item) => [item.name, item]));
const workspaceManifestSource = await requiredText(join(root, "Cargo.toml"));
if (workspaceManifestSource !== undefined) {
  const workspacePackage = tomlSection(workspaceManifestSource, "workspace.package");
  if (workspacePackage === undefined || !/^publish\s*=\s*false\s*(?:#.*)?$/mu.test(workspacePackage)) {
    violations.push("Cargo.toml: [workspace.package] must set publish to false");
  }
  const workspaceDependencies = tomlSection(workspaceManifestSource, "workspace.dependencies");
  if (workspaceDependencies === undefined) {
    violations.push("Cargo.toml: [workspace.dependencies] is missing");
  } else {
    for (const dependency of tomlAssignments(workspaceDependencies)) {
      if (/\bpath\s*=/u.test(dependency.value)) continue;
      const direct = /^"([^"]+)"/u.exec(dependency.value)?.[1];
      const inline = /\bversion\s*=\s*"([^"]+)"/u.exec(dependency.value)?.[1];
      const version = direct ?? inline;
      if (version === undefined || !isExactCargoVersion(version)) {
        violations.push(
          `Cargo.toml: [workspace.dependencies].${dependency.name} must use an exact = version`,
        );
      }
    }
  }
}
for (const crate of requiredCrates) {
  const manifestPath = join(root, "crates", crate, "Cargo.toml");
  if (!packageByName.has(crate)) violations.push(`crates/${crate}/Cargo.toml: required crate is absent`);
  const manifestSource = await requiredText(manifestPath);
  if (manifestSource !== undefined) {
    const packageSection = tomlSection(manifestSource, "package");
    if (packageSection === undefined || !/^publish\.workspace\s*=\s*true\s*(?:#.*)?$/mu.test(packageSection)) {
      violations.push(`${relative(root, manifestPath)}: [package] must inherit publish=false`);
    }
  }
}

for (const crate of adapterCrates) {
  const manifest = packageByName.get(crate);
  if (!manifest) continue;
  for (const dependency of manifest.dependencies) {
    if (adapterCrates.has(dependency)) {
      violations.push(`crates/${crate}/Cargo.toml: adapter ${crate} depends on ${dependency}`);
    }
  }
}

const runtime = packageByName.get("desktop-runtime");
if (runtime) {
  for (const dependency of runtime.dependencies) {
    if (adapterCrates.has(dependency) || dependency === "desktop-app") {
      violations.push(`crates/desktop-runtime/Cargo.toml: runtime depends on ${dependency}`);
    }
  }
}

async function walk(directory, extensions) {
  let entries;
  try {
    const metadata = await lstat(directory);
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
      violations.push(`${relative(root, directory)}: required source directory must be a regular directory`);
      return [];
    }
    entries = await readdir(directory, { withFileTypes: true });
  } catch {
    violations.push(`${relative(root, directory)}: required source directory is missing or unreadable`);
    return [];
  }
  const files = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      violations.push(`${relative(root, path)}: linked source entries are forbidden`);
    } else if (entry.isDirectory()) files.push(...(await walk(path, extensions)));
    else if (extensions.some((extension) => entry.name.endsWith(extension))) files.push(path);
  }
  return files;
}

const referenceScanExtensions = new Set([
  ".c",
  ".bicep",
  ".bicepparam",
  ".bat",
  ".cc",
  ".cmd",
  ".cpp",
  ".cs",
  ".csproj",
  ".css",
  ".example",
  ".html",
  ".js",
  ".jsx",
  ".json",
  ".lock",
  ".md",
  ".mjs",
  ".mts",
  ".ps1",
  ".props",
  ".rs",
  ".sh",
  ".slnx",
  ".targets",
  ".toml",
  ".ts",
  ".tsx",
  ".txt",
  ".xml",
  ".yaml",
  ".yml",
]);
const referenceScanExactNames = new Set([
  ".editorconfig",
  ".gitattributes",
  ".gitignore",
  ".node-version",
  ".npmrc",
  ".nvmrc",
  ".dockerignore",
  "containerfile",
  "dockerfile",
  "makefile",
]);
const referenceScanIgnoredDirectories = new Set([
  ".git",
  "bin",
  "dist",
  "node_modules",
  "obj",
  "target",
]);

function isReferenceScanFile(name) {
  const normalizedName = name.toLowerCase();
  return referenceScanExtensions.has(extname(normalizedName))
    || referenceScanExactNames.has(normalizedName);
}

async function walkFirstPartyInputs(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      if (!referenceScanIgnoredDirectories.has(entry.name)) {
        violations.push(
          `${normalizedRepositoryPath(path)}: linked first-party input is forbidden`,
        );
      }
      continue;
    }
    if (entry.isDirectory()) {
      if (!referenceScanIgnoredDirectories.has(entry.name)) {
        files.push(...(await walkFirstPartyInputs(path)));
      }
      continue;
    }
    if (entry.isFile() && isReferenceScanFile(entry.name)) {
      files.push(path);
    }
  }
  return files;
}

async function walkOptionalFirstPartyInputRoot(directory) {
  let rootEntry;
  try {
    rootEntry = await lstat(directory);
  } catch (error) {
    if (error?.code === "ENOENT") return [];
    throw error;
  }

  if (rootEntry.isSymbolicLink()) {
    violations.push(
      `${normalizedRepositoryPath(directory)}: linked first-party input root is forbidden`,
    );
    return [];
  }
  if (!rootEntry.isDirectory()) {
    violations.push(
      `${normalizedRepositoryPath(directory)}: first-party input root must be a directory`,
    );
    return [];
  }
  return walkFirstPartyInputs(directory);
}

async function rootFirstPartyInputs() {
  const entries = await readdir(root, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isSymbolicLink()) {
      if (!referenceScanIgnoredDirectories.has(entry.name)) {
        violations.push(
          `${normalizedRepositoryPath(path)}: linked first-party input is forbidden`,
        );
      }
      continue;
    }
    if (entry.isFile() && isReferenceScanFile(entry.name)) {
      files.push(path);
    }
  }
  return files;
}

const referenceScanRoots = [
  ".github",
  "apps",
  "crates",
  "helpers",
  "infra",
  "packages",
  "services",
  "tests",
  "tools",
];
const referenceScanAdditionalFiles = [
  "docs/provenance/vault-validation.json",
];
const referenceScanFiles = [
  ...(await rootFirstPartyInputs()),
  ...referenceScanAdditionalFiles.map((path) => join(root, path)),
  ...(await Promise.all(
    referenceScanRoots.map((path) => walkOptionalFirstPartyInputRoot(join(root, path))),
  )).flat(),
];
for (const path of referenceScanFiles) {
  const source = await requiredText(path);
  if (source !== undefined && hasForbiddenReferenceVaultDependency(path, source)) {
    violations.push(
      `${normalizedRepositoryPath(path)}: first-party input references the external context library`,
    );
  }
}

const rustFiles = await walk(join(root, "crates"), [".rs"]);
for (const path of rustFiles) {
  const source = await requiredText(path);
  if (source === undefined) continue;
  const displayPath = relative(root, path);
  const crate = displayPath.split(/[\\/]/)[1];
  if (crate === "desktop-app" && referenceVaultPattern.test(source)) {
    violations.push(`${displayPath}: composition root references the reference vault`);
  }
  if (crate !== "desktop-app" && /\b(?:tauri|tauri_plugin_[a-z_]+)::/.test(source)) {
    violations.push(`${displayPath}: Tauri import outside the composition root`);
  }
  const processPatterns = [
    // `abort` is a test-only no-unwind assertion helper, not a child-process
    // capability. All other direct std::process access remains forbidden.
    /\bstd::process(?!(?:::abort\b))/,
    /\btokio::process\b/,
    /\bCommand\s*::\s*new\b/,
    /\bCreateProcess(?:A|W)?\b/,
    /\bShellExecute(?:Ex)?(?:A|W)?\b/,
  ];
  if (
    processPatterns.some((pattern) => pattern.test(source))
    && !boundedProcessAdapterPaths.has(normalizedRepositoryPath(path))
  ) {
    violations.push(`${displayPath}: product child-process primitive`);
  }
}

const rendererRoots = [
  join(root, "apps", "desktop-ui", "src"),
  join(root, "packages", "ui", "src"),
];
for (const rendererRoot of rendererRoots) {
  for (const path of await walk(rendererRoot, [".ts", ".tsx", ".js", ".jsx"])) {
    const source = await requiredText(path);
    if (source === undefined) continue;
    const displayPath = relative(root, path);
    if (referenceVaultPattern.test(source)) {
      violations.push(`${displayPath}: renderer references the reference vault`);
    }
    const forbidden = [
      [/@tauri-apps\/plugin-(?:fs|shell|http|sql|process|updater)/, "broad Tauri plugin"],
      [/\b(?:fetch|XMLHttpRequest|WebSocket)\s*\(/, "renderer network primitive"],
      [/\b(?:localStorage|sessionStorage|indexedDB)\b/, "renderer durable storage"],
      [/\b(?:run_shell|spawn|execute_sql|apply_patch_text|write_path|read_path)\b/, "forbidden IPC primitive"],
    ];
    for (const [pattern, label] of forbidden) {
      if (pattern.test(source)) violations.push(`${displayPath}: ${label}`);
    }
  }
}

const hostCommandsPath = join(root, "crates", "desktop-app", "src", "commands.rs");
const hostCommandsSource = await requiredText(hostCommandsPath);
const rendererClientPath = join(
  root,
  "apps",
  "desktop-ui",
  "src",
  "lib",
  "hostClient",
  "contracts.ts",
);
const rendererClientSource = await requiredText(rendererClientPath);
const ipcEnvelopePath = join(root, "crates", "desktop-ipc", "src", "envelope.rs");
const ipcEnvelopeSource = await requiredText(ipcEnvelopePath);
const runtimeCommandPath = join(root, "crates", "desktop-runtime", "src", "command.rs");
const runtimeCommandSource = await requiredText(runtimeCommandPath);
const catalogTestPath = join(
  root,
  "apps",
  "desktop-ui",
  "src",
  "lib",
  "hostClient",
  "commandCatalog.test.ts",
);
const catalogTestSource = await requiredText(catalogTestPath);
const editsPath = join(root, "crates", "desktop-app", "src", "edits.rs");
const editsSource = await requiredText(editsPath);
const storeExecutionPath = join(root, "crates", "desktop-store", "src", "execution.rs");
const storeExecutionSource = await requiredText(storeExecutionPath);
const updatePath = join(root, "crates", "desktop-update", "src", "lib.rs");
const updateSource = await requiredText(updatePath);
if (hostCommandsSource !== undefined) {
  const readySource = /const READY_COMMANDS:[^=]+\[([\s\S]*?)\];/.exec(hostCommandsSource)?.[1];
  const recoverySource = /const RECOVERY_COMMANDS:[^=]+\[([\s\S]*?)\];/.exec(hostCommandsSource)?.[1];
  if (readySource === undefined || !sameOrderedValues(quotedStrings(readySource), reviewedReadyCommands)) {
    violations.push(`${relative(root, hostCommandsPath)}: ready capability projection drifted from the reviewed command catalog`);
  }
  if (recoverySource === undefined || !sameOrderedValues(quotedStrings(recoverySource), recoveryCommands)) {
    violations.push(`${relative(root, hostCommandsPath)}: recovery capability projection is not fail-closed`);
  }
  if (!/allowed_commands:\s*supported_commands\(state\.boot_mode\(\)\)/.test(hostCommandsSource)) {
    violations.push(`${relative(root, hostCommandsPath)}: dispatch is not bound to the current capability projection`);
  }
  const cachePolicySource = /fn should_cache_reply\([^)]*\)[^{]*\{([\s\S]*?)\n\}/.exec(
    hostCommandsSource,
  )?.[1];
  const recoveryExcludedFromReplyCache = cachePolicySource !== undefined
    && /LocalCommand::PrepareChangesRecovery\s*\{\s*\.\.\s*\}/u.test(cachePolicySource)
    && /LocalCommand::DecideChangesRecovery\s*\{\s*\.\.\s*\}/u.test(cachePolicySource);
  if (!recoveryExcludedFromReplyCache) {
    violations.push(`${relative(root, hostCommandsPath)}: recovery commands must remain excluded from reply caching`);
  }
  if (
    !/crate::recovery::prepare_recovery\([\s\S]{0,160}renderer_session/u.test(hostCommandsSource)
    || !/crate::recovery::decide_recovery\([\s\S]{0,160}renderer_session/u.test(hostCommandsSource)
  ) {
    violations.push(`${relative(root, hostCommandsPath)}: recovery dispatch must pass authenticated renderer authority`);
  }
}
if (rendererClientSource !== undefined) {
  const clientCatalogSource = /export const desktopHostCommands\s*=\s*\[([\s\S]*?)\]\s*as const/.exec(
    rendererClientSource,
  )?.[1];
  const clientCatalogValues = clientCatalogSource === undefined
    ? undefined
    : quotedStrings(expandBmadModelCommandAliases(clientCatalogSource));
  if (
    clientCatalogValues === undefined
    || !sameOrderedValues(clientCatalogValues, reviewedReadyCommands)
  ) {
    violations.push(`${relative(root, rendererClientPath)}: renderer command catalog drifted from the host projection`);
  }
}
if (catalogTestSource !== undefined) {
  const expectedCatalogSource = /expect\(desktopHostCommands\)\.toEqual\(\[([\s\S]*?)\]\);/.exec(
    catalogTestSource,
  )?.[1];
  if (
    expectedCatalogSource === undefined
    || !sameOrderedValues(quotedStrings(expectedCatalogSource), reviewedReadyCommands)
  ) {
    violations.push(`${relative(root, catalogTestPath)}: tested renderer catalog drifted from the reviewed command catalog`);
  }
}
if (ipcEnvelopeSource !== undefined) {
  const knownCommandSource = /fn is_known_command\([^)]*\)[^{]*\{([\s\S]*?)\n\}/.exec(
    ipcEnvelopeSource,
  )?.[1];
  if (
    knownCommandSource === undefined
    || !sameOrderedValues(quotedStrings(knownCommandSource), reviewedReadyCommands)
  ) {
    violations.push(`${relative(root, ipcEnvelopePath)}: build-known IPC catalog drifted from the reviewed command catalog`);
  }
  const preparePayload = /struct PrepareChangesRecoveryPayload\s*\{([\s\S]*?)\n\}/.exec(
    ipcEnvelopeSource,
  )?.[1];
  const decidePayload = /struct DecideChangesRecoveryPayload\s*\{([\s\S]*?)\n\}/.exec(
    ipcEnvelopeSource,
  )?.[1];
  const recoveryPayloadFields = (source) => source === undefined
    ? []
    : [...source.matchAll(/^\s*([a-z][a-z0-9_]*):/gmu)].map((match) => match[1]);
  if (
    !sameOrderedValues(
      recoveryPayloadFields(preparePayload),
      ["workspace_id", "workspace_grant_epoch", "journal_id"],
    )
    || !sameOrderedValues(
      recoveryPayloadFields(decidePayload),
      ["recovery_approval_id", "displayed_recovery_hash", "choice"],
    )
  ) {
    violations.push(`${relative(root, ipcEnvelopePath)}: recovery payload accepted fields drifted from the closed contract`);
  }
}
if (runtimeCommandSource !== undefined) {
  if (
    !/Self::PrepareChangesRecovery\s*\{\s*\.\.\s*\}\s*=>\s*"changes\.recovery\.prepare"/u.test(runtimeCommandSource)
    || !/Self::DecideChangesRecovery\s*\{\s*\.\.\s*\}\s*=>\s*"changes\.recovery\.decide"/u.test(runtimeCommandSource)
    || !/Self::DecideApproval\s*\{\s*\.\.\s*\}\s*=>\s*"approval\.decide"/u.test(runtimeCommandSource)
  ) {
    violations.push(`${relative(root, runtimeCommandPath)}: recovery authority is not separated from ordinary approval`);
  }
}
if (editsSource !== undefined) {
  const openRecoveryProjection = /"recovery_required"\s*\|\s*"restoring"\s*\|\s*"manual_review"/u;
  if (!openRecoveryProjection.test(editsSource)) {
    violations.push(`${relative(root, editsPath)}: unresolved recovery journals are not all projected as open`);
  }
}
if (storeExecutionSource !== undefined && updateSource !== undefined) {
  const unresolvedRecoveryTest = /assert_eq!\(states,\s*\["recovery_required",\s*"restoring",\s*"manual_review"\]\)/u;
  const updateBlocksOpenJournal = /if has_active_journal\s*\{[\s\S]{0,160}UpdateBlockReason::ActiveEffectJournal/u;
  if (
    !sameOrderedValues(updateBlockingRecoveryStates, ["recovery_required", "restoring", "manual_review"])
    || !unresolvedRecoveryTest.test(storeExecutionSource)
    || !updateBlocksOpenJournal.test(updateSource)
  ) {
    violations.push("unresolved recovery journals must remain update-blocking");
  }
}

const capabilityPath = join(root, "crates", "desktop-app", "capabilities", "main.json");
const capability = await requiredJson(capabilityPath);
const allowedPermissions = new Set([
  "allow-host-bootstrap",
  "allow-host-dispatch",
  "allow-host-projection-snapshot",
  "allow-host-projection-events",
  "core:window:allow-close",
  "core:window:allow-minimize",
  "core:window:allow-toggle-maximize",
  "core:window:allow-start-dragging",
]);
if (capability) {
  if (
    capability.identifier !== "main-workbench"
    || capability.local !== true
    || JSON.stringify(capability.windows) !== '["main"]'
  ) {
    violations.push(`${relative(root, capabilityPath)}: capability is not bound to the local main window`);
  }
  if (JSON.stringify(capability.platforms) !== '["windows"]') {
    violations.push(`${relative(root, capabilityPath)}: capability must target Windows only`);
  }
  if (containsReferenceVault(capability)) {
    violations.push(`${relative(root, capabilityPath)}: capability references the reference vault`);
  }
  if (!Array.isArray(capability.permissions)) {
    violations.push(`${relative(root, capabilityPath)}: permissions must be an explicit array`);
  } else {
    if (new Set(capability.permissions).size !== capability.permissions.length) {
      violations.push(`${relative(root, capabilityPath)}: duplicate permissions are forbidden`);
    }
    for (const permission of capability.permissions) {
      if (!allowedPermissions.has(permission)) {
        violations.push(`${relative(root, capabilityPath)}: unexpected permission ${String(permission)}`);
      }
    }
    for (const permission of allowedPermissions) {
      if (!capability.permissions.includes(permission)) {
        violations.push(`${relative(root, capabilityPath)}: required narrow permission ${permission} is missing`);
      }
    }
  }
}

const configPath = join(root, "crates", "desktop-app", "tauri.conf.json");
const config = await requiredJson(configPath);
if (config) {
  const security = config.app?.security;
  if (config.app?.withGlobalTauri !== false || security?.assetProtocol?.enable !== false) {
    violations.push(`${relative(root, configPath)}: global Tauri or asset protocol is enabled`);
  }
  if (!Array.isArray(config.app?.windows) || config.app.windows.length !== 0) {
    violations.push(`${relative(root, configPath)}: windows must be created by the guarded composition root`);
  }
  if (security?.freezePrototype !== true || security?.dangerousDisableAssetCspModification !== false) {
    violations.push(`${relative(root, configPath)}: renderer prototype or asset CSP hardening is disabled`);
  }
  if (config.build?.frontendDist !== "../../apps/desktop-ui/dist") {
    violations.push(`${relative(root, configPath)}: production frontendDist is not the reviewed renderer output`);
  }
  const expectedBeforeDevCommand =
    "pnpm --filter @sapphirus/desktop-ui dev --host 127.0.0.1";
  const expectedBeforeBuildCommand = "pnpm --filter @sapphirus/desktop-ui build";
  if (config.build?.beforeDevCommand !== expectedBeforeDevCommand) {
    violations.push(
      `${relative(root, configPath)}: beforeDevCommand must stay in the repository workspace`,
    );
  }
  if (config.build?.beforeBuildCommand !== expectedBeforeBuildCommand) {
    violations.push(
      `${relative(root, configPath)}: beforeBuildCommand must stay in the repository workspace`,
    );
  }
  if (containsReferenceVault(config)) {
    violations.push(`${relative(root, configPath)}: production configuration references the reference vault`);
  }
  if (
    config.bundle?.externalBin !== undefined
    && (!Array.isArray(config.bundle.externalBin) || config.bundle.externalBin.length > 0)
  ) {
    violations.push(`${relative(root, configPath)}: product sidecar binaries are forbidden`);
  }
  if (JSON.stringify(security?.capabilities) !== '["main-workbench"]') {
    violations.push(`${relative(root, configPath)}: production capability list must be exactly main-workbench`);
  }
  const productionCsp = typeof security?.csp === "string" ? security.csp : "";
  validateProductionCsp(productionCsp, relative(root, configPath));
  if (config.bundle?.createUpdaterArtifacts !== false) {
    violations.push(`${relative(root, configPath)}: updater artifacts enabled before organization signing is configured`);
  }
  if (config.bundle?.windows?.nsis?.installMode !== "currentUser") {
    violations.push(`${relative(root, configPath)}: internal NSIS build is not current-user scoped`);
  }
}

{
  const scorecardPath = join(root, "docs", "readiness", "100-percent-scorecard.json");
  const expectedCapabilities = [
    "bmad_foundation",
    "full_bmad_breadth",
    "offline_developer_checkout",
    "reproducible_installable_offline_prototype",
    "complete_current_source_installable_exe",
    "deterministic_help_backend",
    "user_facing_deterministic_help",
    "production_model_backed_help",
    "d3_governed_edits_backend",
    "user_facing_governed_edits",
    "integrated_d2_d3_desktop",
    "first_honest_ai_desktop_prototype",
    "horizontal_governed_foundation",
    "internal_pilot_readiness",
  ];
  try {
    const scorecard = JSON.parse(await readFile(scorecardPath, "utf8"));
    const actual = (scorecard.capabilities ?? []).map((record) => record.capability);
    if (!sameOrderedValues(actual, expectedCapabilities)) {
      violations.push("docs/readiness/100-percent-scorecard.json: capability key set drifted from the reviewed 14");
    }
  } catch {
    violations.push("docs/readiness/100-percent-scorecard.json: missing or unreadable readiness scorecard");
  }
}

if (violations.length > 0) {
  console.error("Architecture boundary violations:\n" + violations.map((item) => `- ${item}`).join("\n"));
  process.exit(1);
}

console.log("Architecture boundaries verified from the Cargo lock, source, capabilities, and Tauri config.");
