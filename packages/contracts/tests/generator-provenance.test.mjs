import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const repositoryRoot = path.resolve(packageRoot, "..", "..");
const generatorPath = path.join(packageRoot, "scripts", "generate.mjs");
const schemaLockPath = path.join(packageRoot, "schema-lock.json");
const packageManifestPath = path.join(packageRoot, "package.json");
const rootCargoManifestPath = path.join(repositoryRoot, "Cargo.toml");
const rootCargoLockPath = path.join(repositoryRoot, "Cargo.lock");

const [
  generatorSource,
  schemaLockSource,
  packageManifestSource,
  rootCargoManifestSource,
  rootCargoLockSource,
] = await Promise.all([
  readFile(generatorPath, "utf8"),
  readFile(schemaLockPath, "utf8"),
  readFile(packageManifestPath, "utf8"),
  readFile(rootCargoManifestPath, "utf8"),
  readFile(rootCargoLockPath, "utf8"),
]);
const schemaLock = JSON.parse(schemaLockSource);
const packageManifest = JSON.parse(packageManifestSource);

const expectedToolLock = Object.freeze({
  schemaVersion: "sapphirus.contract-codegen-tool-lock.v1",
  rust: Object.freeze({
    identity: "cargo-typify@0.6.1",
    packageId: "cargo-typify",
    version: "0.6.1",
    command: "cargo-typify.exe",
    resolvedExecutable: "target/contract-tools/bin/cargo-typify.exe",
    packageSource: "crates.io",
    packageSha256: "dacf8eaa5f73f53e96392b36723d37e110c51f0e596c5e158a16c37190c5f7ee",
    executableIdentity: Object.freeze({
      algorithm: "pe-coff-debug-normalized-sha256-v1",
      fileSize: 8167936,
      normalizedSha256: "29eee6240f4657e66504be3e1195366a8fc201085c41a53af9e3d9ea556ee56d",
    }),
    versionArguments: Object.freeze(["typify", "--version"]),
    versionExitCode: 0,
  }),
  dotnet: Object.freeze({
    identity: "Corvus.Json.Cli@5.1.0",
    packageId: "Corvus.Json.Cli",
    version: "5.1.0",
    command: "corvusjson",
    toolManifest: ".config/dotnet-tools.json",
    packageSource: "nuget.org",
    packageSha256: "d621eb857fcb073ebd6f59d4b820e339df8725a1a466c031f981cca2cd517343",
    packageSha512:
      "CSFbHpadwPMujU7EVXBRq86B+dRFXQ+FUU57e0YGA5mQGbIoUWGpCXqTiJNR3gMaK2UnB8epeFD7AqVGPgCmLA==",
    sdkExecutable: "C:/Program Files/dotnet/dotnet.exe",
    sdkAuthenticode: Object.freeze({
      status: "Valid",
      signatureType: "Authenticode",
      signerSubject: "CN=.NET, O=Microsoft Corporation, L=Redmond, S=Washington, C=US",
      signerThumbprint: "BB793DB742624269BB5F4515BBE9A3DF418F588D",
      originalFilename: ".NET Host",
    }),
    toolClosure: Object.freeze({
      archivePrefix: "tools/net10.0/any/",
      entryPoint: "Corvus.Json.Cli.dll",
      fileCount: 359,
      treeSha256: "d78b66ac06257175cbaa668c1c0946e03048d03cce1c7cce59cf3e537427dfc3",
    }),
    versionArguments: Object.freeze(["version"]),
    versionExitCode: 1,
  }),
});
const expectedRustGenerationArguments = Object.freeze([
  "typify",
  "{input}",
  "--output",
  "{output}",
]);
const expectedDotnetGenerationArguments = Object.freeze([
  "jsonschema",
  "{input}",
  "--engine",
  "V5",
  "--useSchema",
  "Draft202012",
  "--useUnixLineEndings",
  "true",
  "--optionalAsNullable",
  "None",
  "--assertFormat",
  "false",
  "--disableOptionalNamingHeuristics",
  "true",
  "--rootNamespace",
  "{namespace}",
  "--outputRootTypeName",
  "{rootType}",
  "--outputRootAccessibility",
  "Public",
  "--defaultAccessibility",
  "Public",
  "--outputPath",
  "{output}",
]);

