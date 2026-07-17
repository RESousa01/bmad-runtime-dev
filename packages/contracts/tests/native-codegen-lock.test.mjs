import assert from "node:assert/strict";
import {
  copyFile,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  assertNoInheritedNativeToolInjection,
  assertInternalReferenceClosure,
  canonicalDotnetOutputRoot,
  findOptionalNullableProperties,
  normalizedPortableExecutableSha256,
  partitionOptionalNullableRoot,
  preflightNativeTools,
  resolveRepoLocalExecutable,
  transformSchemaTree,
  validateToolLock,
} from "../scripts/lib/native-codegen.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const repositoryRoot = path.resolve(packageRoot, "..", "..");
const baseline = JSON.parse(await readFile(
  path.join(repositoryRoot, "tools", "contract-codegen", "tool-lock.json"),
  "utf8",
));
const clone = () => structuredClone(baseline);

async function createContractCodegenTemp(prefix) {
  const temporaryRoot = path.join(repositoryRoot, "target", "contract-codegen");
  await mkdir(temporaryRoot, { recursive: true });
  return mkdtemp(path.join(temporaryRoot, prefix));
}

async function copyTree(sourceRoot, destinationRoot) {
  await mkdir(destinationRoot, { recursive: true });
  const entries = await readdir(sourceRoot, { withFileTypes: true });
  for (const entry of entries) {
    const source = path.join(sourceRoot, entry.name);
    const destination = path.join(destinationRoot, entry.name);
    if (entry.isDirectory()) {
      await copyTree(source, destination);
    } else {
      assert.equal(entry.isFile(), true, `${source} must be a regular test fixture file.`);
      await copyFile(source, destination);
    }
  }
}

async function withEnvironment(name, value, operation) {
  const previous = process.env[name];
  process.env[name] = value;
  try {
    return await operation();
  } finally {
    if (previous === undefined) delete process.env[name];
    else process.env[name] = previous;
  }
}

test("native codegen lock accepts the reviewed exact configuration", () => {
  assert.doesNotThrow(() => validateToolLock(clone()));
});

test("Corvus staging fixes the absolute output-root length across checkout paths", () => {
  const shortRoot = path.join("C:\\", "w", "a", "p");
  const longerRoot = path.join("C:\\", "worktrees", "sapphirus", "a", "p");
  for (const mode of ["production", "qualification"]) {
    const expectedLength = baseline.stagingPolicy.dotnetOutputPathLengths[mode];
    const shortOutput = canonicalDotnetOutputRoot(shortRoot, mode, baseline.stagingPolicy);
    const longerOutput = canonicalDotnetOutputRoot(longerRoot, mode, baseline.stagingPolicy);
    assert.equal(shortOutput.length, expectedLength);
    assert.equal(longerOutput.length, expectedLength);
    assert.match(path.basename(shortOutput), /^dotnet-+$/u);
    assert.match(path.basename(longerOutput), /^dotnet-+$/u);
  }
  assert.throws(
    () => canonicalDotnetOutputRoot(
      path.join("C:\\", "x".repeat(100)),
      "production",
      baseline.stagingPolicy,
    ),
    /CONTRACT_GENERATOR_NONDETERMINISTIC/u,
  );
});

test("native codegen lock rejects unknown and missing nested fields", () => {
  for (const mutate of [
    (lock) => { lock.tools.rust.unreviewedFallback = true; },
    (lock) => { delete lock.tools.dotnet.restoreArguments; },
    (lock) => { lock.sourceSet.production.roots[0].alias = "substituted"; },
    (lock) => { delete lock.normalizationPolicy.rejectRunPathLeaks; },
    (lock) => { lock.invocations.qualification.extraOutput = "elsewhere"; },
    (lock) => { lock.bootstrapLocks.cargo.unreviewed = true; },
    (lock) => { delete lock.tools.rust.installMetadata.rustc; },
  ]) {
    const candidate = clone();
    mutate(candidate);
    assert.throws(() => validateToolLock(candidate), /CONTRACT_LOCK_BOOTSTRAP_UNREVIEWED/u);
  }
});

