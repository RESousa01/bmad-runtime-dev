import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

const root = new URL("../", import.meta.url);

async function readJson(relativePath) {
  return JSON.parse(await readFile(new URL(relativePath, root), "utf8"));
}

test("the Windows bundle publishes Forge while retaining the legacy application identity", async () => {
  const config = await readJson("crates/desktop-app/tauri.conf.json");

  assert.equal(config.productName, "Forge");
  assert.equal(config.mainBinaryName, "forge");
  assert.equal(config.identifier, "com.sapphirus.desktop");
  assert.match(config.bundle.longDescription, /^Forge\b/);
  assert.doesNotMatch(config.bundle.longDescription, /Sapphirus/);
  assert.deepEqual(config.bundle.icon, [
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/128x128@2x.png",
    "icons/icon.icns",
    "icons/icon.ico",
  ]);
});

test("repository and release entry points publish the Forge product identity", async () => {
  const rootPackage = await readJson("package.json");
  const readme = await readFile(new URL("README.md", root), "utf8");
  const cargoManifest = await readFile(
    new URL("crates/desktop-app/Cargo.toml", root),
    "utf8",
  );
  const signedBuild = await readFile(
    new URL("tools/build-signed-windows-installer.ps1", root),
    "utf8",
  );
  const qualification = await readFile(
    new URL("tools/qualify-windows-installer.ps1", root),
    "utf8",
  );
  const workflow = await readFile(
    new URL(".github/workflows/release-windows-signed.yml", root),
    "utf8",
  );

  assert.equal(rootPackage.name, "forge-desktop");
  assert.match(readme, /^# Forge Desktop\b/u);
  assert.match(cargoManifest, /description = "Forge Windows desktop composition root"/u);
  assert.match(signedBuild, /target\/release\/forge\.exe/u);
  assert.match(qualification, /Get-Process -Name 'forge'/u);
  assert.match(qualification, /Join-Path \$installDirectory 'forge\.exe'/u);
  assert.match(workflow, /application_name: \$\{\{ steps\.release-metadata\.outputs\.application_name \}\}/u);
  assert.match(workflow, /target\/release\/\$\{\{ steps\.release-metadata\.outputs\.application_name \}\}/u);
  assert.doesNotMatch(workflow, /target\/release\/sapphirus\.exe/u);
});

test("active renderer document metadata uses the Forge name", async () => {
  const html = await readFile(new URL("apps/desktop-ui/index.html", root), "utf8");

  assert.match(html, /<title>Forge<\/title>/);
  assert.doesNotMatch(html, /Sapphirus/);
});

test("Tauri cuts over to the parallel Forge v2 renderer package", async () => {
  const config = await readJson("crates/desktop-app/tauri.conf.json");
  const rendererPackage = await readJson("apps/desktop-ui/package.json");

  assert.equal(rendererPackage.name, "@forge/desktop-ui");
  assert.equal(config.build.frontendDist, "../../apps/desktop-ui/dist");
  assert.equal(
    config.build.beforeDevCommand,
    "pnpm --filter @forge/desktop-ui dev --host 127.0.0.1",
  );
  assert.equal(config.build.beforeBuildCommand, "pnpm --filter @forge/desktop-ui build");
});

test("the production icon source is the Forge anvil aperture", async () => {
  const icon = await readFile(new URL("crates/desktop-app/icons/app-icon.svg", root), "utf8");
  const html = await readFile(new URL("apps/desktop-ui/index.html", root), "utf8");

  assert.match(icon, /Forge application mark/);
  assert.match(icon, /code aperture/);
  assert.doesNotMatch(icon, /Sapphirus|sapphire/i);
  assert.match(html, /href="\/forge-mark\.svg"/);
});

test("public renderer and native shell copy no longer exposes the legacy name", async () => {
  const publicSurfaces = [
    "apps/desktop-ui/src/App.tsx",
    "apps/desktop-ui/src/main.tsx",
    "apps/desktop-ui/src/components/HomeView.tsx",
    "apps/desktop-ui/src/components/TaskWorkspace.tsx",
    "apps/desktop-ui/src/components/WorkspacePanel.tsx",
    "apps/desktop-ui/src/components/settings/SettingsDialog.tsx",
    "apps/desktop-ui/src/lib/workspaceReadSource.ts",
    "crates/desktop-app/src/lib.rs",
    "crates/desktop-app/src/bmad_model/config.rs",
  ];

  for (const relativePath of publicSurfaces) {
    const source = await readFile(new URL(relativePath, root), "utf8");
    assert.doesNotMatch(source, /Sapphirus/, relativePath);
  }
});
