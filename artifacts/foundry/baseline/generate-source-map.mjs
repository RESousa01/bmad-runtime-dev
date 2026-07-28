import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = execFileSync(
  "git",
  ["rev-parse", "--show-toplevel"],
  { encoding: "utf8" },
).trim();
const commit = "7956da370b0f9f16c5aea3b21f6e39383fa3171d";
execFileSync("git", ["cat-file", "-e", `${commit}^{commit}`], {
  cwd: repositoryRoot,
  stdio: "ignore",
});
const tree = execFileSync("git", ["rev-parse", `${commit}^{tree}`], {
  cwd: repositoryRoot,
  encoding: "utf8",
}).trim();

const roots = [
  "apps/desktop-ui/src",
  "crates/desktop-app",
  "crates/desktop-cloud",
  "crates/desktop-egress",
  "infra/desktop-support",
  "packages/contracts",
  "services/desktop-support-api",
  "services/desktop-support-api.Tests",
  "tools",
];
const matchers = [
  /ModelEndpoint/i,
  /ModelDeployment/i,
  /AzureOpenAIClient/i,
  /model-access/i,
  /model_access/i,
  /bmad_model/i,
  /gpt-5-mini/i,
  /openai\.azure\.com/i,
  /cognitiveservices\.azure\.com/i,
  /direct-model-providers/i,
];
const pathMatchers = [
  /^apps\/desktop-ui\/src\/app\/model\//i,
  /^apps\/desktop-ui\/src\/features\/model-context\//i,
  /^apps\/desktop-ui\/src\/features\/timeline\/timelineModel(?:\.test)?\.ts$/i,
  /^apps\/desktop-ui\/src\/lib\/bmadModelProjection(?:\.test)?\.ts$/i,
  /^apps\/desktop-ui\/src\/components\/SessionContextPanel(?:\.test)?\.tsx$/i,
  /^apps\/desktop-ui\/src\/components\/composer\/AgentSelector(?:\.test)?\.tsx$/i,
  /^crates\/desktop-app\/src\/bmad_model(?:\.rs|\/)/i,
  /^crates\/desktop-cloud\//i,
  /^packages\/contracts\/(?:schemas|fixtures\/(?:valid|invalid))\/model-(?:access|context-consent)/i,
  /^packages\/contracts\/generated\/typescript\/schema\/model-(?:access|context-consent)/i,
  /^packages\/contracts\/generated\/dotnet\/SapphirusContractsCatalog\.(?:ModelAccess|ModelContextConsent)/i,
  /^services\/desktop-support-api\/Model\//i,
  /^services\/desktop-support-api\/(?:ModelAuthority\.cs|Sql\/SqlModelCallIdempotencyStore\.cs)$/i,
  /^services\/desktop-support-api\.Tests\/Model\//i,
  /^services\/desktop-support-api\.Tests\/(?:ModelAuthorityContractTests|ProductionCompositionTests)\.cs$/i,
];
const grepPattern = matchers.map((matcher) => matcher.source).join("|");
const explicitFiles = new Set([
  "apps/desktop-ui/src/App.tsx",
  "apps/desktop-ui/src/components/SessionContextPanel.tsx",
  "apps/desktop-ui/src/components/composer/AgentSelector.tsx",
  "apps/desktop-ui/src/lib/bmadModelProjection.ts",
  "crates/desktop-app/src/bmad_model/config.rs",
  "crates/desktop-cloud/src/production_round_trip.rs",
  "crates/desktop-cloud/src/support_contract.rs",
  "crates/desktop-cloud/src/transport.rs",
  "infra/desktop-support/main.bicep",
  "packages/contracts/fixtures/valid/model-access-receipt.json",
  "packages/contracts/fixtures/valid/model-access-request.json",
  "packages/contracts/fixtures/valid/model-access-result.json",
  "packages/contracts/schemas/model-access-receipt.schema.json",
  "packages/contracts/schemas/model-access-request.schema.json",
  "packages/contracts/schemas/model-access-result.schema.json",
  "services/desktop-support-api.Tests/Sapphirus.DesktopSupportApi.Tests.csproj",
  "services/desktop-support-api/Configuration/AzureClientRegistration.cs",
  "services/desktop-support-api/Configuration/ProductionComposition.cs",
  "services/desktop-support-api/Configuration/ProductionOptions.cs",
  "services/desktop-support-api/Model/AzureOpenAiModelAccessBroker.cs",
  "services/desktop-support-api/Model/ModelAccessCoordinator.cs",
  "services/desktop-support-api/Model/ModelAccessProfile.cs",
  "services/desktop-support-api/Program.cs",
  "services/desktop-support-api/Sapphirus.DesktopSupportApi.csproj",
  "services/desktop-support-api/Security.cs",
  "services/desktop-support-api/appsettings.json",
]);

