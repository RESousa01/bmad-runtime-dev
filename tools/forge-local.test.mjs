import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import test from "node:test";

const read = (path) => readFile(new URL(`../${path}`, import.meta.url), "utf8");

test("local compose runs the Forge UI and the existing support API without model credentials", async () => {
  const compose = await read("infra/local/compose.yaml");
  const packageManifest = JSON.parse(await read("package.json"));

  assert.match(
    compose,
    /127\.0\.0\.1:\$\{FORGE_FRONTEND_PORT:-1420\}:1420/u,
  );
  assert.match(
    compose,
    /image:\s+\$\{FORGE_FRONTEND_IMAGE:-forge-frontend:local\}/u,
  );
  assert.match(compose, /desktop-support-api:/u);
  assert.match(compose, /services\/desktop-support-api/u);
  assert.match(compose, /ASPNETCORE_ENVIRONMENT: Development/u);
  assert.match(compose, /FORGE_SUPPORT_PORT:-18080/u);
  assert.match(compose, /healthz\/ready/u);
  assert.doesNotMatch(compose, /healthz\/live/u);
  assert.match(compose, /image:\s+\$\{DESKTOP_SUPPORT_IMAGE:-[^}]+\}/u);
  assert.match(compose, /forge-local:/u);
  assert.doesNotMatch(compose, /OPENAI_API_KEY|AZURE_OPENAI_API_KEY/u);
  assert.match(compose, /healthcheck:/u);
  assert.equal(
    packageManifest.scripts["local:contracts:test"],
    "node --test tools/forge-local.test.mjs",
  );
  assert.match(
    packageManifest.scripts["verify:source"],
    /pnpm local:contracts:test/u,
  );
});

test("local frontend image uses the pinned workspace toolchain", async () => {
  const dockerfile = await read("infra/local/frontend.Dockerfile");

  assert.match(
    dockerfile,
    /FROM node:24\.18\.0-alpine@sha256:a0b9bf06e4e6193cf7a0f58816cc935ff8c2a908f81e6f1a95432d679c54fbfd/u,
  );
  assert.match(dockerfile, /pnpm install --frozen-lockfile/u);
  assert.match(dockerfile, /@forge\/desktop-ui/u);
  assert.match(dockerfile, /--host[", ]+0\.0\.0\.0/u);
  assert.match(dockerfile, /@forge\/desktop-ui build/u);
  assert.match(dockerfile, /vite", "preview/u);
  assert.match(dockerfile, /COPY[^\n]+tsconfig\.base\.json/u);
  assert.ok(
    dockerfile.indexOf("pnpm install --frozen-lockfile")
      > dockerfile.indexOf("COPY apps/desktop-ui apps/desktop-ui"),
    "workspace install must run after source copies so pnpm links survive Docker COPY",
  );
});

test("support image locked restore consumes the committed NuGet lock", async () => {
  const dockerfile = await read("services/desktop-support-api/Dockerfile");
  const lockCopy = dockerfile.search(/COPY[^\n]*packages\.lock\.json/u);
  const lockedRestore = dockerfile.search(/RUN dotnet restore --locked-mode/u);

  assert.ok(lockCopy >= 0, "support image must copy packages.lock.json");
  assert.ok(lockedRestore >= 0, "support image must use locked restore");
  assert.ok(
    lockCopy < lockedRestore,
    "packages.lock.json must be present before dotnet restore --locked-mode",
  );
});

test("Azure template passes the reviewed desktop client ID into the API container", async () => {
  const bicep = await read("infra/desktop-support/main.bicep");
  const compiled = JSON.parse(await read("infra/desktop-support/main.json"));
  const compiledSource = JSON.stringify(compiled);

  assert.match(bicep, /param approvedDesktopClientId string/u);
  assert.match(
    bicep,
    /\{\s*name:\s*'Sapphirus__ApprovedDesktopClientId',\s*value:\s*approvedDesktopClientId\s*\}/u,
  );
  assert.ok(
    compiled.parameters?.approvedDesktopClientId,
    "compiled template must expose approvedDesktopClientId",
  );
  assert.match(compiledSource, /Sapphirus__ApprovedDesktopClientId/u);
  assert.match(compiledSource, /parameters\('approvedDesktopClientId'\)/u);
});

test("both Azure parameter examples and deployment guidance require the approved desktop client", async () => {
  const currentExample = await read("infra/desktop-support/main.example.bicepparam");
  const legacyExample = await read("infra/desktop-support/main.bicepparam.example");
  const guide = await read("docs/guides/azure-support-plane-deployment-guide.md");

  for (const example of [currentExample, legacyExample]) {
    assert.match(example, /param approvedDesktopClientId\s*=/u);
    assert.doesNotMatch(
      example,
      /approvedDesktopClientId\s*=\s*'00000000-0000-0000-0000-000000000000'/u,
      "the API rejects Guid.Empty, even though it has the right textual shape",
    );
  }
  assert.match(guide, /approvedDesktopClientId/u);
  assert.match(
    guide,
    /desktop\s+public-client\s+app registration|desktop\s+client\s+app registration/iu,
  );
});

test("both Development support harnesses use readiness, loopback ports, init, and injectable images", async () => {
  const forgeCompose = await read("infra/local/compose.yaml");
  const supportCompose = await read("infra/desktop-support/local/docker-compose.yml");
  const supportDevelopmentService = supportCompose.split(
    "\n  production-configuration-contract:",
  )[0];

  for (const compose of [forgeCompose, supportDevelopmentService]) {
    assert.match(compose, /image:\s+\$\{DESKTOP_SUPPORT_IMAGE:-[^}]+\}/u);
    assert.match(compose, /\n\s+init:\s+true\s*$/mu);
    assert.match(
      compose,
      /127\.0\.0\.1:\$\{FORGE_SUPPORT_PORT:-18080\}:8080/u,
    );
    assert.match(compose, /healthz\/ready/u);
    assert.doesNotMatch(compose, /healthz\/live/u);
  }
});

test("optional Production configuration profile proves boot shape without claiming Azure emulation", async () => {
  const compose = await read("infra/desktop-support/local/docker-compose.yml");
  const documentation = await read("infra/desktop-support/local/README.md");
  const productionService = compose.split(
    "\n  production-configuration-contract:",
  )[1] ?? "";

  assert.doesNotMatch(compose, /Local Docker emulation/iu);
  assert.doesNotMatch(documentation, /Local Docker emulation/iu);
  assert.match(documentation, /local runtime\/configuration rehearsal/iu);
  assert.match(compose, /production-configuration-contract:/u);
  assert.match(compose, /profiles:\s*\[[^\]]*"production-configuration-contract"[^\]]*\]/u);
  assert.match(compose, /ASPNETCORE_ENVIRONMENT:\s+Production/u);
  assert.match(compose, /healthz\/live/u);
  assert.match(productionService, /network_mode:\s+none/u);
  assert.doesNotMatch(productionService, /\n\s+ports:/u);
  assert.doesNotMatch(productionService, /APPLICATIONINSIGHTS_CONNECTION_STRING/u);
  for (const key of [
    "ApprovedDesktopClientId",
    "ManagedIdentityClientId",
    "AppConfigurationEndpoint",
    "KeyVaultUri",
    "ReceiptSigningKeyName",
    "SqlServer",
    "SqlDatabase",
    "ModelEndpoint",
    "ModelDeployment",
    "ProviderProfileHash",
    "ModelProfileHash",
    "ModelCapabilityHash",
    "DeploymentHash",
  ]) {
    assert.match(compose, new RegExp(`Sapphirus__${key}:`, "u"));
  }
  assert.match(documentation, /boot\/configuration fidelity/iu);
  assert.match(documentation, /does not emulate|does not call|not Azure service emulation/iu);
});

