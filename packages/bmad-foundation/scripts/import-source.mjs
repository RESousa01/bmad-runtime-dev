import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  lstat,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { dirname, posix, relative, resolve, sep } from "node:path";
import process from "node:process";
import { inflateRawSync } from "node:zlib";

const packageRoot = resolve(import.meta.dirname, "..");
const sourceRoot = resolve(packageRoot, "source");
const manifestPath = resolve(packageRoot, "source-manifest.json");
const deviceName = /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/iu;

function normalize(path) {
  return path.split(sep).join("/");
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function exactSourceTarget(source) {
  const target = resolve(sourceRoot, source.id, source.version);
  assert.ok(
    target.startsWith(`${sourceRoot}${sep}`),
    `${source.id} source target escapes the package source root.`,
  );
  return target;
}

function safeRelativePath(path, label) {
  assert.ok(path && !path.includes("\0"), `${label} is empty or contains NUL.`);
  assert.ok(!path.includes("\\"), `${label} contains a backslash.`);
  assert.equal(path, path.normalize("NFC"), `${label} is not NFC-normalized.`);
  assert.ok(!path.startsWith("/") && !/^[A-Za-z]:/u.test(path), `${label} is absolute.`);
  const segments = path.split("/");
  for (const segment of segments) {
    assert.ok(segment && segment !== "." && segment !== "..", `${label} contains traversal.`);
    assert.ok(!segment.endsWith(".") && !segment.endsWith(" "), `${label} has an unsafe suffix.`);
    assert.ok(!segment.includes(":"), `${label} contains an alternate-stream separator.`);
    assert.ok(!deviceName.test(segment), `${label} contains a reserved device name.`);
  }
  return segments.join("/");
}

function findEndOfCentralDirectory(archive) {
  const minimum = Math.max(0, archive.length - 65_557);
  for (let offset = archive.length - 22; offset >= minimum; offset -= 1) {
    if (archive.readUInt32LE(offset) === 0x06054b50) return offset;
  }
  throw new Error("ZIP end-of-central-directory record is missing.");
}

function readZip(archive, source) {
  const end = findEndOfCentralDirectory(archive);
  const disk = archive.readUInt16LE(end + 4);
  const centralDisk = archive.readUInt16LE(end + 6);
  const entriesOnDisk = archive.readUInt16LE(end + 8);
  const entryCount = archive.readUInt16LE(end + 10);
  const centralSize = archive.readUInt32LE(end + 12);
  const centralOffset = archive.readUInt32LE(end + 16);
  assert.equal(disk, 0, "multi-disk ZIP archives are unsupported.");
  assert.equal(centralDisk, 0, "multi-disk ZIP archives are unsupported.");
  assert.equal(entriesOnDisk, entryCount, "ZIP entry counts disagree.");
  assert.notEqual(entryCount, 0xffff, "ZIP64 archives require an explicit importer upgrade.");
  assert.notEqual(centralOffset, 0xffffffff, "ZIP64 archives require an explicit importer upgrade.");
  assert.equal(centralOffset + centralSize, end, "ZIP central directory is not canonical.");

  const files = [];
  const seen = new Set();
  const seenCase = new Map();
  let cursor = centralOffset;
  for (let index = 0; index < entryCount; index += 1) {
    assert.equal(archive.readUInt32LE(cursor), 0x02014b50, `ZIP central entry ${index} is malformed.`);
    const flags = archive.readUInt16LE(cursor + 8);
    const compression = archive.readUInt16LE(cursor + 10);
    const compressedSize = archive.readUInt32LE(cursor + 20);
    const byteLength = archive.readUInt32LE(cursor + 24);
    const nameLength = archive.readUInt16LE(cursor + 28);
    const extraLength = archive.readUInt16LE(cursor + 30);
    const commentLength = archive.readUInt16LE(cursor + 32);
    const externalAttributes = archive.readUInt32LE(cursor + 38);
    const localOffset = archive.readUInt32LE(cursor + 42);
    assert.equal(flags & 0x1, 0, "encrypted ZIP entries are unsupported.");
    assert.ok(compression === 0 || compression === 8, `unsupported ZIP compression method ${compression}.`);

    const archivePath = archive.subarray(cursor + 46, cursor + 46 + nameLength).toString("utf8");
    cursor += 46 + nameLength + extraLength + commentLength;
    if (archivePath === `${source.archive_root}/`) continue;
    assert.ok(
      archivePath.startsWith(`${source.archive_root}/`),
      `archive member escapes the declared root: ${archivePath}`,
    );
    const relativePath = safeRelativePath(
      archivePath.slice(source.archive_root.length + 1).replace(/\/$/u, ""),
      archivePath,
    );
    const unixMode = externalAttributes >>> 16;
    const unixKind = unixMode & 0xf000;
    if (archivePath.endsWith("/")) continue;
    assert.ok(!seen.has(relativePath), `duplicate normalized path: ${relativePath}`);
    const folded = relativePath.toLocaleLowerCase("en-US");
    assert.ok(!seenCase.has(folded), `case-colliding paths: ${seenCase.get(folded)} and ${relativePath}`);
    seen.add(relativePath);
    seenCase.set(folded, relativePath);

    assert.equal(archive.readUInt32LE(localOffset), 0x04034b50, `local header is missing: ${archivePath}`);
    const localNameLength = archive.readUInt16LE(localOffset + 26);
    const localExtraLength = archive.readUInt16LE(localOffset + 28);
    const dataOffset = localOffset + 30 + localNameLength + localExtraLength;
    const compressed = archive.subarray(dataOffset, dataOffset + compressedSize);
    const bytes = compression === 0 ? Buffer.from(compressed) : inflateRawSync(compressed);
    assert.equal(bytes.length, byteLength, `uncompressed size drifted: ${archivePath}`);
    const entryType = unixKind === 0xa000 ? "symlink_metadata" : "file";
    let linkTarget = null;
    if (entryType === "symlink_metadata") {
      linkTarget = bytes.toString("utf8");
      assert.ok(
        linkTarget
        && !linkTarget.includes("\0")
        && !linkTarget.includes("\\")
        && !linkTarget.startsWith("/")
        && !/^[A-Za-z]:/u.test(linkTarget),
        `unsafe symbolic link target: ${archivePath}`,
      );
      const resolvedTarget = posix.normalize(posix.join(posix.dirname(relativePath), linkTarget));
      assert.ok(
        resolvedTarget !== ".."
        && !resolvedTarget.startsWith("../")
        && !posix.isAbsolute(resolvedTarget),
        `symbolic link escapes the archive root: ${archivePath}`,
      );
    }
    files.push({
      path: relativePath,
      bytes,
      entry_type: entryType,
      link_target: linkTarget,
      archive_mode: unixMode,
      is_executable: entryType === "file" && (unixMode & 0o111) !== 0,
    });
  }
  assert.equal(cursor, end, "ZIP central directory length drifted.");
  return files.sort((left, right) => left.path.localeCompare(right.path, "en"));
}

function classify(sourceId, path) {
  if (path.endsWith("/SKILL.md") || path === "SKILL.md") {
    if (sourceId === "method" && path.startsWith("src/")) return "primary_method_skill";
    if (sourceId === "method" && path.startsWith("test/fixtures/")) return "upstream_test_fixture";
    if (sourceId === "method" && path.startsWith("web-bundles/")) return "web_distribution_bundle";
    if (sourceId === "builder" && /^skills\/[^/]+\/SKILL\.md$/u.test(path)) {
      return "primary_builder_skill";
    }
    if (sourceId === "builder" && path === "skills/bmad-module-builder/assets/setup-skill-template/SKILL.md") {
      return "nested_builder_template";
    }
    if (sourceId === "builder" && path.startsWith("samples/")) return "builder_sample_fixture";
  }
  if (path === "LICENSE" || path === "TRADEMARK.md") return "legal";
  if (/^(?:test|tests|samples)\//u.test(path)) return "test_or_sample_fixture";
  if (/(?:^|\/)(?:assets?|templates?)\//u.test(path)) return "template_or_asset";
  if (/\.(?:cjs|js|mjs|ps1|py|sh)$/iu.test(path)) return "script_source";
  if (
    /^(?:\.claude-plugin|\.github|website)\//u.test(path)
    || /^(?:package(?:-lock)?\.json|pnpm-lock\.yaml|AGENTS\.md)$/u.test(path)
  ) {
    return "distribution_metadata";
  }
  if (/\.(?:md|mdx)$/iu.test(path)) return "documentation";
  return "source_resource";
}

function sourceSummary(source, files) {
  const members = files.map((file) => ({
    source_id: source.id,
    path: file.path,
    byte_length: file.bytes.length,
    sha256: sha256(file.bytes),
    class: file.entry_type === "symlink_metadata" ? "internal_symlink_metadata" : classify(source.id, file.path),
    entry_type: file.entry_type,
    link_target: file.link_target,
    archive_mode: file.archive_mode,
    is_executable: file.is_executable,
  }));
  return {
    source: {
      id: source.id,
      version: source.version,
      archive_sha256: source.archive_sha256,
      archive_root: source.archive_root,
      regular_file_count: members.length,
      skill_md_count: members.filter((member) => member.path.endsWith("/SKILL.md")).length,
    },
    members,
  };
}

async function stageSource(source, files) {
  const stage = resolve(sourceRoot, `.import-${source.id}-${process.pid}`);
  assert.ok(stage.startsWith(`${sourceRoot}${sep}.import-`), "unsafe import staging path.");
  await rm(stage, { recursive: true, force: true });
  await mkdir(stage, { recursive: true });
  for (const file of files) {
    if (file.entry_type === "symlink_metadata") continue;
    const destination = resolve(stage, ...file.path.split("/"));
    assert.ok(destination.startsWith(`${stage}${sep}`), `staged path escapes: ${file.path}`);
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(destination, file.bytes, { mode: file.is_executable ? 0o555 : 0o444 });
  }
  return stage;
}

async function replaceSource(source, stage) {
  const target = exactSourceTarget(source);
  const backup = resolve(sourceRoot, `.backup-${source.id}-${process.pid}`);
  assert.ok(backup.startsWith(`${sourceRoot}${sep}.backup-`), "unsafe import backup path.");
  let backedUp = false;
  try {
    await rm(backup, { recursive: true, force: true });
    try {
      await rename(target, backup);
      backedUp = true;
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
    await mkdir(dirname(target), { recursive: true });
    await rename(stage, target);
    if (backedUp) await rm(backup, { recursive: true, force: true });
  } catch (error) {
    try {
      await rm(target, { recursive: true, force: true });
      if (backedUp) await rename(backup, target);
    } catch (rollbackError) {
      throw new AggregateError([error, rollbackError], `failed to replace and restore ${source.id} source`);
    }
    throw error;
  }
}

async function walkFiles(root) {
  const files = [];
  async function visit(directory) {
    const metadata = await lstat(directory);
    assert.ok(metadata.isDirectory() && !metadata.isSymbolicLink(), `linked source directory: ${directory}`);
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const absolute = resolve(directory, entry.name);
      const path = normalize(relative(root, absolute));
      if (entry.isSymbolicLink()) throw new Error(`linked source entry: ${path}`);
      if (entry.isDirectory()) await visit(absolute);
      else if (entry.isFile()) files.push(path);
      else throw new Error(`unsupported source entry: ${path}`);
    }
  }
  await visit(root);
  return files.sort((left, right) => left.localeCompare(right, "en"));
}

function assertClassCounts(source, members) {
  const count = (kind) => members.filter((member) => member.class === kind).length;
  if (source.id === "method") {
    assert.equal(count("primary_method_skill"), 48, "Method primary skill count drifted.");
    assert.equal(count("upstream_test_fixture"), 3, "Method validator fixture count drifted.");
    assert.equal(count("web_distribution_bundle"), 6, "Method web bundle count drifted.");
  } else {
    assert.equal(count("primary_builder_skill"), 5, "Builder production skill count drifted.");
    assert.equal(count("nested_builder_template"), 1, "Builder nested template count drifted.");
    assert.equal(count("builder_sample_fixture"), 7, "Builder sample count drifted.");
  }
}

async function checkRetained(lock) {
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  assert.equal(manifest.schema_version, "sapphirus.bmad.source-manifest/v1");
  assert.deepEqual(
    manifest.sources.map((source) => source.id),
    ["builder", "method"],
    "source manifest order drifted.",
  );
  const sortedMembers = [...manifest.members].sort((left, right) => {
    const sourceOrder = left.source_id.localeCompare(right.source_id, "en");
    return sourceOrder || left.path.localeCompare(right.path, "en");
  });
  assert.deepEqual(manifest.members, sortedMembers, "source manifest members are not sorted.");

  for (const source of lock) {
    const summary = manifest.sources.find((item) => item.id === source.id);
    assert.ok(summary, `${source.id} source summary is missing.`);
    assert.equal(summary.archive_sha256, source.archive_sha256);
    assert.equal(summary.regular_file_count, source.regular_file_count);
    assert.equal(summary.skill_md_count, source.skill_md_count);
    const members = manifest.members.filter((member) => member.source_id === source.id);
    assert.equal(members.length, source.regular_file_count, `${source.id} manifest count drifted.`);
    assertClassCounts(source, members);

    const root = exactSourceTarget(source);
    const materializedMembers = members.filter((member) => member.entry_type === "file");
    const diskPaths = await walkFiles(root);
    const compatibilitySidecars = source.id === "builder"
      ? ["distribution-decision.json", "source-notes.json"]
      : [];
    assert.deepEqual(
      diskPaths,
      [...materializedMembers.map((member) => member.path), ...compatibilitySidecars]
        .sort((left, right) => left.localeCompare(right, "en")),
      `${source.id} retained paths drifted.`,
    );
    await Promise.all(members.map(async (member) => {
      safeRelativePath(member.path, `${source.id}:${member.path}`);
      if (member.entry_type === "symlink_metadata") {
        assert.equal(member.class, "internal_symlink_metadata");
        assert.match(member.link_target ?? "", /\S/u);
        return;
      }
      const bytes = await readFile(resolve(root, ...member.path.split("/")));
      assert.equal(bytes.length, member.byte_length, `${source.id}:${member.path} byte length drifted.`);
      assert.equal(sha256(bytes), member.sha256, `${source.id}:${member.path} hash drifted.`);
      assert.equal(classify(source.id, member.path), member.class, `${source.id}:${member.path} class drifted.`);
    }));
  }
  console.log("source import verified: method=597 builder=299");
}

function argument(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

async function main() {
  const lockDocument = JSON.parse(await readFile(resolve(packageRoot, "source-lock.json"), "utf8"));
  const lock = [...lockDocument.sources].sort((left, right) => left.id.localeCompare(right.id, "en"));
  if (process.argv.includes("--check")) {
    await checkRetained(lock);
    return;
  }
  assert.ok(process.argv.includes("--import"), "use --import to replace retained source trees or --check to verify.");

  const archivePaths = {
    method: argument("--method-archive") ?? process.env.BMAD_METHOD_ARCHIVE,
    builder: argument("--builder-archive") ?? process.env.BMAD_BUILDER_ARCHIVE,
  };
  const summaries = [];
  const stages = [];
  for (const source of lock) {
    const archivePath = archivePaths[source.id];
    assert.ok(archivePath, `${source.id} archive path is required.`);
    const archive = await readFile(archivePath);
    assert.equal(archive.length, source.archive_byte_length, `${source.id} archive byte length drifted.`);
    assert.equal(sha256(archive), source.archive_sha256, `${source.id} archive SHA-256 drifted.`);
    const files = readZip(archive, source);
    assert.equal(files.length, source.regular_file_count, `${source.id} archive file count drifted.`);
    const summary = sourceSummary(source, files);
    assert.equal(summary.source.skill_md_count, source.skill_md_count, `${source.id} skill count drifted.`);
    assertClassCounts(source, summary.members);
    summaries.push(summary);
    stages.push({ source, stage: await stageSource(source, files) });
  }

  for (const staged of stages) await replaceSource(staged.source, staged.stage);
  const manifest = {
    schema_version: "sapphirus.bmad.source-manifest/v1",
    importer: lockDocument.importer,
    sources: summaries.map((summary) => summary.source),
    members: summaries.flatMap((summary) => summary.members),
  };
  const temporaryManifest = `${manifestPath}.tmp-${process.pid}`;
  await writeFile(temporaryManifest, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  await rm(manifestPath, { force: true });
  await rename(temporaryManifest, manifestPath);
  await checkRetained(lock);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exitCode = 1;
});
