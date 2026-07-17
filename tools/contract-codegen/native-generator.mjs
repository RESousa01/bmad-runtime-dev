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
  const exeBytes = await readFile(vendorExe);
  const metaBytes = await readFile(vendorMeta);

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
  return { exeBytes, metaBytes };
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
  process.stdout.write(
    `native-generator: restored reviewed cargo-typify to ${rust.resolvedExecutable} and ${rust.installMetadata.path}.\n`,
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
