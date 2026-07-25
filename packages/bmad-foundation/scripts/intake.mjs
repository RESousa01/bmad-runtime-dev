#!/usr/bin/env node
// ADR-0008 verbatim intake: ships MIT-licensed method source into the sealed
// foundation and records every member in the semantic ledger.
//
// Two rules make this safe to re-run:
//
//   1. **Append, never renumber.** Existing member ids (`method-001` …) are
//      referenced by the adoption ledger's `sourceDecisions`, by
//      `runtimeProjections[].sourceMemberIds`, and by hand-reviewed pins in
//      verify.mjs. A file already in the ledger keeps its id and its reviewed
//      treatments; only genuinely new files get the next id.
//   2. **Executables are never shipped.** ADR-0003's executable exclusion
//      survives ADR-0008: a `.py`/`.js`/`.sh`/`.html` member is recorded with a
//      `reject` treatment and its body stays out of the package. Adopting an
//      instruction body does not adopt its authority.
//
// Usage: node scripts/intake.mjs --source <path-to-upstream-checkout> [--check]

import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { dirname, join, posix, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const checkOnly = args.includes("--check");
const sourceRoot = args[args.indexOf("--source") + 1];
if (args.indexOf("--source") < 0 || sourceRoot === undefined) {
  process.stderr.write("usage: node scripts/intake.mjs --source <upstream-checkout> [--check]\n");
  process.exit(2);
}

/// Where adopted upstream bodies live inside the package. Kept separate from
/// `runtime/`, which holds Sapphirus-authored projections, so the two are
/// never confused by a reader or by the gate.
const ADOPTED_ROOT = "source/method/6.10.0";
const SOURCE_ID = "method";

/// Text and data are adoptable; anything the host could be tricked into
/// treating as a program is not.
const ADOPTABLE_EXTENSIONS = new Set([".md", ".toml", ".csv", ".yaml", ".yml", ".json"]);
const REJECTED_EXTENSIONS = new Set([".py", ".js", ".mjs", ".cjs", ".sh", ".ps1", ".bat", ".cmd", ".html", ".htm"]);

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function extensionOf(path) {
  const index = path.lastIndexOf(".");
  return index < 0 ? "" : path.slice(index).toLowerCase();
}

async function walk(directory, results = []) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const absolute = join(directory, entry.name);
    if (entry.isSymbolicLink()) continue;
    if (entry.isDirectory()) {
      await walk(absolute, results);
    } else if (entry.isFile()) {
      results.push(absolute);
    }
  }
  return results;
}

const upstreamSrc = join(sourceRoot, "src");
if (!(await stat(upstreamSrc).catch(() => null))?.isDirectory()) {
  process.stderr.write(`error: ${upstreamSrc} is not a directory\n`);
  process.exit(2);
}

const files = (await walk(upstreamSrc))
  .map((absolute) => posix.join("src", relative(upstreamSrc, absolute).split(sep).join("/")))
  .sort();

const ledgerPath = join(packageRoot, "semantic-source-ledger.json");
const ledger = JSON.parse(await readFile(ledgerPath, "utf8"));
const existing = new Map(
  ledger.sourceMembers
    .filter((member) => member.sourceId === SOURCE_ID)
    .map((member) => [member.member, member]),
);
let nextOrdinal = ledger.sourceMembers
  .map((member) => Number.parseInt(member.id.replace(/^\D+/u, ""), 10))
  .reduce((highest, value) => Math.max(highest, Number.isFinite(value) ? value : 0), 0);

const adopted = [];
const rejected = [];
const appended = [];

for (const member of files) {
  const extension = extensionOf(member);
  const isAdoptable = ADOPTABLE_EXTENSIONS.has(extension);
  const isRejected = REJECTED_EXTENSIONS.has(extension);
  if (!isAdoptable && !isRejected) continue;
  const bytes = await readFile(join(sourceRoot, member));
  const digest = sha256(bytes);

  let record = existing.get(member);
  if (record === undefined) {
    nextOrdinal += 1;
    record = {
      id: `${SOURCE_ID}-${String(nextOrdinal).padStart(3, "0")}`,
      sourceId: SOURCE_ID,
      member,
      sha256: digest,
      treatments: [],
    };
    ledger.sourceMembers.push(record);
    existing.set(member, record);
    appended.push(record.id);
  } else if (record.sha256 !== digest) {
    process.stderr.write(
      `error: ${member} drifted from its reviewed hash (${record.sha256} -> ${digest})\n`,
    );
    process.exit(1);
  }

  if (isRejected) {
    // Recorded, reviewed, never shipped.
    if (!record.treatments.some((treatment) => treatment.decision === "reject")) {
      record.treatments.push({
        decision: "reject",
        rationale: "Executable source; ADR-0003 executable exclusion survives ADR-0008.",
      });
    }
    rejected.push(member);
    continue;
  }

  if (!record.treatments.some((treatment) => treatment.decision === "adopt")) {
    record.treatments.push({
      decision: "adopt",
      rationale: "Verbatim MIT-licensed method text adopted under ADR-0008.",
    });
  }
  adopted.push({ member, bytes, digest });
}

