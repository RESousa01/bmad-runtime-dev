import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  ContractValidationError,
  parseAndValidateContract,
} from "../generated/typescript/validation.mjs";
import {
  validateContractErrorSemantics,
  validateMethodAdvanceResultSemantics,
  validateMethodHelpProposalSemantics,
  validateMethodHelpRecommendationSemantics,
  validatePackageCompatibilitySemantics,
  validateRemoteJobHandoffSemantics,
  validateRemoteJobHandoffTransition,
} from "../generated/typescript/semantic-validation.mjs";
import {
  validateCandidateAction,
  validateBmadMethodAdvanceResult,
  validateBmadMethodHelpProposal,
  validateBmadMethodHelpRecommendation,
  validateContractError,
  validateFilesystemCapability,
  validatePackageCompatibility,
  validateRemoteJobHandoff,
} from "../generated/typescript/validators.mjs";

const packageRoot = fileURLToPath(new URL("../", import.meta.url));

async function fixture(relativePath) {
  return readFile(path.join(packageRoot, "fixtures", relativePath), "utf8");
}

test("standalone Ajv validator accepts the sealed candidate fixture", async () => {
  const source = await fixture("valid/windows-local-candidate.json");
  const value = parseAndValidateContract(source, "candidate-action");
  assert.equal(value.schemaVersion, "sapphirus.candidate-action.v1");
  assert.equal(validateCandidateAction(value), true);
});

test("strict parsing rejects duplicate keys before standalone validation", async () => {
  const source = await fixture("invalid/duplicate-member.json");
  assert.throws(
    () => parseAndValidateContract(source, "candidate-action"),
    (error) => error?.code === "DUPLICATE_MEMBER",
  );
});

test("standalone sealed Help roots enforce structure, semantics, and strict host timestamps", async () => {
  const cases = [
    ["method-help-proposal", "bmad-method-help-proposal", validateBmadMethodHelpProposal],
    [
      "method-help-recommendation",
      "bmad-method-help-recommendation",
      validateBmadMethodHelpRecommendation,
    ],
    ["method-advance-result", "bmad-method-advance-result", validateBmadMethodAdvanceResult],
  ];
  for (const [name, kind, validate] of cases) {
    const source = await fixture(`valid/bmad/${name}.json`);
    const value = parseAndValidateContract(source, kind);
    assert.equal(validate(value), true, name);
  }

  const proposal = JSON.parse(await fixture("valid/bmad/method-help-proposal.json"));
  assert.deepEqual(validateMethodHelpProposalSemantics(proposal), []);
  proposal.rationaleSummary = "unsafe\u202etext";
  assert.throws(
    () => parseAndValidateContract(JSON.stringify(proposal), "bmad-method-help-proposal"),
    (error) => error instanceof ContractValidationError
      && error.issues.some((issue) => issue.keyword === "BMAD_UNSAFE_TEXT"),
  );

  const recommendation = JSON.parse(await fixture("valid/bmad/method-help-recommendation.json"));
  assert.deepEqual(validateMethodHelpRecommendationSemantics(recommendation), []);
  recommendation.createdAt = "2026-02-31T10:00:00.000Z";
  assert.ok(validateMethodHelpRecommendationSemantics(recommendation)
    .some(({ code, field }) => code === "INVALID_UTC_INSTANT" && field === "createdAt"));
  recommendation.createdAt = "0000-02-29T10:00:00.000Z";
  assert.ok(!validateMethodHelpRecommendationSemantics(recommendation)
    .some(({ code }) => code === "INVALID_UTC_INSTANT"));

  const advanceResult = JSON.parse(await fixture("valid/bmad/method-advance-result.json"));
  assert.deepEqual(validateMethodAdvanceResultSemantics(advanceResult), []);
  advanceResult.receivedAt = "2026-02-31T10:00:00.000Z";
  assert.ok(validateMethodAdvanceResultSemantics(advanceResult)
    .some(({ code, field }) => code === "INVALID_UTC_INSTANT" && field === "receivedAt"));

  proposal.rationaleSummary = "safe";
  proposal.untrustedAuthority = true;
  assert.equal(validateBmadMethodHelpProposal(proposal), false);
  proposal.evidenceTokenIds = [];
  delete proposal.untrustedAuthority;
  assert.equal(validateBmadMethodHelpProposal(proposal), false);
  proposal.evidenceTokenIds = Array.from(
    { length: 65 },
    (_, index) => `evidence_01J${index.toString(32).toUpperCase().padStart(16, "0")}`,
  );
  assert.equal(validateBmadMethodHelpProposal(proposal), false);

  assert.throws(
    () => parseAndValidateContract(
      '{"proposalKind":"no_recommendation","reasonCode":"catalog_evidence_absent","reasonCode":"dependency_unavailable"}',
      "bmad-method-help-proposal",
    ),
    (error) => error?.code === "DUPLICATE_MEMBER",
  );
});

