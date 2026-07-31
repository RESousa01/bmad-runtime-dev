import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import {
  compareIsoDate,
  fail,
  parseArgs,
  readJson,
  rootFrom,
  todayIso,
} from "./lib.mjs";

function visit(id, claims, visiting, visited) {
  if (visited.has(id)) return;
  if (visiting.has(id)) throw new Error(`claim supersession cycle includes ${id}`);
  visiting.add(id);
  const claim = claims.get(id);
  for (const target of claim?.supersedes ?? []) {
    if (!claims.has(target)) throw new Error(`${id}: unknown supersession target ${target}`);
    visit(target, claims, visiting, visited);
  }
  visiting.delete(id);
  visited.add(id);
}

try {
  const options = parseArgs(process.argv.slice(2));
  const root = rootFrom(options);
  const registry = readJson(resolve(root, "docs/_meta/claims.json"));
  const manifestPath = resolve(root, "docs/_meta/knowledge-manifest.json");
  const manifest = existsSync(manifestPath) ? readJson(manifestPath) : {};
  const allowedGates = new Set(manifest.allowedGates ?? []);
  const claims = new Map((registry.claims ?? []).map((claim) => [claim.id, claim]));
  const archiveLimitations = [];
  for (const claim of claims.values()) {
    for (const field of [
      "id",
      "subject",
      "class",
      "statement",
      "status",
      "ownerRole",
      "verifiedAt",
      "reviewDue",
      "evidence",
      "supersedes",
    ]) {
      if (claim[field] === undefined || claim[field] === null) {
        throw new Error(`${claim.id ?? "<claim>"}: missing ${field}`);
      }
    }
    if (claim.status === "current" && compareIsoDate(claim.reviewDue, todayIso()) < 0) {
      throw new Error(`${claim.id}: current claim is past reviewDue`);
    }
    const evidence = claim.evidence ?? [];
    if (evidence.length === 0) throw new Error(`${claim.id}: claim has no evidence`);
    if (
      claim.class === "implemented" &&
      evidence.every((item) => /(?:^|\/)(?:plans?|reviews?)(?:\/|$)/i.test(item.path ?? ""))
    ) {
      throw new Error(`${claim.id}: implemented claim relies only on plan or review prose`);
    }
    for (const item of evidence) {
      if (item.kind === "gate" && !allowedGates.has(item.command)) {
        throw new Error(`${claim.id}: gate command is not allowlisted`);
      }
      if (item.path && !existsSync(resolve(root, item.path))) {
        throw new Error(`${claim.id}: missing evidence path ${item.path}`);
      }
      if (item.commit) {
        if (!/^[0-9a-f]{40}$/.test(item.commit)) {
          throw new Error(`${claim.id}: invalid evidence commit`);
        }
        if (options["archive-mode"]) {
          archiveLimitations.push(`${claim.id}:${item.commit}:not_verified_in_archive`);
        } else {
          const git = spawnSync("git", ["cat-file", "-e", `${item.commit}^{commit}`], {
            cwd: root,
            encoding: "utf8",
          });
          if (git.status !== 0) throw new Error(`${claim.id}: evidence commit is unavailable`);
        }
      }
    }
  }
  for (const id of claims.keys()) visit(id, claims, new Set(), new Set());
  for (const limitation of archiveLimitations) process.stdout.write(`${limitation}\n`);
  process.stdout.write(`Claims verified (${claims.size})\n`);
} catch (error) {
  fail(error);
}