function parseJsonArtifactSource(source, relativePath) {
  if (source.trim().length === 0) {
    return { error: `${relativePath} is empty.`, source, value: null };
  }

  try {
    return { error: null, source, value: JSON.parse(source) };
  } catch (error) {
    return {
      error: `${relativePath} is malformed JSON: ${error.message}`,
      source,
      value: null,
    };
  }
}

async function loadJsonArtifact(relativePath) {
  try {
    const source = await readFile(path.join(repositoryRoot, relativePath), "utf8");
    return parseJsonArtifactSource(source, relativePath);
  } catch (error) {
    if (error?.code === "ENOENT") {
      return {
        error: `Missing required JSON artifact ${relativePath}.`,
        source: "",
        value: null,
      };
    }
    throw error;
  }
}

async function nonEmptyFileStatus(relativePath) {
  try {
    const fileStat = await stat(path.join(repositoryRoot, relativePath));
    if (!fileStat.isFile()) return `${relativePath} is not a regular file.`;
    if (fileStat.size === 0) return `${relativePath} is empty.`;
    return null;
  } catch (error) {
    if (error?.code === "ENOENT") return `${relativePath} is missing.`;
    throw error;
  }
}

function assertExactNativeToolLock(toolLock) {
  assert.equal(toolLock?.schemaVersion, expectedToolLock.schemaVersion);

  const rust = toolLock?.tools?.rust;
  assert.deepEqual(
    {
      identity: rust?.identity,
      packageId: rust?.packageId,
      version: rust?.version,
      command: rust?.command,
      resolvedExecutable: rust?.resolvedExecutable,
      packageSource: rust?.packageSource,
      packageSha256: rust?.packageSha256,
      executableIdentity: rust?.executableIdentity,
      versionArguments: rust?.versionArguments,
      versionExitCode: rust?.versionExitCode,
    },
    expectedToolLock.rust,
  );
  assert.deepEqual(rust?.generationArguments, expectedRustGenerationArguments);

  const dotnet = toolLock?.tools?.dotnet;
  assert.deepEqual(
    {
      identity: dotnet?.identity,
      packageId: dotnet?.packageId,
      version: dotnet?.version,
      command: dotnet?.command,
      toolManifest: dotnet?.toolManifest,
      packageSource: dotnet?.packageSource,
      packageSha256: dotnet?.packageSha256,
      packageSha512: dotnet?.packageSha512,
      sdkExecutable: dotnet?.sdkExecutable,
      sdkAuthenticode: dotnet?.sdkAuthenticode,
      toolClosure: dotnet?.toolClosure,
      versionArguments: dotnet?.versionArguments,
      versionExitCode: dotnet?.versionExitCode,
    },
    expectedToolLock.dotnet,
  );
  assert.deepEqual(dotnet?.generationArguments, expectedDotnetGenerationArguments);
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function createValidToolLockFixture() {
  return {
    schemaVersion: expectedToolLock.schemaVersion,
    tools: {
      rust: {
        ...expectedToolLock.rust,
        versionArguments: [...expectedToolLock.rust.versionArguments],
        generationArguments: [...expectedRustGenerationArguments],
      },
      dotnet: {
        ...expectedToolLock.dotnet,
        versionArguments: [...expectedToolLock.dotnet.versionArguments],
        generationArguments: [...expectedDotnetGenerationArguments],
      },
    },
  };
}

function tomlSection(source, sectionName) {
  const lines = source.split(/\r?\n/u);
  const headerIndex = lines.findIndex((line) => line.trim() === `[${sectionName}]`);
  if (headerIndex === -1) return "";

  const sectionLines = [];
  for (const line of lines.slice(headerIndex + 1)) {
    if (/^\s*\[[^\]]+\]\s*$/u.test(line)) break;
    sectionLines.push(line);
  }
  return sectionLines.join("\n");
}

function cargoWorkspaceMembers(source) {
  const membersSource = tomlSection(source, "workspace").match(
    /^members\s*=\s*\[([\s\S]*?)\]/mu,
  )?.[1];
  if (!membersSource) return [];
  return [...membersSource.matchAll(/"([^"]+)"/gu)].map((match) => match[1]);
}

function cargoLockHasPackage(source, packageName, packageVersion) {
  return source
    .split(/^\[\[package\]\]\s*$/mu)
    .some(
      (block) =>
        new RegExp(`^name = "${packageName}"$`, "mu").test(block)
        && new RegExp(`^version = "${packageVersion.replaceAll(".", "\\.")}"$`, "mu").test(block),
    );
}