const treeEntries = execFileSync(
  "git",
  ["ls-tree", "-r", "-z", commit, "--", ...roots],
  {
    cwd: repositoryRoot,
    encoding: null,
    maxBuffer: 16 * 1024 * 1024,
  },
)
  .toString("utf8")
  .split("\0")
  .filter(Boolean)
  .map((entry) => {
    const tab = entry.indexOf("\t");
    const metadata = entry.slice(0, tab).split(" ");
    return {
      gitBlob: metadata[2],
      path: entry.slice(tab + 1),
    };
  });

const matchedPaths = new Set(
  execFileSync(
    "git",
    [
      "grep",
      "-I",
      "-l",
      "-i",
      "-E",
      grepPattern,
      commit,
      "--",
      ...roots,
    ],
    {
      cwd: repositoryRoot,
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    },
  )
    .split(/\r?\n/u)
    .filter(Boolean)
    .map((line) => line.slice(`${commit}:`.length)),
);

const selectedEntries = treeEntries
  .filter(
    ({ path }) =>
      explicitFiles.has(path)
      || matchedPaths.has(path)
      || pathMatchers.some((matcher) => matcher.test(path)),
  )
  .sort((left, right) => left.path.localeCompare(right.path));

const files = selectedEntries.map(({ gitBlob, path }) => {
  const content = execFileSync("git", ["cat-file", "blob", gitBlob], {
    cwd: repositoryRoot,
    encoding: null,
    maxBuffer: 64 * 1024 * 1024,
  });
  return {
    path,
    bytes: content.byteLength,
    sha256: createHash("sha256").update(content).digest("hex"),
    gitBlob,
  };
});

const openCodeRoot = "C:\\tmp\\forge-v2\\library\\opencode";
const expectedOpenCodeCommit = "0a601cf334b9a83cc2854108a2b860f25e6e7e8e";
const openCodeCommit = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: openCodeRoot,
  encoding: "utf8",
}).trim();
if (openCodeCommit !== expectedOpenCodeCommit) {
  throw new Error(
    `OpenCode reference moved: expected ${expectedOpenCodeCommit}, received ${openCodeCommit}`,
  );
}
const openCodeStatus = execFileSync(
  "git",
  ["status", "--porcelain=v1", "--untracked-files=no"],
  { cwd: openCodeRoot, encoding: "utf8" },
).trim();
const openCodeExpected = new Map([
  [
    "packages/core/src/models-dev.ts",
    "236c8e3ecfaf33da7c1cf194316367cce3868a3c66c57e2cc7d62746f8457d50",
  ],
  [
    "packages/core/src/plugin/provider/azure.ts",
    "d3a184d99fe0ad63455518d600ca5b8333d59fc10e811c8a253451471f12fbb8",
  ],
  [
    "packages/opencode/src/provider/provider.ts",
    "1261e7365cad5508dcd8686801367e0f5c77afd5d64b8e9a2703262d8c3729b6",
  ],
  [
    "packages/opencode/src/server/routes/instance/httpapi/handlers/provider.ts",
    "3d4ed003519b82de193461a3f5600c174224d9bbd92609fd8688bb50d0afe761",
  ],
]);
const openCodeFiles = [...openCodeExpected.entries()].map(
  ([path, packageSourceMapSha256]) => {
    const content = readFileSync(join(openCodeRoot, ...path.split("/")));
    const sha256 = createHash("sha256").update(content).digest("hex");
    return {
      path,
      bytes: content.byteLength,
      sha256,
      packageSourceMapSha256,
      matchesPackageSourceMap: sha256 === packageSourceMapSha256,
    };
  },
);

const result = {
  schemaVersion: "forge.foundry-baseline-source-map.v1",
  generatedAt: new Date().toISOString(),
  hashAlgorithm: "sha256",
  repository: {
    root: repositoryRoot,
    branch: "main",
    commit,
    tree,
    fileCount: files.length,
    files,
  },
  discovery: {
    roots,
    contentPatterns: matchers.map((matcher) => matcher.source),
    pathPatterns: pathMatchers.map((matcher) => matcher.source),
    explicitFiles: [...explicitFiles].sort(),
    source: "Committed blobs from the frozen repository commit, not mutable working-tree bytes.",
  },
  openCodeReference: {
    root: openCodeRoot,
    commit: openCodeCommit,
    clean: openCodeStatus.length === 0,
    architectureReferenceOnly: true,
    files: openCodeFiles,
  },
  plan: {
    archive:
      "C:\\Users\\rodri\\Downloads\\forge-foundry-opencode-connection-plan.zip",
    archiveSha256:
      "5d024e69957ce01160e3f72ab31074b99ecd591a316a290dfaf9a64cc6c74bc1",
    implementationPlanSha256:
      "07ef06a719a5ecd9a4b044b00bd58d7ace43ea9d7407973c88456461391e5348",
  },
};

writeFileSync(
  join(scriptDirectory, "source-map.json"),
  `${JSON.stringify(result, null, 2)}\n`,
  "utf8",
);
console.log(
  `foundry baseline: wrote ${files.length} committed file hashes for ${commit}`,
);