test("native codegen lock rejects bootstrap, runtime, and transform substitutions", () => {
  const mutations = [
    (lock) => { lock.tools.rust.installArguments[2] = "--force"; },
    (lock) => { lock.tools.rust.runtimeValidator = "jsonschema@0.45.0"; },
    (lock) => { lock.tools.dotnet.runtime = "Corvus.Text.Json@5.2.0"; },
    (lock) => { lock.tools.dotnet.license = "UNKNOWN"; },
    (lock) => { lock.tools.typescript.packageIntegrities["ajv@8.17.1"] = "sha512-substituted"; },
    (lock) => { lock.stagingPolicy.transformVersion = "internal-$defs-bundle-v2"; },
    (lock) => { lock.stagingPolicy.productionOptionalNullablePolicy = "allow-lossy"; },
    (lock) => { lock.bootstrapLocks.cargo.status = "pending_native_harness_review"; },
    (lock) => { lock.bootstrapLocks.dotnetPackages.sha256 = "0".repeat(64); },
    (lock) => { lock.tools.rust.installMetadata.installKey = "cargo-typify 0.6.1 (substituted)"; },
    (lock) => { lock.tools.rust.license = "MIT OR Apache-2.0"; },
    (lock) => { lock.tools.dotnet.sdkExecutable = "dotnet"; },
  ];
  for (const mutate of mutations) {
    const candidate = clone();
    mutate(candidate);
    assert.throws(
      () => validateToolLock(candidate),
      /CONTRACT_(?:GENERATOR_VERSION_MISMATCH|LOCK_BOOTSTRAP_UNREVIEWED)/u,
    );
  }
});

test("optional-nullable presence analysis includes unconstrained and type-omitted schemas", () => {
  const transformed = partitionOptionalNullableRoot({
    type: "object",
    additionalProperties: false,
    properties: {
      unconstrained: {},
      booleanSchema: true,
      typeOmitted: { minLength: 2 },
      nonNullable: { type: "string" },
    },
  }, "NullPresenceProbe", {});
  assert.equal(transformed.oneOf.length, 8);
  const requiredSets = transformed.oneOf.map((variant) => new Set(variant.required));
  for (const name of ["unconstrained", "booleanSchema", "typeOmitted"]) {
    assert.equal(requiredSets.some((required) => required.has(name)), true, name);
    assert.equal(transformed.oneOf.some((variant) => !Object.hasOwn(variant.properties, name)), true, name);
  }
  assert.equal(transformed.oneOf.every((variant) => Object.hasOwn(variant.properties, "nonNullable")), true);
});

test("optional-nullable presence analysis does not traverse data-valued keywords", () => {
  const findings = findOptionalNullableProperties({
    type: "object",
    properties: {
      actualSchema: {},
    },
    const: { properties: { constData: {} } },
    default: { properties: { defaultData: {} } },
    enum: [{ properties: { enumData: {} } }],
    examples: [{ properties: { exampleData: {} } }],
    dependentRequired: { trigger: ["actualSchema"] },
    allOf: [{ properties: { nestedSchema: {} } }],
    $defs: { Reusable: { properties: { definitionSchema: {} } } },
  }, () => true);

  assert.deepEqual(findings, [
    "#/properties/actualSchema",
    "#/allOf/0/properties/nestedSchema",
    "#/$defs/Reusable/properties/definitionSchema",
  ]);
});

test("schema transform preserves data payloads and property names but rejects nested scopes", () => {
  const transformed = transformSchemaTree({
    $schema: "https://json-schema.org/draft/2020-12/schema",
    $id: "https://schemas.example.test/root.json",
    $defs: {},
    type: "object",
    properties: {
      $id: {
        const: { $id: "data-id", $ref: "data-ref", $defs: { literal: true } },
        examples: [{ $ref: "example-data" }],
      },
    },
  }, { documentRoot: true });
  assert.deepEqual(transformed.properties.$id.const, {
    $id: "data-id",
    $ref: "data-ref",
    $defs: { literal: true },
  });
  assert.deepEqual(transformed.properties.$id.examples, [{ $ref: "example-data" }]);
  const retained = transformSchemaTree({
    $schema: "https://json-schema.org/draft/2020-12/schema",
    $defs: { Local: { type: "string" } },
    const: { $ref: "data-ref" },
    $ref: "#/$defs/Local",
  }, {
    documentRoot: true,
    retainDocumentKeywords: true,
    rewriteReference: (reference) => `rewritten:${reference}`,
  });
  assert.equal(retained.$ref, "rewritten:#/$defs/Local");
  assert.deepEqual(retained.const, { $ref: "data-ref" });
  assert.deepEqual(retained.$defs, { Local: { type: "string" } });
  for (const unsafe of [
    { type: "object", properties: { value: { $id: "https://schemas.example.test/nested" } } },
    { type: "string", $anchor: "nested" },
    { type: "string", customObjectKeyword: { $ref: "https://example.test/external" } },
  ]) {
    assert.throws(
      () => transformSchemaTree(unsafe, { documentRoot: true }),
      /CONTRACT_LANGUAGE_PARITY_FAILED/u,
    );
  }
});

