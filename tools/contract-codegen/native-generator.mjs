#!/usr/bin/env node
// Manage the reviewed, prebuilt native contract generator (cargo-typify).
//
// The native codegen preflight pins the exact byte identity of cargo-typify.exe
// and its cargo install metadata (target/contract-tools/.crates2.json). Building
// cargo-typify from source is not byte-reproducible across machines (Rust embeds
// absolute build paths), so CI restores a reviewed prebuilt binary from vendor/
// instead of `cargo install`. This script keeps that vendored artifact in
// lock-step with tools/contract-codegen/tool-lock.json.
//
// The preflight also checks for the cargo-typify source archive (.crate file) in
// CARGO_HOME/registry/cache, which cargo would normally place there during
// `cargo install`. The restore command populates this path so the preflight
// check passes even when cargo install is skipped.
//
// Usage:
//   node tools/contract-codegen/native-generator.mjs verify    # check vendor vs lock (default)
//   node tools/contract-codegen/native-generator.mjs restore   # place vendor into target/contract-tools
//   node tools/contract-codegen/native-generator.mjs identity <exe>  # print pinnable identity
import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { normalizedPortableExecutableSha256 } from "../../packages/contracts/scripts/lib/native-codegen.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(here, "..", "..");
const vendorRoot = path.join(here, "vendor");
const toolLockPath = path.join(here, "tool-lock.json");
const vendorExeName = "cargo-typify-0.6.1-x86_64-pc-windows-msvc.exe";
const vendorMetaName = "crates2.json";
const vendorCrateName = "cargo-typify-0.6.1.crate";
// Canonical sparse-registry cache directory name for crates.io (Rust 1.65+).
// findCargoArchive in native-codegen.mjs scans all subdirectories, so the name
// just needs to be consistent; using the standard name avoids ambiguity if
// cargo later populates the same directory.
const cargoRegistryDir = "index.crates.io-6f17d22bba15001f";

function fail(message) {
  process.stderr.write(`native-generator: ${message}\n`);
  process.exit(1);
}

function rawSha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

async function readChecksum(checksumPath) {
  // sha256sum format: "<hex>  <filename>"; we only trust the digest field.
  const text = await readFile(checksumPath, "utf8");
  const digest = text.trim().split(/\s+/u)[0]?.toLowerCase() ?? "";
  if (!/^[0-9a-f]{64}$/u.test(digest)) {
    fail(`checksum file ${path.relative(repositoryRoot, checksumPath)} is not a sha256 digest.`);
  }
  return digest;
}

async function loadRustLock() {
  const lock = JSON.parse(await readFile(toolLockPath, "utf8"));
  const rust = lock?.tools?.rust;
  if (!rust?.executableIdentity || !rust?.installMetadata) {
    fail("tool-lock.json is missing the reviewed rust generator identity.");
  }
  return rust;
}

// Every reviewed source of truth the vendored artifact must satisfy.
async function assertVendorMatchesLock(rust) {
  const vendorExe = path.join(vendorRoot, vendorExeName);
  const vendorMeta = path.join(vendorRoot, vendorMetaName);
  const vendorCrate = path.join(vendorRoot, vendorCrateName);
  const exeBytes = await readFile(vendorExe);
  const metaBytes = await readFile(vendorMeta);
  const crateBytes = await readFile(vendorCrate);

  const exeRaw = rawSha256(exeBytes);
  const exeRawExpected = await readChecksum(`${vendorExe}.sha256`);
  if (exeRaw !== exeRawExpected) {
    fail(`vendored ${vendorExeName} raw sha256 ${exeRaw} != checksum file ${exeRawExpected}.`);
  }
  if (exeBytes.length !== rust.executableIdentity.fileSize) {
    fail(`vendored ${vendorExeName} size ${exeBytes.length} != lock ${rust.executableIdentity.fileSize}.`);
  }
  const exeNormalized = normalizedPortableExecutableSha256(exeBytes).replace(/^sha256:/u, "");
  if (exeNormalized !== rust.executableIdentity.normalizedSha256) {
    fail(`vendored ${vendorExeName} normalized sha256 ${exeNormalized} != lock ${rust.executableIdentity.normalizedSha256}.`);
  }

  const metaRaw = rawSha256(metaBytes);
  const metaRawExpected = await readChecksum(`${vendorMeta}.sha256`);
  if (metaRaw !== metaRawExpected) {
    fail(`vendored ${vendorMetaName} raw sha256 ${metaRaw} != checksum file ${metaRawExpected}.`);
  }
  if (metaRaw !== rust.installMetadata.sha256) {
    fail(`vendored ${vendorMetaName} raw sha256 ${metaRaw} != lock ${rust.installMetadata.sha256}.`);
  }

  const crateRaw = rawSha256(crateBytes);
  const crateRawExpected = await readChecksum(`${vendorCrate}.sha256`);
  if (crateRaw !== crateRawExpected) {
    fail(`vendored ${vendorCrateName} raw sha256 ${crateRaw} != checksum file ${crateRawExpected}.`);
  }
  if (crateRaw !== rust.packageSha256) {
    fail(`vendored ${vendorCrateName} raw sha256 ${crateRaw} != lock ${rust.packageSha256}.`);
  }
  return { exeBytes, metaBytes, crateBytes };
}