async function assertFixtureCatalog(relativePath) {
  const artifact = await loadJsonArtifact(relativePath);
  assert.equal(artifact.error, null, artifact.error);
  const catalog = artifact.value;
  assert.deepEqual(
    Object.keys(catalog ?? {}).sort(),
    ["fixtures", "parserLimits", "reasonPriority", "resources", "rootSchema", "schemaVersion"],
    `${relativePath} must use the closed qualification catalog shape.`,
  );
  assert.equal(catalog.schemaVersion, "sapphirus.generator-qualification.catalog.v1");
  assert.equal(catalog.rootSchema, "schemas/qualification.schema.json");
  assert.deepEqual(catalog.resources, ["schemas/qualification-external.schema.json"]);
  assert.deepEqual(catalog.parserLimits, { maxBytes: 262144, maxContainerDepth: 8 });
  assert.ok(
    Array.isArray(catalog.reasonPriority) && catalog.reasonPriority.length > 0,
    `${relativePath} must define non-empty reasonPriority.`,
  );
  assert.equal(
    new Set(catalog.reasonPriority).size,
    catalog.reasonPriority.length,
    `${relativePath} reasonPriority entries must be unique.`,
  );
  for (const reason of catalog.reasonPriority) {
    assert.equal(typeof reason, "string", `${relativePath} reasonPriority entries must be strings.`);
    assert.ok(reason.length > 0, `${relativePath} reasonPriority entries must not be empty.`);
  }
  assert.ok(
    Array.isArray(catalog.fixtures) && catalog.fixtures.length > 0,
    `${relativePath} must catalog at least one fixture.`,
  );

  const fixturesRoot = path.join(repositoryRoot, "tests", "generator-qualification", "fixtures");
  const validitySeen = new Set();
  const idsSeen = new Set();
  const filesSeen = new Set();
  const requiredFields = [
    "covers",
    "expected",
    "file",
    "id",
    "reasonCategory",
    "rejectionStage",
    "validatorInvoked",
  ];
  const optionalFields = ["expectedCanonicalJson", "expectedHash"];
  const allowedFields = new Set([...requiredFields, ...optionalFields]);
  const validStages = new Set(["none", "strict_parser", "structural_validator"]);
  for (const entry of catalog.fixtures) {
    const entryFields = Object.keys(entry ?? {});
    assert.deepEqual(
      requiredFields.filter((field) => !entryFields.includes(field)),
      [],
      `${relativePath} fixture is missing required metadata.`,
    );
    assert.deepEqual(
      entryFields.filter((field) => !allowedFields.has(field)),
      [],
      `${relativePath} fixture contains unsupported metadata.`,
    );
    assert.equal(typeof entry.id, "string", `${relativePath} fixture id must be a string.`);
    assert.ok(entry.id.length > 0, `${relativePath} fixture id must not be empty.`);
    assert.ok(!idsSeen.has(entry.id), `${entry.id} is duplicated in the catalog.`);
    idsSeen.add(entry.id);

    assert.ok(
      entry.expected === "accept" || entry.expected === "reject",
      `${entry.id} expected must be accept or reject.`,
    );
    validitySeen.add(entry.expected);
    const accepted = entry.expected === "accept";
    const expectedDirectory = accepted ? "valid" : "invalid";
    assert.equal(typeof entry.file, "string", `${entry.id} must declare a fixture file.`);
    assert.ok(
      entry.file.startsWith(`fixtures/${expectedDirectory}/`),
      `${entry.file} must stay under fixtures/${expectedDirectory}/.`,
    );
    assert.ok(!entry.file.includes("\\"), `${entry.file} must use portable forward slashes.`);
    assert.ok(!path.posix.isAbsolute(entry.file), `${entry.file} must be relative.`);
    assert.ok(!entry.file.split("/").includes(".."), `${entry.file} must not traverse directories.`);
    if (accepted) {
      assert.equal(entry.reasonCategory, null, `${entry.file} accepted reasonCategory must be null.`);
      assert.equal(entry.rejectionStage, "none", `${entry.file} accepted stage must be none.`);
      assert.equal(entry.validatorInvoked, true, `${entry.file} must invoke the validator.`);
    } else {
      assert.equal(
        typeof entry.reasonCategory,
        "string",
        `${entry.file} must declare a repository-owned reasonCategory.`,
      );
      assert.ok(entry.reasonCategory.length > 0, `${entry.file} reasonCategory must not be empty.`);
      assert.ok(
        catalog.reasonPriority.includes(entry.reasonCategory),
        `${entry.file} reasonCategory must occur in reasonPriority.`,
      );
      assert.ok(
        entry.rejectionStage === "strict_parser"
          || entry.rejectionStage === "structural_validator",
        `${entry.file} rejected stage must identify parser or validator.`,
      );
      assert.equal(
        entry.validatorInvoked,
        entry.rejectionStage === "structural_validator",
        `${entry.file} validatorInvoked contradicts rejectionStage.`,
      );
    }
    assert.ok(validStages.has(entry.rejectionStage), `${entry.file} has an invalid rejectionStage.`);
    assert.equal(
      typeof entry.validatorInvoked,
      "boolean",
      `${entry.file} validatorInvoked must be boolean.`,
    );
    assert.ok(Array.isArray(entry.covers) && entry.covers.length > 0, `${entry.file} covers is empty.`);
    assert.equal(
      new Set(entry.covers).size,
      entry.covers.length,
      `${entry.file} covers entries must be unique.`,
    );
    for (const coverage of entry.covers) {
      assert.equal(typeof coverage, "string", `${entry.file} covers entries must be strings.`);
      assert.ok(coverage.length > 0, `${entry.file} covers entries must not be empty.`);
    }
    if (Object.hasOwn(entry, "expectedCanonicalJson")) {
      assert.equal(
        typeof entry.expectedCanonicalJson,
        "string",
        `${entry.file} expectedCanonicalJson must be a string.`,
      );
    }
    if (Object.hasOwn(entry, "expectedHash")) {
      assert.match(
        entry.expectedHash,
        /^sha256:[0-9a-f]{64}$/u,
        `${entry.file} expectedHash must be a SHA-256 value.`,
      );
    }

    assert.ok(!filesSeen.has(entry.file), `${entry.file} is duplicated in the catalog.`);
    filesSeen.add(entry.file);

    const fixturePath = path.resolve(
      repositoryRoot,
      "tests",
      "generator-qualification",
      entry.file,
    );
    assert.ok(
      fixturePath.startsWith(`${fixturesRoot}${path.sep}`),
      `${entry.file} escapes the fixture root.`,
    );
    const fixtureRelativePath = path.relative(repositoryRoot, fixturePath).replaceAll("\\", "/");
    assert.equal(
      await nonEmptyFileStatus(fixtureRelativePath),
      null,
      `${entry.file} must resolve to a non-empty fixture.`,
    );
  }
  assert.deepEqual(
    [...validitySeen].sort(),
    ["accept", "reject"],
    `${relativePath} must catalog both valid and invalid fixtures.`,
  );
}

