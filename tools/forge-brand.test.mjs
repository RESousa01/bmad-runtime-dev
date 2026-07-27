import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import { test } from "node:test";

const root = new URL("../", import.meta.url);

async function readJson(relativePath) {
  return JSON.parse(await readFile(new URL(relativePath, root), "utf8"));
}

async function isRegularFile(relativePath) {
  try {
    return (await stat(new URL(relativePath, root))).isFile();
  } catch {
    return false;
  }
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

test("the installer dresses itself from real, in-repository Forge assets", async () => {
  const config = await readJson("crates/desktop-app/tauri.conf.json");
  const nsis = config.bundle.windows.nsis;

  // Every path NSIS is pointed at is resolved relative to this config's own
  // directory. A typo here is not a build warning: `tauri build` fails late,
  // on a release machine, after the whole Rust workspace has compiled.
  const assets = {
    "bundle.licenseFile": config.bundle.licenseFile,
    "nsis.installerIcon": nsis.installerIcon,
    "nsis.uninstallerIcon": nsis.uninstallerIcon,
    "nsis.headerImage": nsis.headerImage,
    // Set explicitly rather than left to Tauri's documented fallback: if that
    // fallback ever stops applying, the uninstaller silently reverts to the
    // stock NSIS graphic, which is the one surface nobody looks at twice.
    "nsis.uninstallerHeaderImage": nsis.uninstallerHeaderImage,
    "nsis.sidebarImage": nsis.sidebarImage,
    "nsis.installerHooks": nsis.installerHooks,
    "nsis.customLanguageFiles.English": nsis.customLanguageFiles?.English,
  };
  for (const [field, value] of Object.entries(assets)) {
    assert.equal(typeof value, "string", `${field} must name an installer asset`);
    assert.ok(
      await isRegularFile(`crates/desktop-app/${value}`),
      `${field} points at a missing file: ${value}`,
    );
  }

  // The elevation-free, offline-capable shape the internal build depends on.
  assert.equal(nsis.installMode, "currentUser");
  assert.equal(config.bundle.windows.webviewInstallMode.type, "offlineInstaller");
  assert.deepEqual(nsis.languages, ["English"]);

  // Add/Remove Programs and the file properties dialog read these.
  assert.equal(config.bundle.publisher, "Sapphirus Engineering");
  assert.match(config.bundle.copyright, /^Copyright © \d{4} Sapphirus Engineering$/u);
  assert.doesNotMatch(config.bundle.shortDescription, /Sapphirus|Read-only/u);

  // The header and sidebar must be 24-bit uncompressed BI_RGB at exactly the
  // sizes MUI2 lays out for, or Windows scales or mangles them.
  for (const [asset, width, height] of [
    ["header.bmp", 150, 57],
    ["sidebar.bmp", 164, 314],
  ]) {
    const bitmap = await readFile(
      new URL(`crates/desktop-app/installer/${asset}`, root),
    );
    assert.equal(bitmap.toString("ascii", 0, 2), "BM", asset);
    assert.equal(bitmap.readInt32LE(18), width, `${asset} width`);
    assert.equal(bitmap.readInt32LE(22), height, `${asset} height`);
    assert.equal(bitmap.readUInt16LE(28), 24, `${asset} bit depth`);
    assert.equal(bitmap.readUInt32LE(30), 0, `${asset} compression`);
  }
});

test("the custom NSIS message file covers every message Tauri's template needs", async () => {
  // Supplying `customLanguageFiles.English` replaces Tauri's own English.nsh
  // outright — there is no partial override — so a message Tauri references
  // and this file omits is an undefined-language-string build failure. This
  // is the exact set shipped by @tauri-apps/cli 2.11.4; a CLI upgrade that
  // adds one must fail here, in the source lane, not in a release build.
  const tauriMessages = [
    "addOrReinstall",
    "alreadyInstalled",
    "alreadyInstalledLong",
    "appRunning",
    "appRunningOkKill",
    "chooseMaintenanceOption",
    "choowHowToInstall",
    "createDesktop",
    "deleteAppData",
    "dontUninstall",
    "dontUninstallDowngrade",
    "failedToKillApp",
    "installingWebview2",
    "newerVersionInstalled",
    "older",
    "olderOrUnknownVersionInstalled",
    "silentDowngrades",
    "unableToUninstall",
    "uninstallApp",
    "uninstallBeforeInstalling",
    "unknown",
    "webview2AbortError",
    "webview2DownloadError",
    "webview2DownloadSuccess",
    "webview2Downloading",
    "webview2InstallError",
    "webview2InstallSuccess",
  ];
  const source = await readFile(
    new URL("crates/desktop-app/installer/English.nsh", root),
    "utf8",
  );
  const defined = [...source.matchAll(/^LangString (\w+) /gmu)].map((match) => match[1]);

  for (const message of tauriMessages) {
    assert.ok(defined.includes(message), `English.nsh does not define ${message}`);
  }
  assert.equal(new Set(defined).size, defined.length, "duplicate LangString");

  // Tauri's own file mixes NSIS defines with handlebars placeholders. Only the
  // NSIS define is correct here regardless of how the bundler treats the file.
  assert.doesNotMatch(source, /\{\{/u);
  // A byte-order-mark-less include is read as ANSI by the Unicode script.
  assert.ok(/^[\t\r\n\x20-\x7e]*$/u.test(source), "English.nsh must stay ASCII-only");
});

test("uninstalling keeps local data unless the user explicitly asks otherwise", async () => {
  const messages = await readFile(
    new URL("crates/desktop-app/installer/English.nsh", root),
    "utf8",
  );
  const hooks = await readFile(
    new URL("crates/desktop-app/installer/hooks.nsh", root),
    "utf8",
  );
  const settings = await readFile(
    new URL("apps/desktop-ui/src/components/settings/SettingsDialog.tsx", root),
    "utf8",
  );

  // The stock checkbox says "Delete the application data", which does not tell
  // the user that their transcripts, approvals and workspace grants are what
  // goes. The label has to name the act and the stakes.
  const label = /^LangString deleteAppData \$\{LANG_ENGLISH\} "(.+)"\s*$/mu.exec(messages);
  assert.ok(label, "deleteAppData must be defined");
  assert.match(label[1], /erase/iu);
  assert.match(label[1], /cannot be undone/iu);

  // Destroying the local store in-app costs an exact typed phrase. The
  // uninstaller must not be a cheaper route to the same irreversible act.
  assert.match(hooks, /!macro NSIS_HOOK_PREUNINSTALL/u);
  assert.match(hooks, /\$DeleteAppDataCheckboxState/u);
  assert.match(hooks, /MessageBox[^\n]*\$\(forgeEraseLocalDataConfirm\)/u);
  assert.match(hooks, /StrCpy \$DeleteAppDataCheckboxState 0/u);
  // Release qualification uninstalls with /S and fails on a blocked process,
  // so the prompt must be unreachable in a silent uninstall.
  assert.match(hooks, /\$\{AndIfNot\} \$\{Silent\}/u);
  assert.match(messages, /^LangString forgeEraseLocalDataConfirm /mu);

  // And the app must not promise something the uninstaller does not honour.
  assert.doesNotMatch(settings, /Uninstalling never deletes/u);
});