// Preserve the reviewed grouping: every method member, then every builder
// member, each in ordinal order. The gate pins this sequence.
const memberRank = (member) => {
  const [prefix, ordinal] = member.id.split(/-(?=\d+$)/u);
  return [prefix === SOURCE_ID ? 0 : 1, Number.parseInt(ordinal, 10)];
};
ledger.sourceMembers.sort((left, right) => {
  const [leftGroup, leftOrdinal] = memberRank(left);
  const [rightGroup, rightOrdinal] = memberRank(right);
  return leftGroup - rightGroup || leftOrdinal - rightOrdinal;
});
ledger.adoptedSources = adopted.map(({ member, digest, bytes }) => ({
  sourceId: SOURCE_ID,
  member,
  packagePath: posix.join(ADOPTED_ROOT, member),
  sha256: digest,
  byteLength: bytes.byteLength,
}));

if (checkOnly) {
  process.stdout.write(
    `intake preview: ${adopted.length} adopted, ${rejected.length} rejected, `
      + `${appended.length} new members (${ledger.sourceMembers.length} total)\n`,
  );
  process.exit(0);
}

const adoptedRootAbsolute = join(packageRoot, ADOPTED_ROOT);
await rm(adoptedRootAbsolute, { recursive: true, force: true });
for (const { member, bytes } of adopted) {
  const target = join(packageRoot, ADOPTED_ROOT, member);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, bytes);
}
await writeFile(ledgerPath, `${JSON.stringify(ledger, null, 2)}\n`, "utf8");

// The adoption ledger must close over every source member: a member with no
// recorded decision is an unreviewed member. Existing decisions are left
// untouched; only genuinely new members gain one, mirroring their treatments.
const adoptionPath = join(packageRoot, "adoption-ledger.json");
const adoption = JSON.parse(await readFile(adoptionPath, "utf8"));
const decisions = new Map(
  adoption.sourceDecisions.map((decision) => [decision.sourceMemberId, decision]),
);
// The semantic ledger's member record is the source of truth for treatments;
// the adoption ledger mirrors it, and the gate proves the two agree. Intake
// only ever *appends* a treatment (adopt or reject), so mirroring the whole
// list preserves every previously reviewed decision.
for (const member of ledger.sourceMembers) {
  const existingDecision = decisions.get(member.id);
  const treatments = member.treatments.map((treatment) => ({ ...treatment }));
  if (existingDecision === undefined) {
    const decision = { sourceMemberId: member.id, treatments };
    adoption.sourceDecisions.push(decision);
    decisions.set(member.id, decision);
  } else {
    existingDecision.treatments = treatments;
  }
}
const memberOrder = new Map(ledger.sourceMembers.map((member, index) => [member.id, index]));
adoption.sourceDecisions.sort(
  (left, right) =>
    (memberOrder.get(left.sourceMemberId) ?? 0) - (memberOrder.get(right.sourceMemberId) ?? 0),
);
await writeFile(adoptionPath, `${JSON.stringify(adoption, null, 2)}\n`, "utf8");

// Every adopted body joins the manifest, so the existing byte-for-byte gate
// covers it exactly like a managed projection.
const manifestPath = join(packageRoot, "runtime-manifest.json");
const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
manifest.resources = manifest.resources.filter(
  (resource) => !resource.path.startsWith(`${ADOPTED_ROOT}/`),
);
for (const { member, bytes, digest } of adopted) {
  manifest.resources.push({
    path: posix.join(ADOPTED_ROOT, member),
    contentHash: `sha256:${digest}`,
    byteLength: bytes.byteLength,
    contentKind: "adopted_source",
  });
}
manifest.resources.sort((left, right) => (left.path < right.path ? -1 : left.path > right.path ? 1 : 0));
await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");

process.stdout.write(
  `intake complete: ${adopted.length} adopted bodies shipped, ${rejected.length} executables rejected, `
    + `${appended.length} new members appended (${ledger.sourceMembers.length} total)\n`
    + "run scripts/reseal.mjs next, then apply the reported semantic lock by hand\n",
);