test("BMAD-G0 forbids handwritten Rust and C# contract emitters", () => {
  for (const emitter of ["getRustContracts", "getDotnetContracts"]) {
    assert.ok(
      !generatorSource.includes(emitter),
      `${path.relative(repositoryRoot, generatorPath)} still contains forbidden emitter ${emitter}.`,
    );
  }
});

test("BMAD-G0 contains no deferred native-language coverage marker", () => {
  for (const [relativePath, source] of [
    [path.relative(repositoryRoot, generatorPath), generatorSource],
    [path.relative(repositoryRoot, schemaLockPath), schemaLockSource],
  ]) {
    assert.ok(
      !source.includes("deferred_for_new_families"),
      `${relativePath} still contains deferred_for_new_families.`,
    );
  }
});

test("BMAD-G0 qualification layout contains non-empty seed artifacts", async () => {
  // Catalog and native test names fix one conventional, implementation-feasible layout where
  // the packet leaves filenames open. Executable qualification owns semantic completeness.
  const requiredFiles = [
    ".config/dotnet-tools.json",
    "tools/contract-codegen/tool-lock.json",
    "packages/contracts/scripts/qualify-generators.mjs",
    "tests/generator-qualification/schemas/qualification.schema.json",
    "tests/generator-qualification/schemas/qualification-external.schema.json",
    "tests/generator-qualification/catalog.json",
    "tests/generator-qualification/rust/Cargo.toml",
    "tests/generator-qualification/rust/src/lib.rs",
    "tests/generator-qualification/rust/tests/qualification.rs",
    "tests/generator-qualification/dotnet/Sapphirus.GeneratorQualification.Tests.csproj",
    "tests/generator-qualification/dotnet/GeneratorQualificationTests.cs",
    "tests/generator-qualification/dotnet/packages.lock.json",
  ];
  const failures = [];

  for (const relativePath of requiredFiles) {
    const failure = await nonEmptyFileStatus(relativePath);
    if (failure) failures.push(failure);
  }

  assert.deepEqual(failures, [], `Incomplete BMAD-G0 layout: ${failures.join(" ")}`);
});

