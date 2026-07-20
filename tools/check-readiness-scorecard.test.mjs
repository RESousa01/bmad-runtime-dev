import assert from "node:assert/strict";
import { test } from "node:test";
import {
  READINESS_CAPABILITIES,
  validateReadinessScorecard,
} from "./check-readiness-scorecard.mjs";

const revision = "f53645efa09b5cc4ec5a7fc0fae72454fc21f60c";
const now = 1_785_000_000_000;
const installerSha256 = "a".repeat(64);
const containerDigest = `sha256:${"b".repeat(64)}`;

function evidenceFor(kind, extra = {}) {
  return {
    kind,
    sourceRevision: revision,
    observedAt: now - 1_000,
    urlOrRef: `https://github.com/example/runs/${kind}`,
    ...extra,
  };
}

function completeCapability(capability) {
  const requiredEvidenceKinds = ["source_ci", "local_gate"];
  return {
    capability,
    status: "complete",
    percentage: 100,
    requiredEvidenceKinds,
    evidence: requiredEvidenceKinds.map((kind) =>
      evidenceFor(kind, kind === "local_gate" ? { installerSha256, containerDigest } : {}),
    ),
  };
}

function validFixture() {
  return {
    schemaVersion: "sapphirus.readiness-scorecard.v1",
    sourceRevision: revision,
    capabilities: READINESS_CAPABILITIES.map((capability) => completeCapability(capability)),
  };
}

function withCapability(fixture, capability, patch) {
  return {
    ...fixture,
    capabilities: fixture.capabilities.map((record) =>
      record.capability === capability ? { ...record, ...patch } : record,
    ),
  };
}

test("accepts the all-green fixture as release-ready", () => {
  const result = validateReadinessScorecard(validFixture(), revision, now);
  assert.equal(result.releaseReady, true);
  assert.deepEqual(result.missing, []);
});

test("rejects a missing capability", () => {
  const fixture = validFixture();
  fixture.capabilities = fixture.capabilities.slice(1);
  assert.throws(
    () => validateReadinessScorecard(fixture, revision, now),
    /missing capability bmad_foundation/,
  );
});

test("rejects an unknown capability", () => {
  const fixture = validFixture();
  fixture.capabilities.push({ ...completeCapability("bmad_foundation"), capability: "extra_scope" });
  assert.throws(
    () => validateReadinessScorecard(fixture, revision, now),
    /unknown capability extra_scope/,
  );
});

test("rejects complete records whose percentage is not 100", () => {
  const fixture = withCapability(validFixture(), "full_bmad_breadth", { percentage: 99 });
  assert.throws(
    () => validateReadinessScorecard(fixture, revision, now),
    /complete requires percentage 100/,
  );
});

test("rejects percentage 100 on an incomplete record", () => {
  const fixture = withCapability(validFixture(), "full_bmad_breadth", { status: "incomplete" });
  assert.throws(
    () => validateReadinessScorecard(fixture, revision, now),
    /percentage 100 requires status complete/,
  );
});

test("rejects complete records with empty or insufficient evidence", () => {
  const fixture = withCapability(validFixture(), "internal_pilot_readiness", { evidence: [] });
  assert.throws(
    () => validateReadinessScorecard(fixture, revision, now),
    /complete without required evidence/,
  );
});

test("rejects a mismatched source revision", () => {
  assert.throws(
    () => validateReadinessScorecard(
      { ...validFixture(), sourceRevision: "0".repeat(40) },
      revision,
      now,
    ),
    /source revision mismatch/,
  );
});

test("rejects evidence bound to a different revision", () => {
  const fixture = withCapability(validFixture(), "bmad_foundation", {
    evidence: [
      evidenceFor("source_ci", { sourceRevision: "0".repeat(40) }),
      evidenceFor("local_gate"),
    ],
  });
  assert.throws(
    () => validateReadinessScorecard(fixture, revision, now),
    /source revision mismatch/,
  );
});

test("rejects mismatched installer hashes and container digests", () => {
  const fixture = withCapability(validFixture(), "deterministic_help_backend", {
    evidence: [
      evidenceFor("source_ci", { installerSha256: "c".repeat(64) }),
      evidenceFor("local_gate"),
    ],
  });
  assert.throws(
    () => validateReadinessScorecard(fixture, revision, now),
    /installer hash mismatch/,
  );
  const digestFixture = withCapability(validFixture(), "deterministic_help_backend", {
    evidence: [
      evidenceFor("source_ci", { containerDigest: `sha256:${"d".repeat(64)}` }),
      evidenceFor("local_gate"),
    ],
  });
  assert.throws(
    () => validateReadinessScorecard(digestFixture, revision, now),
    /container digest mismatch/,
  );
});

test("rejects duplicate evidence kinds", () => {
  const fixture = withCapability(validFixture(), "bmad_foundation", {
    evidence: [
      evidenceFor("source_ci"),
      evidenceFor("source_ci"),
      evidenceFor("local_gate"),
    ],
  });
  assert.throws(
    () => validateReadinessScorecard(fixture, revision, now),
    /duplicate evidence kind source_ci/,
  );
});

test("rejects expired and future evidence", () => {
  const expired = withCapability(validFixture(), "bmad_foundation", {
    evidence: [
      evidenceFor("source_ci", { expiresAt: now - 1 }),
      evidenceFor("local_gate"),
    ],
  });
  assert.throws(
    () => validateReadinessScorecard(expired, revision, now),
    /is expired/,
  );
  const future = withCapability(validFixture(), "bmad_foundation", {
    evidence: [
      evidenceFor("source_ci", { observedAt: now + 60_000 }),
      evidenceFor("local_gate"),
    ],
  });
  assert.throws(
    () => validateReadinessScorecard(future, revision, now),
    /observed in the future/,
  );
});

test("reports incomplete capabilities without throwing", () => {
  const fixture = withCapability(validFixture(), "internal_pilot_readiness", {
    status: "incomplete",
    percentage: 20,
    evidence: [],
  });
  const result = validateReadinessScorecard(fixture, revision, now);
  assert.equal(result.releaseReady, false);
  assert.equal(result.missing.length, 1);
  assert.equal(result.missing[0].capability, "internal_pilot_readiness");
  assert.deepEqual(result.missing[0].absentKinds, ["source_ci", "local_gate"]);
});

test("rejects unknown keys anywhere in the document", () => {
  assert.throws(
    () => validateReadinessScorecard({ ...validFixture(), notes: "extra" }, revision, now),
    /unknown key notes/,
  );
  const fixture = withCapability(validFixture(), "bmad_foundation", { waiver: true });
  assert.throws(
    () => validateReadinessScorecard(fixture, revision, now),
    /unknown key waiver/,
  );
});
