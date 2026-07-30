import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import test from "node:test";

const root = process.cwd();

async function source(...parts) {
  return readFile(join(root, ...parts), "utf8");
}

test("reviewed recovery commands stay separated and closed at every executable boundary", async () => {
  const [runtime, ipc, host, guard] = await Promise.all([
    source("crates", "desktop-runtime", "src", "command.rs"),
    source("crates", "desktop-ipc", "src", "envelope.rs"),
    source("crates", "desktop-app", "src", "commands.rs"),
    source("tools", "check-boundaries.mjs"),
  ]);

  for (const command of ["changes.recovery.prepare", "changes.recovery.decide"]) {
    assert.match(runtime, new RegExp(`=> "${command.replaceAll(".", "\\.")}"`));
    assert.match(ipc, new RegExp(`"${command.replaceAll(".", "\\.")}"`));
    assert.match(host, new RegExp(`"${command.replaceAll(".", "\\.")}"`));
    assert.match(guard, new RegExp(`"${command.replaceAll(".", "\\.")}"`));
  }
  assert.match(host, /LocalCommand::PrepareChangesRecovery[\s\S]*LocalCommand::DecideChangesRecovery/u);
  assert.match(host, /crate::recovery::prepare_recovery\([\s\S]*renderer_session/u);
  assert.match(host, /crate::recovery::decide_recovery\([\s\S]*renderer_session/u);
  assert.doesNotMatch(ipc, /struct (?:Prepare|Decide)ChangesRecoveryPayload[\s\S]{0,500}(?:absolute_path|shell|checkpoint_content|provider)/u);
});

test("reviewed catalog and update blocking remain executable guard invariants", async () => {
  const guard = await source("tools", "check-boundaries.mjs");
  assert.match(guard, /reviewedReadyCommands[\s\S]*"changes\.history",\s*"changes\.recovery\.prepare",\s*"changes\.recovery\.decide"/u);
  assert.match(guard, /recoveryExcludedFromReplyCache/u);
  assert.match(guard, /recovery_required[\s\S]*restoring[\s\S]*manual_review[\s\S]*update/u);
});

test("the renderer boundary scan includes the active Forge source", async () => {
  const guard = await source("tools", "check-boundaries.mjs");
  assert.match(
    guard,
    /rendererRoots[\s\S]*join\(root, "apps", "desktop-ui", "src"\)/u,
  );
});

const immutableActionReference = /^[^@\s]+@[0-9a-f]{40}$/u;

test("the immutable-reference pattern rejects every mutable action form", () => {
  for (const mutable of [
    "actions/checkout@v4",
    "anchore/scan-action@v6",
    "dtolnay/rust-toolchain@master",
    "actions/setup-node@main",
    "pnpm/action-setup@release/v4",
    "actions/checkout@34e11487",
    "actions/checkout@34E114876B0B11C390A56381AD16EBD13914F8D5",
  ]) {
    assert.equal(immutableActionReference.test(mutable), false, mutable);
  }
  assert.equal(
    immutableActionReference.test(
      "actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5",
    ),
    true,
  );
});

test("every workflow action reference is an immutable reviewed commit", async () => {
  const files = (await readdir(join(root, ".github", "workflows"))).filter((name) =>
    name.endsWith(".yml"),
  );
  assert.ok(files.length >= 7, "expected the full workflow set");
  const mutable = [];
  for (const file of files) {
    const text = await source(".github", "workflows", file);
    for (const match of text.matchAll(/^\s*(?:- )?uses:\s*([^\s#]+)/gmu)) {
      if (!immutableActionReference.test(match[1])) {
        mutable.push(`${file}: ${match[1]}`);
      }
    }
  }
  assert.deepEqual(mutable, []);
  const guard = await source("tools", "check-boundaries.mjs");
  assert.match(guard, /action reference must use a reviewed full commit SHA/u);
  assert.match(guard, /workflowFiles/u);
});

test("no release artifact compiles the development provider egress path", async () => {
  /*
   * Audit finding, phase 0-2 review: `check-boundaries` required each desktop
   * script to *name* a trust mode but accepted any of the three, so
   * `desktop:build` could be switched to `direct-model-providers` and still
   * pass — which is the state F-09 describes, arriving under a different name.
   *
   * Scoped to `tauri build`: `tauri dev` runs that path deliberately and
   * leaves no artifact. Among build scripts only a `:direct` one may.
   */
  const [manifest, guard] = await Promise.all([
    source("package.json"),
    source("tools", "check-boundaries.mjs"),
  ]);
  const scripts = JSON.parse(manifest).scripts ?? {};

  for (const [name, command] of Object.entries(scripts)) {
    if (typeof command !== "string") continue;
    if (!/\btauri\s+build\b/u.test(command)) continue;
    if (name.endsWith(":direct")) continue;
    assert.doesNotMatch(
      command,
      /--features direct-model-providers/u,
      `${name} builds an artifact with the development provider egress path`,
    );
  }

  // And the rule is enforced, not merely currently satisfied today.
  assert.ok(
    guard.includes("const producesArtifact"),
    "check-boundaries must scope the rule to scripts that produce an artifact",
  );
  assert.ok(
    guard.includes("builds an artifact with direct-model-providers"),
    "check-boundaries must report a release build that compiles the direct path",
  );
});

test("installer qualification covers every bundled BMAD foundation directory", async () => {
  const [configSource, qualification] = await Promise.all([
    source("crates", "desktop-app", "tauri.conf.json"),
    source("tools", "qualify-windows-installer.ps1"),
  ]);
  const config = JSON.parse(configSource);
  const bundledDirectories = Object.keys(config.bundle.resources)
    .filter((path) => path.startsWith("../../packages/bmad-foundation/"))
    .map((path) => path.split("/").at(-1))
    .filter((name) => name !== undefined && !name.includes("."));
  const bundledFiles = Object.keys(config.bundle.resources)
    .filter((path) => path.startsWith("../../packages/bmad-foundation/"))
    .map((path) => path.split("/").at(-1))
    .filter((name) => name?.includes("."));
  const fileList = qualification.match(
    /foreach \(\$file in @\((?<files>[^)]*)\)\)/u,
  );
  const directoryList = qualification.match(
    /foreach \(\$directory in @\((?<directories>[^)]*)\)\)/u,
  );
  assert.ok(fileList?.groups, "qualification file list must exist");
  assert.ok(directoryList?.groups, "qualification directory list must exist");
  const qualifiedFiles = [
    ...fileList.groups.files.matchAll(/'(?<name>[^']+)'/gu),
  ].map((match) => match.groups.name);
  const qualifiedDirectories = [
    ...directoryList.groups.directories.matchAll(/'(?<name>[^']+)'/gu),
  ].map((match) => match.groups.name);

  assert.deepEqual(
    [...qualifiedFiles].sort(),
    [...bundledFiles].sort(),
    "qualification must hash the same foundation files that Tauri bundles",
  );
  assert.deepEqual(
    [...qualifiedDirectories].sort(),
    [...bundledDirectories].sort(),
    "qualification must hash the same foundation directories that Tauri bundles",
  );
});

test("desktop startup accepts the complete managed foundation manifest inventory", async () => {
  const [runtimeManifestSource, loaderSource] = await Promise.all([
    source("packages", "bmad-foundation", "runtime-manifest.json"),
    source("crates", "desktop-app", "src", "bmad_foundation.rs"),
  ]);
  const runtimeManifest = JSON.parse(runtimeManifestSource);
  const allowlist = loaderSource.match(
    /const EXPECTED_RESOURCE_PATHS:\s*\[&str;\s*\d+\]\s*=\s*\[(?<entries>[\s\S]*?)\];/u,
  );
  assert.ok(allowlist?.groups, "native foundation allowlist must exist");

  const acceptedManagedPaths = [
    ...allowlist.groups.entries.matchAll(/"(?<path>[^"]+)"/gu),
  ].map((match) => match.groups.path);
  const manifestedManagedPaths = runtimeManifest.resources
    .map((resource) => resource.path)
    .filter((path) => !path.startsWith("source/method/6.10.0/"));

  assert.deepEqual(
    acceptedManagedPaths,
    manifestedManagedPaths,
    "a packaged managed resource must not make the native app reject its own manifest at startup",
  );

  const kindPolicy = loaderSource.match(
    /fn content_kind_matches_path\([\s\S]*?\n\}/u,
  );
  assert.ok(kindPolicy, "native foundation content-kind policy must exist");
  for (const resource of runtimeManifest.resources.filter(
    ({ path }) =>
      !path.includes("/") &&
      !["NOTICE.md", "adoption-ledger.json", "semantic-source-ledger.json"].includes(path),
  )) {
    assert.ok(
      kindPolicy[0].includes(`"${resource.path}"`),
      `${resource.path} must be explicitly admitted by the native content-kind policy`,
    );
  }
});