test("generated bundle reference closure rejects retrieval and missing targets", () => {
  assert.doesNotThrow(() => assertInternalReferenceClosure({
    $defs: { Local: { type: "string" } },
    $ref: "#/$defs/Local",
  }));
  for (const unsafe of [
    { $defs: {}, $ref: "https://example.test/remote.schema.json" },
    { $defs: {}, $ref: "file:///tmp/schema.json" },
    { $defs: {}, $ref: "#/$defs/Missing" },
    { $defs: { Local: {} }, $dynamicRef: "#/$defs/Local" },
  ]) {
    assert.throws(() => assertInternalReferenceClosure(unsafe), /CONTRACT_LANGUAGE_PARITY_FAILED/u);
  }
});

test("missing repo-local native generator reports fallback forbidden", async () => {
  await assert.rejects(
    resolveRepoLocalExecutable("target/contract-tools/bin/definitely-missing-generator.exe"),
    /CONTRACT_GENERATOR_FALLBACK_FORBIDDEN/u,
  );
});

test("cargo-typify identity normalization ignores only declared PE build metadata", async () => {
  const executable = await readFile(path.join(
    repositoryRoot,
    baseline.tools.rust.resolvedExecutable,
  ));
  const timestampVariant = Buffer.from(executable);
  const peHeaderOffset = timestampVariant.readUInt32LE(0x3c);
  timestampVariant.writeUInt32LE(
    (timestampVariant.readUInt32LE(peHeaderOffset + 8) ^ 0xffffffff) >>> 0,
    peHeaderOffset + 8,
  );
  assert.equal(
    normalizedPortableExecutableSha256(timestampVariant),
    normalizedPortableExecutableSha256(executable),
  );

  const substantiveVariant = Buffer.from(executable);
  substantiveVariant[0x50] ^= 0x20;
  assert.notEqual(
    normalizedPortableExecutableSha256(substantiveVariant),
    normalizedPortableExecutableSha256(executable),
  );
});