test("standalone validation fails closed on unknown properties and contract kinds", async () => {
  const source = await fixture("invalid/unknown-property.json");
  assert.throws(
    () => parseAndValidateContract(source, "candidate-action"),
    (error) =>
      error instanceof ContractValidationError
      && error.issues.some((issue) => issue.keyword === "additionalProperties"),
  );
  assert.throws(
    () => parseAndValidateContract("{}", "future-contract"),
    (error) =>
      error instanceof ContractValidationError
      && error.issues[0]?.keyword === "unknown_contract_kind",
  );
});

test("the public parser fails closed on BMAD semantics and required cross-record context", async () => {
  const descriptorSource = await fixture("valid/bmad/package-descriptor.json");
  const descriptor = parseAndValidateContract(
    descriptorSource,
    "bmad-package-descriptor",
  );
  const catalogSource = await fixture("valid/bmad/capability-catalog.json");
  assert.throws(
    () => parseAndValidateContract(catalogSource, "bmad-capability-catalog"),
    (error) => error instanceof ContractValidationError
      && error.issues.some((issue) => issue.keyword === "BMAD_SEMANTIC_CONTEXT_REQUIRED"),
  );
  const catalog = parseAndValidateContract(
    catalogSource,
    "bmad-capability-catalog",
    { descriptor },
  );
  const methodSource = await fixture("valid/bmad/method-architect-iterative.json");
  assert.throws(
    () => parseAndValidateContract(methodSource, "bmad-method-session"),
    (error) => error instanceof ContractValidationError
      && error.issues.some((issue) => issue.keyword === "BMAD_SEMANTIC_CONTEXT_REQUIRED"),
  );
  assert.equal(
    parseAndValidateContract(methodSource, "bmad-method-session", { catalog })
      .payload.methodShape,
    "architect_iterative",
  );

  for (const [file, kind, context, reason] of [
    [
      "invalid/bmad/projection-hash-substitution.json",
      "bmad-package-descriptor",
      undefined,
      "BMAD_INSTRUCTION_PROJECTION_HASH_MISMATCH",
    ],
    [
      "invalid/bmad/agent-record-hash-substitution.json",
      "bmad-capability-catalog",
      { descriptor },
      "BMAD_AGENT_ROSTER_BINDING_MISMATCH",
    ],
    [
      "invalid/bmad/method-agent-record-transplant.json",
      "bmad-method-session",
      { catalog },
      "BMAD_METHOD_AGENT_CATALOG_TRANSPLANT",
    ],
    [
      "invalid/bmad/builder-windows-reserved-path.json",
      "bmad-builder-authoring",
      undefined,
      "BMAD_BUILDER_PATH_INVALID",
    ],
  ]) {
    const source = await fixture(file);
    assert.throws(
      () => parseAndValidateContract(source, kind, context),
      (error) => error instanceof ContractValidationError
        && error.issues.some((issue) => issue.keyword === reason),
      file,
    );
  }
});

test("standalone validators accept all four schema-first extension families", async () => {
  const cases = [
    ["valid/filesystem-capability.json", "filesystem-capability", validateFilesystemCapability],
    ["valid/contract-error.json", "contract-error", validateContractError],
    ["valid/package-compatibility.json", "package-compatibility", validatePackageCompatibility],
    ["valid/remote-job-handoff.json", "remote-job-handoff", validateRemoteJobHandoff],
  ];

  for (const [file, kind, validate] of cases) {
    const value = parseAndValidateContract(await fixture(file), kind);
    assert.equal(validate(value), true, file);
  }

  const packageDocument = parseAndValidateContract(
    await fixture("valid/package-compatibility.json"),
    "package-compatibility",
  );
  assert.deepEqual(validatePackageCompatibilitySemantics(packageDocument), []);
  const handoffDocument = parseAndValidateContract(
    await fixture("valid/remote-job-handoff.json"),
    "remote-job-handoff",
  );
  assert.deepEqual(validateRemoteJobHandoffSemantics(handoffDocument), []);
  const errorDocument = parseAndValidateContract(
    await fixture("valid/contract-error.json"),
    "contract-error",
  );
  assert.deepEqual(validateContractErrorSemantics(errorDocument), []);
});

