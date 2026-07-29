import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createReadStream, readFileSync, statSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { basename, resolve } from "node:path";
import process from "node:process";

const packageRoot = resolve(import.meta.dirname, "..");

const expected = Object.freeze({
  method: Object.freeze({
    package_name: "bmad-method",
    version: "6.10.0",
    archive_sha256: "2993a38a36a9586d16a62a4eacb3400db25dd1ea594bbfe4682789d7f95ac02b",
    regular_file_count: 597,
    skill_md_count: 57,
    primary_skill_count: 48,
    node: ">=20.12.0",
  }),
  builder: Object.freeze({
    package_name: "bmad-builder",
    version: "2.1.0",
    archive_sha256: "d3c70744a9875623b01856cc907cf558324bacc920f0d860c36ad2788a4d2852",
    regular_file_count: 299,
    skill_md_count: 13,
    primary_skill_count: 5,
    public_action_count: 9,
    internal_action_count: 1,
    node: ">=22.0.0",
  }),
});

function readJson(path) {
  return JSON.parse(readFileSync(resolve(packageRoot, path), "utf8"));
}

function indexById(items, label) {
  assert.ok(Array.isArray(items), `${label} must be an array.`);
  const indexed = new Map(items.map((item) => [item.id, item]));
  assert.equal(indexed.size, items.length, `${label} contains duplicate ids.`);
  assert.deepEqual([...indexed.keys()].sort(), ["builder", "method"], `${label} must contain exactly builder and method.`);
  return indexed;
}

function verifyMetadata() {
  const lock = readJson("source-lock.json");
  const ledger = readJson("semantic-source-ledger.json");
  assert.equal(lock.schema_version, "sapphirus.bmad.source-lock/v1");
  assert.deepEqual(
    lock.acceptance_policy?.required_upgrade_record,
    ["source_diff", "semantic_impact", "compatibility_impact", "migration", "rollback", "approval"],
    "source upgrades require the complete migration and approval record.",
  );
  assert.equal(lock.acceptance_policy?.changed_archive_bytes, "reject");
  assert.equal(lock.acceptance_policy?.folder_name_as_identity, "reject");

  const locked = indexById(lock.sources, "source-lock.json sources");
  const semantic = indexById(ledger.sources, "semantic-source-ledger.json sources");
  for (const [id, facts] of Object.entries(expected)) {
    const source = locked.get(id);
    for (const [field, value] of Object.entries(facts)) {
      if (field === "node") {
        assert.equal(source.runtime?.node, value, `${id} runtime floor drifted.`);
      } else {
        assert.deepEqual(source[field], value, `${id}.${field} drifted.`);
      }
    }
    assert.equal(
      semantic.get(id).archiveSha256,
      source.archive_sha256,
      `${id} semantic ledger and source lock disagree.`,
    );
    assert.equal(
      semantic.get(id).archiveArtifactLabel,
      source.canonical_archive_label,
      `${id} canonical archive label drifted.`,
    );
  }

  const staleMethod = locked
    .get("method")
    .historical_identities
    .find((item) => item.archive_sha256 === "a7c049038099b99081fbd03d22c6a5180edd88dee656bb37c4276b1cc31b4a32");
  assert.equal(staleMethod?.status, "superseded_stale_snapshot", "the prior Method identity must remain auditable.");

  const provenance = readFileSync(resolve(packageRoot, "..", "..", "docs", "bmad", "source-provenance.md"), "utf8");
  const notice = readFileSync(resolve(packageRoot, "NOTICE.md"), "utf8");
  for (const facts of Object.values(expected)) {
    assert.ok(provenance.includes(facts.archive_sha256), "source provenance omits a pinned archive hash.");
  }
  assert.ok(notice.includes("source-lock.json"), "NOTICE must point to the authoritative source lock.");
  return locked;
}

async function sha256File(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

function tar(path, args, encoding = "utf8") {
  const result = spawnSync("tar", [...args, path], {
    encoding,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`tar failed for ${basename(path)}: ${String(result.stderr).trim()}`);
  }
  return result.stdout;
}

function archiveEntries(path) {
  return tar(path, ["-tf"]).split(/\r?\n/u).filter(Boolean);
}

function readArchiveMember(path, member) {
  const result = spawnSync("tar", ["-xOf", path, member], {
    encoding: null,
    maxBuffer: 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`cannot read ${member} from ${basename(path)}: ${String(result.stderr).trim()}`);
  }
  return result.stdout;
}

function sha256Bytes(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function verifyArchive(source, path) {
  assert.ok(path, `${source.id} archive path is required.`);
  assert.equal(statSync(path).size, source.archive_byte_length, `${source.id} archive byte length drifted.`);
  assert.equal(await sha256File(path), source.archive_sha256, `${source.id} archive SHA-256 drifted.`);

  const entries = archiveEntries(path);
  assert.equal(
    entries.filter((entry) => !entry.endsWith("/")).length,
    source.regular_file_count,
    `${source.id} regular file count drifted.`,
  );
  assert.equal(
    entries.filter((entry) => entry.endsWith("/SKILL.md")).length,
    source.skill_md_count,
    `${source.id} SKILL.md count drifted.`,
  );

  for (const [member, expectedHash] of Object.entries(source.identity_members)) {
    const bytes = readArchiveMember(path, `${source.archive_root}/${member}`);
    assert.equal(sha256Bytes(bytes), expectedHash, `${source.id} ${member} identity drifted.`);
    if (member === "package.json") {
      const manifest = JSON.parse(bytes.toString("utf8"));
      assert.equal(manifest.name, source.package_name, `${source.id} package name drifted.`);
      assert.equal(manifest.version, source.version, `${source.id} package version drifted.`);
      assert.equal(manifest.engines?.node, source.runtime.node, `${source.id} Node floor drifted.`);
    }
  }
}

function argument(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

async function main() {
  assert.ok(process.argv.includes("--check"), "verification requires --check.");
  const sources = verifyMetadata();
  if (!process.argv.includes("--metadata-only")) {
    const methodArchive = argument("--method-archive") ?? process.env.BMAD_METHOD_ARCHIVE;
    const builderArchive = argument("--builder-archive") ?? process.env.BMAD_BUILDER_ARCHIVE;
    await verifyArchive(sources.get("method"), methodArchive);
    await verifyArchive(sources.get("builder"), builderArchive);
  }
  console.log("source identities verified: method=6.10.0 builder=2.1.0");
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