test("BMAD-G0 tool lock binds exact native package provenance structurally", async () => {
  const artifact = await loadJsonArtifact("tools/contract-codegen/tool-lock.json");
  assert.equal(artifact.error, null, artifact.error);
  assertExactNativeToolLock(artifact.value);
});

test("tool-lock validation rejects substitution, truncation, and unrelated evidence", () => {
  assert.doesNotThrow(() => assertExactNativeToolLock(createValidToolLockFixture()));

  const mutations = [
    ["Rust version prefix", (lock) => { lock.tools.rust.version = "0.6.10"; }],
    ["C# version suffix", (lock) => { lock.tools.dotnet.version = "5.1.00"; }],
    [
      "global Rust executable",
      (lock) => { lock.tools.rust.resolvedExecutable = "C:/Users/example/.cargo/bin/cargo-typify.exe"; },
    ],
    ["wrong C# command", (lock) => { lock.tools.dotnet.command = "corvus"; }],
    ["normalized C# version exit", (lock) => { lock.tools.dotnet.versionExitCode = 0; }],
    ["missing Rust checksum", (lock) => { delete lock.tools.rust.packageSha256; }],
    ["missing fixed arguments", (lock) => { delete lock.tools.dotnet.generationArguments; }],
    [
      "identity only in an unrelated field",
      (lock) => {
        lock.tools.rust.packageId = "typify-cli";
        lock.tools.rust.note = expectedToolLock.rust.identity;
      },
    ],
  ];

  for (const [label, mutate] of mutations) {
    const candidate = clone(createValidToolLockFixture());
    mutate(candidate);
    assert.throws(() => assertExactNativeToolLock(candidate), label);
  }

  assert.match(
    parseJsonArtifactSource('{"tools":', "tools/contract-codegen/tool-lock.json").error,
    /malformed JSON/u,
  );
});

test("BMAD-G0 .NET tool manifest binds the exact Corvus package and command", async () => {
  const artifact = await loadJsonArtifact(".config/dotnet-tools.json");
  assert.equal(artifact.error, null, artifact.error);
  const tool = artifact.value?.tools?.["corvus.json.cli"];

  assert.deepEqual(
    {
      isRoot: artifact.value?.isRoot,
      manifestVersion: artifact.value?.version,
      packageKeys: Object.keys(artifact.value?.tools ?? {}).sort(),
      tool,
    },
    {
      isRoot: true,
      manifestVersion: 1,
      packageKeys: ["corvus.json.cli"],
      tool: {
        version: "5.1.0",
        commands: ["corvusjson"],
        rollForward: false,
      },
    },
  );
});

test("BMAD-G0 records cargo-typify 0.6.1 as the exact Rust generator", () => {
  const expectedIdentity = "cargo-typify@0.6.1";

  assert.deepEqual(
    {
      packageManifest: packageManifest.sapphirusToolchain?.rustGenerator,
      schemaLock: schemaLock.generators?.rust,
    },
    {
      packageManifest: expectedIdentity,
      schemaLock: expectedIdentity,
    },
  );
  for (const source of [generatorSource, schemaLockSource, packageManifestSource]) {
    assert.ok(!source.includes("typify-cli@0.6.1"), "Legacy typify-cli identity remains.");
  }
});

test("BMAD-G0 records Corvus.Json.Cli 5.1.0 as the exact C# generator", () => {
  const expectedIdentity = "Corvus.Json.Cli@5.1.0";

  assert.deepEqual(
    {
      packageManifest: packageManifest.sapphirusToolchain?.dotnetGenerator,
      schemaLock: schemaLock.generators?.dotnet,
    },
    {
      packageManifest: expectedIdentity,
      schemaLock: expectedIdentity,
    },
  );
  for (const source of [generatorSource, schemaLockSource, packageManifestSource]) {
    assert.ok(
      !source.includes("Corvus.Json.CodeGeneration.Cli@5.1.0"),
      "Legacy Corvus.Json.CodeGeneration.Cli identity remains.",
    );
  }
});