async function verify() {
  const rust = await loadRustLock();
  await assertVendorMatchesLock(rust);
  process.stdout.write(
    `native-generator: vendored cargo-typify matches the reviewed lock (size ${rust.executableIdentity.fileSize}, normalized ${rust.executableIdentity.normalizedSha256}).\n`,
  );
}

async function restore() {
  const rust = await loadRustLock();
  await assertVendorMatchesLock(rust);
  const targetExe = path.join(repositoryRoot, rust.resolvedExecutable);
  const targetMeta = path.join(repositoryRoot, rust.installMetadata.path);
  await mkdir(path.dirname(targetExe), { recursive: true });
  await mkdir(path.dirname(targetMeta), { recursive: true });
  await copyFile(path.join(vendorRoot, vendorExeName), targetExe);
  await copyFile(path.join(vendorRoot, vendorMetaName), targetMeta);

  // The native preflight (preflightNativeTools) also checks for the source
  // archive in CARGO_HOME/registry/cache/<registry>/. Populate this path so the
  // check passes when cargo install was skipped in favour of a prebuilt binary.
  const userProfile = process.env.USERPROFILE ?? process.env.HOME ?? "";
  const cargoHome = process.env.CARGO_HOME
    ?? (userProfile !== "" ? path.join(userProfile, ".cargo") : "");
  if (cargoHome === "" || !path.isAbsolute(cargoHome)) {
    fail("CARGO_HOME or USERPROFILE/HOME must resolve to an absolute path to restore the cargo registry cache.");
  }
  const registryCacheDir = path.join(cargoHome, "registry", "cache", cargoRegistryDir);
  await mkdir(registryCacheDir, { recursive: true });
  await copyFile(path.join(vendorRoot, vendorCrateName), path.join(registryCacheDir, vendorCrateName));

  process.stdout.write(
    `native-generator: restored reviewed cargo-typify to ${rust.resolvedExecutable}, ${rust.installMetadata.path}, and CARGO_HOME/registry/cache/${cargoRegistryDir}/${vendorCrateName}.\n`,
  );
}

async function identity(exePath) {
  if (!exePath) fail("identity requires a path to a cargo-typify executable.");
  const resolved = path.resolve(repositoryRoot, exePath);
  const info = await stat(resolved);
  if (!info.isFile()) fail(`${exePath} is not a regular file.`);
  const bytes = await readFile(resolved);
  const normalized = normalizedPortableExecutableSha256(bytes).replace(/^sha256:/u, "");
  process.stdout.write(`${JSON.stringify({
    fileSize: bytes.length,
    normalizedSha256: normalized,
    rawSha256: rawSha256(bytes),
  }, null, 2)}\n`);
}

const [command = "verify", argument] = process.argv.slice(2);
switch (command) {
  case "verify": await verify(); break;
  case "restore": await restore(); break;
  case "identity": await identity(argument); break;
  default: fail(`unknown command '${command}'. Use verify | restore | identity <exe>.`);
}