test("native preflight rejects a byte-tampered repo-local cargo-typify before probing it", async () => {
  const temporaryRoot = await createContractCodegenTemp("identity-test-");
  try {
    const executable = path.join(temporaryRoot, "cargo-typify.exe");
    await copyFile(
      path.join(repositoryRoot, baseline.tools.rust.resolvedExecutable),
      executable,
    );
    const tampered = await readFile(executable);
    assert.equal(tampered[0x50], 0x69, "The reviewed PE fixture DOS stub changed unexpectedly.");
    tampered[0x50] = 0x49;
    await writeFile(executable, tampered);

    const candidate = clone();
    candidate.tools.rust.resolvedExecutable = path.relative(repositoryRoot, executable)
      .replaceAll("\\", "/");
    await assert.rejects(
      preflightNativeTools(candidate),
      /CONTRACT_GENERATOR_VERSION_MISMATCH:.*cargo-typify.*digest/iu,
    );
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("native preflight rejects an environment-substituted Program Files root before probing it", async () => {
  const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), "sapphirus-fake-program-files-"));
  const fakeProgramFiles = path.join(temporaryRoot, "Program Files");
  const fakeDotnetRoot = path.join(fakeProgramFiles, "dotnet");
  const trustedDotnetRoot = path.join("C:\\", "Program Files", "dotnet");
  try {
    await mkdir(fakeDotnetRoot, { recursive: true });
    await copyFile(path.join(trustedDotnetRoot, "dotnet.exe"), path.join(fakeDotnetRoot, "dotnet.exe"));
    for (const entry of await readdir(trustedDotnetRoot, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        await symlink(
          path.join(trustedDotnetRoot, entry.name),
          path.join(fakeDotnetRoot, entry.name),
          "junction",
        );
      }
    }

    await withEnvironment("ProgramFiles", fakeProgramFiles, async () => {
      await assert.rejects(
        preflightNativeTools(clone()),
        /CONTRACT_LOCK_BOOTSTRAP_UNREVIEWED:.*Program Files.*substitut/iu,
      );
    });
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("native preflight rejects inherited .NET startup hooks before locating native generators", async () => {
  const candidate = clone();
  candidate.tools.rust.resolvedExecutable =
    "target/contract-tools/bin/definitely-missing-generator.exe";

  await withEnvironment(
    "DoTnEt_StArTuP_HoOkS",
    path.join(os.tmpdir(), "unsigned-startup-hook.dll"),
    async () => {
      await assert.rejects(
        preflightNativeTools(candidate),
        /CONTRACT_LOCK_BOOTSTRAP_UNREVIEWED:.*DOTNET_STARTUP_HOOKS.*inherited/iu,
      );
    },
  );
});

test("native injection gate rejects dangerous names case-insensitively and by prefix", async () => {
  for (const name of [
    "dotnet_additional_deps",
    "DoTnEt_ShArEd_StOrE",
    "DOTNET_ROOT_X64",
    "dotnet_roll_forward",
    "DOTNET_MODIFIABLE_ASSEMBLIES",
    "DoTnEt_EnAbLeDiAgNoStIcS",
    "CoreClr_Profiler_Path",
    "CoReHoSt_TrAcEfIlE",
    "comPlus_AltJit",
    "Cor_Profiler",
    "MsBuildSdKsPath",
    "DevPath",
  ]) {
    await withEnvironment(name, "attacker-controlled", async () => {
      assert.throws(
        () => assertNoInheritedNativeToolInjection(),
        new RegExp(`CONTRACT_LOCK_BOOTSTRAP_UNREVIEWED:.*${name}`, "iu"),
        name,
      );
    });
  }
});

test("native injection gate permits harmless CLI flags and the canonical setup-dotnet root", async () => {
  for (const [name, value] of [
    ["DOTNET_CLI_TELEMETRY_OPTOUT", "1"],
    ["dotnet_nologo", "1"],
    ["DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1"],
    ["DOTNET_CLI_HOME", path.join(os.tmpdir(), "ignored-dotnet-home")],
    ["DOTNET_ROOT", "C:\\Program Files\\dotnet"],
    ["dotnet_root_x64", "C:\\Program Files\\dotnet\\"],
  ]) {
    await withEnvironment(name, value, async () => {
      assert.doesNotThrow(() => assertNoInheritedNativeToolInjection(), name);
    });
  }
});

test("native child environments are locked and exclude inherited search and cache roots", async () => {
  const nativeCodegen = await import("../scripts/lib/native-codegen.mjs");
  assert.equal(
    typeof nativeCodegen.createLockedNativeChildEnvironment,
    "function",
    "native codegen must expose its reviewed child-environment policy",
  );

  const rustEnvironment = nativeCodegen.createLockedNativeChildEnvironment("rust");
  const dotnetEnvironment = nativeCodegen.createLockedNativeChildEnvironment("dotnet");
  const powershellEnvironment = nativeCodegen.createLockedNativeChildEnvironment("powershell");
  assert.deepEqual(rustEnvironment, {});
  assert.deepEqual(dotnetEnvironment, {
    DOTNET_EnableDiagnostics: "0",
    DOTNET_EnableDiagnostics_Debugger: "0",
    DOTNET_EnableDiagnostics_IPC: "0",
    DOTNET_EnableDiagnostics_Profiler: "0",
  });
  assert.deepEqual(powershellEnvironment, {
    PSModulePath: "C:\\__sapphirus_no_module_search__",
  });

  for (const environment of [rustEnvironment, dotnetEnvironment, powershellEnvironment]) {
    assert.equal(Object.isFrozen(environment), true);
    for (const name of [
      "PATH",
      "Path",
      "NUGET_PACKAGES",
      "CARGO_HOME",
      "MSBuildSDKsPath",
      "USERPROFILE",
    ]) {
      assert.equal(Object.hasOwn(environment, name), false, name);
    }
  }
});

test("genuine native preflight cannot load inherited PowerShell modules", {
  skip: process.platform !== "win32" ? "Windows-native qualification only" : false,
}, async () => {
  const temporaryRoot = await mkdtemp(path.join(os.tmpdir(), "sapphirus-hostile-psmodules-"));
  const marker = path.join(temporaryRoot, "forged-authenticode-marker.txt");
  const securityRoot = path.join(temporaryRoot, "Microsoft.PowerShell.Security");
  const utilityRoot = path.join(temporaryRoot, "Microsoft.PowerShell.Utility");
  try {
    await Promise.all([mkdir(securityRoot, { recursive: true }), mkdir(utilityRoot, { recursive: true })]);
    await writeFile(path.join(securityRoot, "Microsoft.PowerShell.Security.psd1"), [
      "@{",
      "  RootModule = 'Microsoft.PowerShell.Security.psm1'",
      "  ModuleVersion = '3.0.0.0'",
      "  FunctionsToExport = @('Get-AuthenticodeSignature')",
      "}",
      "",
    ].join("\n"));
    await writeFile(path.join(securityRoot, "Microsoft.PowerShell.Security.psm1"), [
      "function Get-AuthenticodeSignature {",
      `  Set-Content -LiteralPath '${marker.replaceAll("'", "''")}' -Value 'executed'`,
      "  [pscustomobject]@{",
      "    Status = 'Valid'",
      "    SignatureType = 'Authenticode'",
      `    SignerCertificate = [pscustomobject]@{ Subject = '${baseline.tools.dotnet.sdkAuthenticode.signerSubject.replaceAll("'", "''")}'; Thumbprint = '${baseline.tools.dotnet.sdkAuthenticode.signerThumbprint}' }`,
      "  }",
      "}",
      "Export-ModuleMember -Function Get-AuthenticodeSignature",
      "",
    ].join("\n"));
    await writeFile(path.join(utilityRoot, "Microsoft.PowerShell.Utility.psd1"), [
      "@{",
      "  RootModule = 'Microsoft.PowerShell.Utility.psm1'",
      "  ModuleVersion = '3.0.0.0'",
      "  FunctionsToExport = @('ConvertTo-Json')",
      "}",
      "",
    ].join("\n"));
    await writeFile(path.join(utilityRoot, "Microsoft.PowerShell.Utility.psm1"), [
      "function ConvertTo-Json {",
      "  [CmdletBinding()] param(",
      "    [Parameter(ValueFromPipeline = $true)] $InputObject,",
      "    [switch] $Compress",
      "  )",
      `  process { '${JSON.stringify(baseline.tools.dotnet.sdkAuthenticode).replaceAll("'", "''")}' }`,
      "}",
      "Export-ModuleMember -Function ConvertTo-Json",
      "",
    ].join("\n"));

    await withEnvironment("PSModulePath", temporaryRoot, async () => {
      await preflightNativeTools(clone());
    });
    await assert.rejects(readFile(marker, "utf8"), (error) => error?.code === "ENOENT");
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("native preflight rejects a tampered expanded Corvus closure before probing it", async () => {
  const temporaryRoot = await createContractCodegenTemp("tampered-corvus-");
  const trustedPackageRoot = path.join(
    process.env.USERPROFILE,
    ".nuget",
    "packages",
    "corvus.json.cli",
    "5.1.0",
  );
  const candidatePackageRoot = path.join(
    temporaryRoot,
    "corvus.json.cli",
    "5.1.0",
  );
  try {
    await mkdir(candidatePackageRoot, { recursive: true });
    for (const name of [
      "corvus.json.cli.5.1.0.nupkg",
      "corvus.json.cli.5.1.0.nupkg.sha512",
    ]) {
      await copyFile(path.join(trustedPackageRoot, name), path.join(candidatePackageRoot, name));
    }
    const candidateClosure = path.join(candidatePackageRoot, "tools", "net10.0", "any");
    await copyTree(
      path.join(trustedPackageRoot, "tools", "net10.0", "any"),
      candidateClosure,
    );
    const cliAssembly = path.join(candidateClosure, "Corvus.Json.Cli.dll");
    const originalStats = await lstat(cliAssembly);
    assert.equal(originalStats.isFile(), true);
    await rm(cliAssembly);
    await copyFile(
      path.join(trustedPackageRoot, "tools", "net10.0", "any", "Corvus.Json.Cli.dll"),
      cliAssembly,
    );
    const tampered = await readFile(cliAssembly);
    tampered[0] ^= 0xff;
    await writeFile(cliAssembly, tampered);

    await withEnvironment("NUGET_PACKAGES", temporaryRoot, async () => {
      await assert.rejects(
        preflightNativeTools(clone()),
        /CONTRACT_GENERATOR_VERSION_MISMATCH:.*Corvus.*closure/iu,
      );
    });
  } finally {
    await rm(temporaryRoot, { recursive: true, force: true });
  }
});

test("native codegen lock rejects source-set and output-path drift", () => {
  for (const mutate of [
    (lock) => { lock.sourceSet.production.roots.pop(); },
    (lock) => { lock.sourceSet.production.roots[0].id = "https://attacker.invalid/schema.json"; },
    (lock) => { lock.sourceSet.production.dependencies[0].file = "substitute.schema.json"; },
    (lock) => { lock.invocations.production.rustOutput = "../outside.rs"; },
    (lock) => { lock.invocations.qualification.dotnetNamespace = "Substituted"; },
  ]) {
    const candidate = clone();
    mutate(candidate);
    assert.throws(() => validateToolLock(candidate));
  }
});