test("contract errors reject path disclosures and control characters without rejecting prose", async () => {
  const valid = parseAndValidateContract(
    await fixture("valid/contract-error.json"),
    "contract-error",
  );
  for (const message of [
    "Use and/or wording; see https://example.invalid/help.",
    "Input / output remains descriptive, and 1/2 is a ratio.",
    "Support is available 24/7 after signing in again.",
    "A operação falhou; tente novamente.",
    "Η λειτουργία απέτυχε· δοκιμάστε ξανά.",
    "تعذر إكمال الطلب؛ حاول مرة أخرى.",
    "请求无法完成，请重试。",
  ]) {
    assert.deepEqual(validateContractErrorSemantics({ ...valid, message }), [], message);
  }

  for (const message of [
    "Unable to read C:/Users/example/source.txt.",
    "Unable to read C:\\Users\\example\\source.txt.",
    "Unable to read \\\\server\\share\\source.txt.",
    "Unable to read \\\\?\\C:\\source.txt.",
    "Unable to read file://host/share/source.txt.",
    "Unable to read /home/example/source.txt.",
    "Unable to read /équipe/source.txt.",
    "Unable to read /用户/源代码.txt.",
    "Unable to read /workspace.",
    "Unable to read //server/share/source.txt.",
    "Unable to read //?/C:/source.txt.",
    "Unable,C:/Users/example/source.txt.",
    "URI:file://host/share/source.txt.",
    "Failure,/var/lib/source.txt.",
  ]) {
    assert.ok(
      validateContractErrorSemantics({ ...valid, message }).some(
        (error) => error.code === "ERROR_MESSAGE_PATH_DISCLOSURE",
      ),
      message,
    );
  }

  for (const message of [
    "Line one\nLine two",
    "Tabbed\tvalue",
    `Delete${String.fromCharCode(0x7f)}character`,
    "Direction\u202eoverride",
    "Isolate\u2066hidden\u2069text",
  ]) {
    assert.ok(
      validateContractErrorSemantics({ ...valid, message }).some(
        (error) => error.code === "ERROR_MESSAGE_CONTROL_CHARACTER",
      ),
    );
  }
});

test("contract-error adversarial fixtures pass structure and fail public semantics", async () => {
  for (const [file, reason] of [
    ["invalid/contract-error-control-character.json", "ERROR_MESSAGE_CONTROL_CHARACTER"],
    ["invalid/contract-error-path-disclosure.json", "ERROR_MESSAGE_PATH_DISCLOSURE"],
    ["invalid/contract-error-details-ref-control-character.json", "ERROR_DETAILS_REF_CONTROL_CHARACTER"],
    ["invalid/contract-error-details-ref-local-path.json", "ERROR_DETAILS_REF_LOCAL_PATH"],
  ]) {
    const value = parseAndValidateContract(await fixture(file), "contract-error");
    assert.ok(
      validateContractErrorSemantics(value).some((error) => error.code === reason),
      file,
    );
  }
});

test("contract-error details references allow opaque schemes and reject local roots", async () => {
  const valid = parseAndValidateContract(
    await fixture("valid/contract-error.json"),
    "contract-error",
  );
  for (const detailsRef of [
    "cas://sha256/ab/cd",
    "azure-blob://error-details/fixture.json",
    "https://support.example.invalid/errors/fixture",
  ]) {
    assert.deepEqual(
      validateContractErrorSemantics({ ...valid, detailsRef }),
      [],
      detailsRef,
    );
  }

  for (const detailsRef of [
    "C:/Users/example/source/error.json",
    "C:\\Users\\example\\source\\error.json",
    "\\\\server\\share\\error.json",
    "//server/share/error.json",
    "//?/C:/error.json",
    "file://host/share/error.json",
    "/var/lib/error.json",
    "/équipe/error.json",
    "/用户/错误.json",
  ]) {
    assert.ok(
      validateContractErrorSemantics({ ...valid, detailsRef }).some(
        (error) => error.code === "ERROR_DETAILS_REF_LOCAL_PATH",
      ),
      detailsRef,
    );
  }

  assert.ok(
    validateContractErrorSemantics({
      ...valid,
      detailsRef: "cas://errors/line\nbreak",
    }).some((error) => error.code === "ERROR_DETAILS_REF_CONTROL_CHARACTER"),
  );
  assert.ok(
    validateContractErrorSemantics({
      ...valid,
      detailsRef: "cas://errors/direction\u202eoverride",
    }).some((error) => error.code === "ERROR_DETAILS_REF_CONTROL_CHARACTER"),
  );
});

test("remote handoff validators reject unknown state and direct-apply claims", async () => {
  for (const file of [
    "invalid/remote-handoff-unknown-state.json",
    "invalid/remote-handoff-direct-apply.json",
  ]) {
    const source = await fixture(file);
    assert.throws(
      () => parseAndValidateContract(source, "remote-job-handoff"),
      (error) => error instanceof ContractValidationError,
      file,
    );
  }
});

test("the published semantic validator binds each handoff version to its predecessor", async () => {
  const current = parseAndValidateContract(
    await fixture("valid/remote-job-handoff.json"),
    "remote-job-handoff",
  );
  const previous = {
    ...structuredClone(current),
    handoffVersion: current.handoffVersion - 1,
    handoffHash: current.previousHandoffHash,
  };
  assert.deepEqual(validateRemoteJobHandoffTransition(previous, current), []);

  const unlinked = {
    ...structuredClone(current),
    previousHandoffHash: "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
  };
  assert.ok(
    validateRemoteJobHandoffTransition(previous, unlinked).some(
      (error) => error.code === "HANDOFF_PREVIOUS_HASH_MISMATCH",
    ),
  );
});