test("BMAD-G0 qualification schemas are non-empty local Draft 2020-12 documents", async () => {
  const rootArtifact = await loadJsonArtifact(
    "tests/generator-qualification/schemas/qualification.schema.json",
  );
  const externalArtifact = await loadJsonArtifact(
    "tests/generator-qualification/schemas/qualification-external.schema.json",
  );
  for (const artifact of [rootArtifact, externalArtifact]) {
    assert.equal(artifact.error, null, artifact.error);
    assert.equal(artifact.value?.$schema, "https://json-schema.org/draft/2020-12/schema");
    assert.equal(typeof artifact.value?.$id, "string");
    assert.ok(Object.keys(artifact.value).length > 2);
  }

  const references = [];
  JSON.stringify(rootArtifact.value, (key, value) => {
    if (key === "$ref" && typeof value === "string") references.push(value);
    return value;
  });
  assert.ok(
    references.some((reference) => reference.includes("qualification-external.schema.json")),
    "Qualification root schema must exercise the local external reference.",
  );
});

test("BMAD-G0 root fixture catalog has closed metadata and contained fixtures", async () => {
  await assertFixtureCatalog("tests/generator-qualification/catalog.json");
});

test("BMAD-G0 wires the Rust qualification crate and exact validator into the root lock", async () => {
  const rustManifestPath = "tests/generator-qualification/rust/Cargo.toml";
  const rustManifestStatus = await nonEmptyFileStatus(rustManifestPath);
  assert.equal(rustManifestStatus, null, rustManifestStatus);
  const rustManifestSource = await readFile(path.join(repositoryRoot, rustManifestPath), "utf8");

  assert.ok(
    cargoWorkspaceMembers(rootCargoManifestSource).includes("tests/generator-qualification/rust"),
    "Root Cargo workspace must contain tests/generator-qualification/rust.",
  );
  assert.match(
    tomlSection(rootCargoManifestSource, "workspace.package"),
    /^rust-version\s*=\s*"1\.97\.0"\s*$/mu,
  );
  assert.match(
    tomlSection(rustManifestSource, "dependencies"),
    /^jsonschema\s*=\s*"=0\.44\.1"\s*$/mu,
  );
  assert.ok(
    cargoLockHasPackage(rootCargoLockSource, "jsonschema", "0.44.1"),
    "Root Cargo.lock must resolve jsonschema 0.44.1.",
  );
});

test("BMAD-G0 C# qualification project pins its runtime and locked restore", async () => {
  const projectPath =
    "tests/generator-qualification/dotnet/Sapphirus.GeneratorQualification.Tests.csproj";
  const projectStatus = await nonEmptyFileStatus(projectPath);
  assert.equal(projectStatus, null, projectStatus);
  const projectSource = await readFile(path.join(repositoryRoot, projectPath), "utf8");
  assert.match(projectSource, /<TargetFramework>\s*net10\.0\s*<\/TargetFramework>/u);
  assert.match(
    projectSource,
    /<RestorePackagesWithLockFile>\s*true\s*<\/RestorePackagesWithLockFile>/u,
  );

  const corvusReference = [...projectSource.matchAll(/<PackageReference\b([^>]*?)\/?\s*>/gu)]
    .map((match) => match[1])
    .find((attributes) => /\bInclude\s*=\s*["']Corvus\.Text\.Json["']/u.test(attributes));
  assert.ok(corvusReference, "C# project must reference Corvus.Text.Json.");
  assert.match(corvusReference, /\bVersion\s*=\s*["']\[5\.1\.0\]["']/u);

  const packageLockArtifact = await loadJsonArtifact(
    "tests/generator-qualification/dotnet/packages.lock.json",
  );
  assert.equal(packageLockArtifact.error, null, packageLockArtifact.error);
  const targetDependencies = Object.values(packageLockArtifact.value?.dependencies ?? {});
  const lockedCorvusEntries = targetDependencies
    .map((dependencies) => dependencies?.["Corvus.Text.Json"])
    .filter(Boolean);
  assert.ok(lockedCorvusEntries.length > 0, "NuGet lock must contain Corvus.Text.Json.");
  for (const entry of lockedCorvusEntries) {
    assert.equal(entry.requested, "[5.1.0, 5.1.0]");
    assert.equal(entry.resolved, "5.1.0");
    assert.equal(typeof entry.contentHash, "string");
    assert.ok(entry.contentHash.length > 0);
  }
});