test("native launcher can start offline and leaves model selection to the UI", async () => {
  const launcher = await read("tools/start-forge-local.ps1");
  const tauriLocalConfig = await read("infra/local/tauri.conf.json");
  const provider = await read("crates/desktop-cloud/src/direct_provider.rs");
  const modelConfig = await read("crates/desktop-app/src/bmad_model/config.rs");
  const coordinator = await read("crates/desktop-app/src/bmad_model/coordinator.rs");

  assert.doesNotMatch(launcher, /OPENAI_API_KEY|AZURE_OPENAI_API_KEY/u);
  assert.match(launcher, /docker compose/u);
  assert.match(launcher, /pnpm exec tauri dev/u);
  assert.doesNotMatch(launcher, /cargo run/u);
  assert.match(launcher, /infra\\local\\tauri\.conf\.json/u);
  assert.match(launcher, /Settings > Providers/u);
  assert.doesNotMatch(launcher, /FORGE_MODEL_PROVIDER|OPENAI_MODEL/u);
  assert.doesNotMatch(launcher, /SetEnvironmentVariable\([^\n]+(?:User|Machine)/u);
  assert.doesNotMatch(launcher, /\.env/u);
  assert.match(tauriLocalConfig, /"beforeDevCommand": null/u);
  assert.match(tauriLocalConfig, /http:\/\/127\.0\.0\.1:1420/u);
  assert.match(provider, /from_selection/u);
  for (const source of [provider, modelConfig, coordinator]) {
    assert.doesNotMatch(
      source,
      /"(?:FORGE_MODEL_PROVIDER|OPENAI_MODEL|AZURE_OPENAI_MODEL|AZURE_OPENAI_DEPLOYMENT)"/u,
    );
  }
});

test("native launcher closes only known Forge hosts before starting current source", async () => {
  const launcher = await read("tools/start-forge-local.ps1");

  assert.match(launcher, /LOCALAPPDATA/u);
  assert.match(launcher, /Forge[\\/]forge\.exe/u);
  assert.match(launcher, /target[\\/]debug[\\/]desktop-app\.exe/u);
  assert.match(launcher, /Stop-Process/u);
  assert.ok(
    launcher.indexOf("Stop-Process") < launcher.indexOf("pnpm exec tauri dev"),
    "known single-instance hosts must stop before the current source host starts",
  );
  assert.doesNotMatch(launcher, /Get-Process\s+-Name\s+forge[^\n]*\|\s*Stop-Process/u);
});

test(
  "native launcher is valid PowerShell",
  { skip: process.platform !== "win32" },
  () => {
    const launcherPath = fileURLToPath(
      new URL("../tools/start-forge-local.ps1", import.meta.url),
    );
    const script = [
      "$tokens = $null",
      "$errors = $null",
      `[void][System.Management.Automation.Language.Parser]::ParseFile('${launcherPath.replaceAll("'", "''")}', [ref]$tokens, [ref]$errors)`,
      "if ($errors.Count -gt 0) { $errors | ForEach-Object { Write-Error $_.Message }; exit 1 }",
    ].join("; ");
    const parsed = spawnSync(
      "powershell.exe",
      ["-NoProfile", "-Command", script],
      { encoding: "utf8" },
    );

    assert.equal(parsed.status, 0, parsed.stderr || parsed.stdout);
  },
);
