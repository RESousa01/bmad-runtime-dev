import {
  type BmadHelpApprovedProjection,
  type BmadHelpCancelledProjection,
  type BmadHelpCompletedRecommendationProjection,
  type BmadHelpContextReviewProjection,
  type BmadHelpEvidenceClass,
  type BmadHelpNoRecommendationReason,
  type BmadHelpReceiptSummaryProjection,
  type BmadHelpRunCompletedProjection,
  type BmadHelpTerminalProjection,
  type ModelAuthStatusProjection,
} from "../bmadModelProjection";
import {
  type BmadAgentMenuProjection,
  type BmadAvailability,
  type BmadBlockerCode,
  type BmadHelpActionProjection,
  type BmadHelpRecommendationProjection,
  type BmadHelpRunCreatedProjection,
  type BmadInstalledSkillProjection,
  type BmadLibrarySnapshot,
  type BmadMethodAgentProjection,
  type BmadProjectionSource,
} from "../bmadProjection";
import {
  BMAD_HELP_COMPLETED_SCHEMA,
  BMAD_HELP_RECOMMENDATION_SCHEMA,
  BMAD_HELP_RUN_SCHEMA,
  BMAD_LIBRARY_SCHEMA,
  BMAD_MODEL_RECEIPT_SCHEMA,
  type LatestBmadHelpRunResult,
} from "./contracts";
import { bmadProjectionLimits } from "./bmadProtocolConstants";
import {
  asBmadAvailability,
  asBmadCursor,
  asBmadEntrypointKind,
  asBmadHelpConfidence,
  asBmadHelpIntent,
  asBmadIdentifier,
  asBmadMenuTargetKind,
  asBmadNonemptySafeText,
  asBmadSafeText,
  asBoolean,
  asContractId,
  asModelRegion,
  asNullableBmadBlockerCode,
  asNullableBmadIdentifier,
  asRecord,
  asRelativePath,
  assertExactKeys,
  assertUniqueIdentities,
  assertUniqueRelativePaths,
  asSha256,
  asSingleLineText,
  asTextContent,
  asUnsignedInteger,
  fail,
  parseBmadBlockerCodes,
  utf8Length,
} from "./validation";
import { parseDispatchReply } from "./workspaceProtocol";

export function parseBmadProjectionSource(
  value: unknown,
): BmadProjectionSource {
  const source = asRecord(value);
  assertExactKeys(source, ["sourceKind", "packageName", "packageVersion"]);
  if (source.sourceKind !== "sealed_foundation") {
    return fail();
  }
  return {
    sourceKind: source.sourceKind,
    packageName: asBmadNonemptySafeText(
      source.packageName,
      bmadProjectionLimits.identifierBytes,
    ),
    packageVersion: asBmadNonemptySafeText(
      source.packageVersion,
      bmadProjectionLimits.identifierBytes,
    ),
  };
}

export function assertBmadAvailabilityBlockers(
  availability: BmadAvailability,
  blockerCodes: readonly BmadBlockerCode[],
): void {
  const expected =
    availability === "available"
      ? []
      : [
          (
            {
              capability_disabled: "bmad_capability_disabled",
              dependency_unavailable: "bmad_dependency_unavailable",
              orphan_skill: "bmad_help_catalog_orphan",
              network_unavailable: "bmad_network_reference_unavailable",
              source_prompt_unavailable: "bmad_source_prompt_unavailable",
            } as const
          )[availability],
        ];
  if (
    blockerCodes.length !== expected.length ||
    blockerCodes.some((code, index) => code !== expected[index])
  ) {
    fail();
  }
}

export function parseBmadHelpRecommendation(
  value: unknown,
): BmadHelpRecommendationProjection {
  const recommendation = asRecord(value);
  assertExactKeys(recommendation, [
    "schemaVersion",
    "displayName",
    "moduleCode",
    "skillName",
    "action",
    "confidence",
    "source",
    "reason",
    "requiredGuidance",
    "expectedArtifacts",
    "availability",
    "blockerCodes",
    "completionClaimed",
  ]);
  if (
    recommendation.schemaVersion !== BMAD_HELP_RECOMMENDATION_SCHEMA ||
    recommendation.completionClaimed !== false ||
    !Array.isArray(recommendation.expectedArtifacts) ||
    recommendation.expectedArtifacts.length >
      bmadProjectionLimits.expectedArtifacts
  ) {
    return fail();
  }
  const availability = asBmadAvailability(recommendation.availability);
  const blockerCodes = parseBmadBlockerCodes(recommendation.blockerCodes);
  assertBmadAvailabilityBlockers(availability, blockerCodes);
  return {
    schemaVersion: BMAD_HELP_RECOMMENDATION_SCHEMA,
    displayName: asBmadNonemptySafeText(
      recommendation.displayName,
      bmadProjectionLimits.identifierBytes,
    ),
    moduleCode: asBmadIdentifier(recommendation.moduleCode),
    skillName: asBmadIdentifier(recommendation.skillName),
    action: asNullableBmadIdentifier(recommendation.action),
    confidence: asBmadHelpConfidence(recommendation.confidence),
    source: parseBmadProjectionSource(recommendation.source),
    reason: asBmadNonemptySafeText(
      recommendation.reason,
      bmadProjectionLimits.helpReasonBytes,
    ),
    requiredGuidance: asBoolean(recommendation.requiredGuidance),
    expectedArtifacts: recommendation.expectedArtifacts.map((artifact) =>
      asBmadNonemptySafeText(artifact, bmadProjectionLimits.identifierBytes),
    ),
    availability,
    blockerCodes,
    completionClaimed: false,
  };
}

export function parseBmadHelpRunCreated(
  value: unknown,
  expectedWorkspaceId: string,
): BmadHelpRunCreatedProjection {
  const run = asRecord(value);
  assertExactKeys(run, [
    "schemaVersion",
    "runKind",
    "lifecycle",
    "workspaceId",
    "runId",
    "sessionId",
    "currentIntent",
    "runnable",
    "completionClaimed",
    "recommendation",
  ]);
  if (
    run.schemaVersion !== BMAD_HELP_RUN_SCHEMA ||
    run.runKind !== "bmad_help" ||
    run.lifecycle !== "created_unbound" ||
    run.runnable !== false ||
    run.completionClaimed !== false
  ) {
    return fail();
  }
  const workspaceId = asContractId(run.workspaceId);
  if (workspaceId !== expectedWorkspaceId) {
    return fail();
  }
  const projection: BmadHelpRunCreatedProjection = {
    schemaVersion: BMAD_HELP_RUN_SCHEMA,
    runKind: "bmad_help",
    lifecycle: "created_unbound",
    workspaceId,
    runId: asContractId(run.runId),
    sessionId: asContractId(run.sessionId),
    currentIntent: asBmadHelpIntent(run.currentIntent),
    runnable: false,
    completionClaimed: false,
    recommendation: parseBmadHelpRecommendation(run.recommendation),
  };
  if (
    utf8Length(JSON.stringify(projection)) >
    bmadProjectionLimits.helpRunResponseBytes
  ) {
    return fail();
  }
  return projection;
}

export function asPositiveSafeInteger(value: unknown): number {
  const parsed = asUnsignedInteger(value);
  if (parsed === 0) return fail();
  return parsed;
}

export function asBmadRendererText(
  value: unknown,
  maximumBytes: number,
): string {
  const text = asBmadNonemptySafeText(value, maximumBytes);
  if (/(?:\\\\|[A-Za-z]:[\\/]|file:\/\/|(?:^|\s)\/(?:[^\s]|$))/iu.test(text)) {
    return fail();
  }
  return text;
}

export function parseModelAuthStatus(
  value: unknown,
): ModelAuthStatusProjection {
  const status = asRecord(value);
  const developmentOnly = asBoolean(status.developmentOnly);
  assertExactKeys(status, [
    "status",
    "mode",
    "authEpoch",
    "developmentOnly",
    "destinationLabel",
    "signInAvailable",
    "signOutAvailable",
  ]);
  if (
    (status.status !== "unavailable" &&
      status.status !== "development_ready") ||
    (status.mode !== "offline" &&
      status.mode !== "deterministic_development") ||
    status.signInAvailable !== false ||
    status.signOutAvailable !== true ||
    (status.status === "development_ready"
      ? status.mode !== "deterministic_development" || developmentOnly !== true
      : status.mode !== "offline" || developmentOnly !== false)
  ) {
    return fail();
  }
  return {
    status: status.status,
    mode: status.mode,
    authEpoch: asUnsignedInteger(status.authEpoch),
    developmentOnly,
    destinationLabel: asBmadRendererText(status.destinationLabel, 256),
    signInAvailable: false,
    signOutAvailable: true,
  };
}

export function parseBmadHelpReview(
  value: unknown,
  expectedWorkspaceId: string,
): BmadHelpContextReviewProjection {
  const review = asRecord(value);
  assertExactKeys(review, [
    "workspaceId",
    "workspaceGrantEpoch",
    "runId",
    "sessionId",
    "destinationLabel",
    "developmentOnly",
    "consentDisclosure",
    "manifestHash",
    "purpose",
    "region",
    "retentionMode",
    "expiresAt",
    "items",
    "exclusions",
    "secretFindings",
    "totalOutboundBytes",
    "totalTokenEstimate",
    "redactionLimitation",
  ]);
  if (
    review.workspaceId !== expectedWorkspaceId ||
    typeof review.developmentOnly !== "boolean" ||
    review.retentionMode !== "transient_no_store" ||
    !Array.isArray(review.items) ||
    review.items.length === 0 ||
    review.items.length > bmadProjectionLimits.reviewItems ||
    !Array.isArray(review.exclusions) ||
    review.exclusions.length > bmadProjectionLimits.reviewExclusions ||
    !Array.isArray(review.secretFindings) ||
    review.secretFindings.length > bmadProjectionLimits.reviewSecretFindings
  ) {
    return fail();
  }
  const items = review.items.map((value) => {
    const item = asRecord(value);
    assertExactKeys(item, [
      "relativeLabel",
      "semanticRole",
      "language",
      "outboundByteCount",
      "tokenEstimate",
      "classification",
      "redactions",
      "outboundContent",
    ]);
    if (
      !["public", "internal", "confidential"].includes(
        String(item.classification),
      ) ||
      !Array.isArray(item.redactions) ||
      item.redactions.length > 32
    ) {
      return fail();
    }
    const outboundContent = asTextContent(
      item.outboundContent,
      bmadProjectionLimits.reviewTextBytes,
    );
    const outboundByteCount = asPositiveSafeInteger(item.outboundByteCount);
    if (outboundByteCount !== utf8Length(outboundContent)) return fail();
    return {
      relativeLabel: asRelativePath(item.relativeLabel),
      semanticRole: asBmadIdentifier(item.semanticRole),
      language: item.language === null ? null : asBmadIdentifier(item.language),
      outboundByteCount,
      tokenEstimate: asPositiveSafeInteger(item.tokenEstimate),
      classification: item.classification as
        "public" | "internal" | "confidential",
      redactions: item.redactions.map((value) => {
        const redaction = asRecord(value);
        assertExactKeys(redaction, ["kind", "occurrenceCount"]);
        return {
          kind: asBmadIdentifier(redaction.kind),
          occurrenceCount: asPositiveSafeInteger(redaction.occurrenceCount),
        };
      }),
      outboundContent,
    };
  });
  assertUniqueRelativePaths(items.map(({ relativeLabel }) => relativeLabel));
  const exclusions = review.exclusions.map((value) => {
    const exclusion = asRecord(value);
    assertExactKeys(exclusion, ["relativeLabel", "reason"]);
    return {
      relativeLabel: asRelativePath(exclusion.relativeLabel),
      reason: asBmadRendererText(exclusion.reason, 1_024),
    };
  });
  const secretFindings = review.secretFindings.map((value) => {
    const finding = asRecord(value);
    assertExactKeys(finding, ["relativeLabel", "kind", "occurrenceCount"]);
    return {
      relativeLabel: asRelativePath(finding.relativeLabel),
      kind: asBmadIdentifier(finding.kind),
      occurrenceCount: asPositiveSafeInteger(finding.occurrenceCount),
    };
  });
  const totalOutboundBytes = asPositiveSafeInteger(review.totalOutboundBytes);
  const totalTokenEstimate = asPositiveSafeInteger(review.totalTokenEstimate);
  if (
    items.reduce((total, item) => total + item.outboundByteCount, 0) !==
      totalOutboundBytes ||
    items.reduce((total, item) => total + item.tokenEstimate, 0) !==
      totalTokenEstimate ||
    totalOutboundBytes > bmadProjectionLimits.reviewTextBytes
  ) {
    return fail();
  }
  const projection: BmadHelpContextReviewProjection = {
    workspaceId: asContractId(review.workspaceId),
    workspaceGrantEpoch: asPositiveSafeInteger(review.workspaceGrantEpoch),
    runId: asContractId(review.runId),
    sessionId: asContractId(review.sessionId),
    destinationLabel: asBmadRendererText(review.destinationLabel, 256),
    developmentOnly: review.developmentOnly,
    consentDisclosure: asBmadRendererText(review.consentDisclosure, 4_096),
    manifestHash: asSha256(review.manifestHash),
    purpose: asBmadIdentifier(review.purpose),
    region: asModelRegion(review.region),
    retentionMode: "transient_no_store",
    expiresAt: asPositiveSafeInteger(review.expiresAt),
    items,
    exclusions,
    secretFindings,
    totalOutboundBytes,
    totalTokenEstimate,
    redactionLimitation: asBmadRendererText(review.redactionLimitation, 1_024),
  };
  if (
    utf8Length(JSON.stringify(projection)) >
    bmadProjectionLimits.reviewProjectionBytes
  )
    return fail();
  return projection;
}

export function parseBmadHelpApproval(
  value: unknown,
): BmadHelpApprovedProjection {
  const approval = asRecord(value);
  assertExactKeys(approval, [
    "manifestHash",
    "decisionId",
    "expiresAt",
    "sendEligible",
  ]);
  if (approval.sendEligible !== true) return fail();
  return {
    manifestHash: asSha256(approval.manifestHash),
    decisionId: asContractId(approval.decisionId),
    expiresAt: asPositiveSafeInteger(approval.expiresAt),
    sendEligible: true,
  };
}

export function parseBmadHelpCancellation(
  value: unknown,
): BmadHelpCancelledProjection {
  const cancellation = asRecord(value);
  assertExactKeys(cancellation, ["manifestHash", "decisionId"]);
  return {
    manifestHash: asSha256(cancellation.manifestHash),
    decisionId: asContractId(cancellation.decisionId),
  };
}

export const bmadTerminalReasons = new Set<
  BmadHelpTerminalProjection["reason"]
>(["cancelled", "consent_expired", "consent_consumed", "failed"]);

export function parseBmadHelpTerminal(
  value: unknown,
  expectedWorkspaceId: string,
): BmadHelpTerminalProjection {
  const terminal = asRecord(value);
  assertExactKeys(terminal, [
    "workspaceId",
    "reason",
    "resumable",
    "sendEligible",
  ]);
  const workspaceId = asContractId(terminal.workspaceId);
  const reason = asBmadIdentifier(
    terminal.reason,
  ) as BmadHelpTerminalProjection["reason"];
  if (
    workspaceId !== expectedWorkspaceId ||
    !bmadTerminalReasons.has(reason) ||
    terminal.resumable !== false ||
    terminal.sendEligible !== false
  )
    return fail();
  return { workspaceId, reason, resumable: false, sendEligible: false };
}

export const evidenceClasses = new Set<BmadHelpEvidenceClass>([
  "authoritative",
  "user_asserted",
  "heuristic",
  "contextual",
]);

export const noRecommendationReasons = new Set<BmadHelpNoRecommendationReason>([
  "catalog_evidence_absent",
  "completion_evidence_ambiguous",
  "dependency_unavailable",
]);

export function parseBmadCompletedRecommendation(
  value: unknown,
): BmadHelpCompletedRecommendationProjection {
  const recommendation = asRecord(value);
  if (recommendation.recommendationKind === "recommended_capability") {
    assertExactKeys(recommendation, [
      "recommendationKind",
      "displayName",
      "moduleCode",
      "skillName",
      "action",
      "evidenceClass",
      "guidanceRequired",
      "rationaleSummary",
      "createdAt",
    ]);
    const evidenceClass = asBmadIdentifier(
      recommendation.evidenceClass,
    ) as BmadHelpEvidenceClass;
    if (!evidenceClasses.has(evidenceClass)) return fail();
    return {
      recommendationKind: "recommended_capability",
      displayName: asBmadRendererText(recommendation.displayName, 256),
      moduleCode: asBmadIdentifier(recommendation.moduleCode),
      skillName: asBmadIdentifier(recommendation.skillName),
      action: asNullableBmadIdentifier(recommendation.action),
      evidenceClass,
      guidanceRequired: asBoolean(recommendation.guidanceRequired),
      rationaleSummary: asBmadRendererText(
        recommendation.rationaleSummary,
        4_096,
      ),
      createdAt: asPositiveSafeInteger(recommendation.createdAt),
    };
  }
  if (recommendation.recommendationKind !== "no_recommendation") return fail();
  assertExactKeys(recommendation, [
    "recommendationKind",
    "reasonCode",
    "createdAt",
  ]);
  const reasonCode = asBmadIdentifier(
    recommendation.reasonCode,
  ) as BmadHelpNoRecommendationReason;
  if (!noRecommendationReasons.has(reasonCode)) return fail();
  return {
    recommendationKind: "no_recommendation",
    reasonCode,
    createdAt: asPositiveSafeInteger(recommendation.createdAt),
  };
}

export function parseBmadReceipt(
  value: unknown,
): BmadHelpReceiptSummaryProjection {
  const receipt = asRecord(value);
  assertExactKeys(receipt, [
    "schemaVersion",
    "receiptId",
    "status",
    "retentionMode",
    "region",
    "inputBytes",
    "outputBytes",
    "startedAt",
    "completedAt",
  ]);
  if (
    receipt.schemaVersion !== BMAD_MODEL_RECEIPT_SCHEMA ||
    receipt.status !== "succeeded" ||
    receipt.retentionMode !== "transient_no_store"
  )
    return fail();
  const inputBytes = asPositiveSafeInteger(receipt.inputBytes);
  const outputBytes = asPositiveSafeInteger(receipt.outputBytes);
  const startedAt = asPositiveSafeInteger(receipt.startedAt);
  const completedAt = asPositiveSafeInteger(receipt.completedAt);
  if (
    inputBytes > bmadProjectionLimits.receiptInputBytes ||
    outputBytes > bmadProjectionLimits.receiptOutputBytes ||
    startedAt > completedAt
  )
    return fail();
  return {
    schemaVersion: BMAD_MODEL_RECEIPT_SCHEMA,
    receiptId: asContractId(receipt.receiptId),
    status: "succeeded",
    retentionMode: "transient_no_store",
    region: asModelRegion(receipt.region),
    inputBytes,
    outputBytes,
    startedAt,
    completedAt,
  };
}

export function parseBmadHelpCompleted(
  value: unknown,
  expectedWorkspaceId: string,
): BmadHelpRunCompletedProjection {
  const result = asRecord(value);
  assertExactKeys(result, [
    "schemaVersion",
    "runKind",
    "lifecycle",
    "workspaceId",
    "runId",
    "sessionId",
    "runnable",
    "completionClaimed",
    "recommendation",
    "receipt",
  ]);
  if (
    result.schemaVersion !== BMAD_HELP_COMPLETED_SCHEMA ||
    result.runKind !== "bmad_help" ||
    result.lifecycle !== "completed" ||
    result.workspaceId !== expectedWorkspaceId ||
    result.runnable !== false ||
    result.completionClaimed !== true
  )
    return fail();
  const recommendation = parseBmadCompletedRecommendation(
    result.recommendation,
  );
  const receipt = parseBmadReceipt(result.receipt);
  if (recommendation.createdAt < receipt.completedAt) return fail();
  const projection: BmadHelpRunCompletedProjection = {
    schemaVersion: BMAD_HELP_COMPLETED_SCHEMA,
    runKind: "bmad_help",
    lifecycle: "completed",
    workspaceId: asContractId(result.workspaceId),
    runId: asContractId(result.runId),
    sessionId: asContractId(result.sessionId),
    runnable: false,
    completionClaimed: true,
    recommendation,
    receipt,
  };
  if (
    utf8Length(JSON.stringify(projection)) >
    bmadProjectionLimits.modelResponseBytes
  )
    return fail();
  return projection;
}

export function parseBmadInstalledSkill(
  value: unknown,
): BmadInstalledSkillProjection {
  const skill = asRecord(value);
  assertExactKeys(skill, [
    "moduleCode",
    "skillName",
    "displayName",
    "description",
    "actions",
    "entrypointKind",
    "distributionProfile",
    "installProfile",
    "validationProfile",
    "availability",
    "blockerCodes",
    "hiddenFromHelp",
  ]);
  if (
    !Array.isArray(skill.actions) ||
    skill.actions.length > bmadProjectionLimits.actionsPerSkill
  ) {
    return fail();
  }
  return {
    moduleCode: asBmadIdentifier(skill.moduleCode),
    skillName: asBmadIdentifier(skill.skillName),
    displayName: asBmadSafeText(
      skill.displayName,
      bmadProjectionLimits.identifierBytes,
    ),
    description: asBmadSafeText(
      skill.description,
      bmadProjectionLimits.descriptionBytes,
    ),
    actions: skill.actions.map(asBmadIdentifier),
    entrypointKind: asBmadEntrypointKind(skill.entrypointKind),
    distributionProfile: asBmadIdentifier(skill.distributionProfile),
    installProfile: asBmadIdentifier(skill.installProfile),
    validationProfile: asBmadIdentifier(skill.validationProfile),
    availability: asBmadAvailability(skill.availability),
    blockerCodes: parseBmadBlockerCodes(skill.blockerCodes),
    hiddenFromHelp: asBoolean(skill.hiddenFromHelp),
  };
}

export function parseBmadHelpAction(value: unknown): BmadHelpActionProjection {
  const action = asRecord(value);
  assertExactKeys(action, [
    "moduleCode",
    "skillName",
    "action",
    "displayName",
    "menuCode",
    "description",
    "requiredGuidance",
    "expectedArtifacts",
    "availability",
    "blockerCodes",
  ]);
  if (
    !Array.isArray(action.expectedArtifacts) ||
    action.expectedArtifacts.length > bmadProjectionLimits.expectedArtifacts
  ) {
    return fail();
  }
  return {
    moduleCode: asBmadIdentifier(action.moduleCode),
    skillName: asBmadIdentifier(action.skillName),
    action: asNullableBmadIdentifier(action.action),
    displayName: asBmadSafeText(
      action.displayName,
      bmadProjectionLimits.identifierBytes,
    ),
    menuCode: asNullableBmadIdentifier(action.menuCode),
    description: asBmadSafeText(
      action.description,
      bmadProjectionLimits.descriptionBytes,
    ),
    requiredGuidance: asBoolean(action.requiredGuidance),
    expectedArtifacts: action.expectedArtifacts.map((artifact) =>
      asBmadSafeText(artifact, bmadProjectionLimits.identifierBytes),
    ),
    availability: asBmadAvailability(action.availability),
    blockerCodes: parseBmadBlockerCodes(action.blockerCodes),
  };
}

export function parseBmadAgentMenu(value: unknown): BmadAgentMenuProjection {
  const menu = asRecord(value);
  assertExactKeys(menu, [
    "code",
    "description",
    "targetKind",
    "displayLabel",
    "availability",
    "availabilityReason",
  ]);
  return {
    code: asBmadIdentifier(menu.code),
    description: asBmadSafeText(
      menu.description,
      bmadProjectionLimits.descriptionBytes,
    ),
    targetKind: asBmadMenuTargetKind(menu.targetKind),
    displayLabel: asBmadSafeText(
      menu.displayLabel,
      bmadProjectionLimits.identifierBytes,
    ),
    availability: asBmadAvailability(menu.availability),
    availabilityReason: asNullableBmadBlockerCode(menu.availabilityReason),
  };
}

export function parseBmadMethodAgent(
  value: unknown,
): BmadMethodAgentProjection {
  const agent = asRecord(value);
  assertExactKeys(agent, [
    "moduleCode",
    "agentCode",
    "name",
    "title",
    "icon",
    "team",
    "description",
    "availability",
    "blockerCodes",
    "menus",
  ]);
  if (
    !Array.isArray(agent.menus) ||
    agent.menus.length > bmadProjectionLimits.menusPerAgent
  ) {
    return fail();
  }
  const menus = agent.menus.map(parseBmadAgentMenu);
  assertUniqueIdentities(menus.map(({ code }) => code));
  return {
    moduleCode: asBmadIdentifier(agent.moduleCode),
    agentCode: asBmadIdentifier(agent.agentCode),
    name: asBmadSafeText(agent.name, bmadProjectionLimits.identifierBytes),
    title: asBmadSafeText(agent.title, bmadProjectionLimits.identifierBytes),
    icon: asBmadSafeText(agent.icon, bmadProjectionLimits.iconBytes),
    team: asBmadIdentifier(agent.team),
    description: asBmadSafeText(
      agent.description,
      bmadProjectionLimits.descriptionBytes,
    ),
    availability: asBmadAvailability(agent.availability),
    blockerCodes: parseBmadBlockerCodes(agent.blockerCodes),
    menus,
  };
}

function parseBmadBuilderPackage(value: unknown) {
  const builder = asRecord(value);
  assertExactKeys(builder, [
    "packageName",
    "packageVersion",
    "packageKind",
    "displayName",
    "activationState",
    "resourceCount",
    "descriptorDigest",
    "blockerCodes",
  ]);
  if (
    (builder.packageKind !== "agent" && builder.packageKind !== "workflow") ||
    builder.activationState !== "installed_inactive" ||
    !Array.isArray(builder.blockerCodes) ||
    builder.blockerCodes.length !== 1 ||
    builder.blockerCodes[0] !== "builder_engine_gated"
  ) {
    return fail();
  }
  const descriptorDigest = asSingleLineText(builder.descriptorDigest, 16);
  if (!/^[0-9a-f]{12}$/.test(descriptorDigest)) {
    return fail();
  }
  return {
    packageName: asSingleLineText(builder.packageName, 128),
    packageVersion: asSingleLineText(builder.packageVersion, 64),
    packageKind: builder.packageKind,
    displayName: asSingleLineText(builder.displayName, 128),
    activationState: "installed_inactive",
    resourceCount: asUnsignedInteger(builder.resourceCount),
    descriptorDigest,
    blockerCodes: ["builder_engine_gated"],
  } as const;
}

export function parseBmadLibrarySnapshot(value: unknown): BmadLibrarySnapshot {
  const snapshot = asRecord(value);
  assertExactKeys(snapshot, [
    "schemaVersion",
    "scope",
    "source",
    "installedSkills",
    "helpActions",
    "methodAgents",
    "builderPackages",
    "nextCursor",
  ]);
  if (
    snapshot.schemaVersion !== BMAD_LIBRARY_SCHEMA ||
    snapshot.scope !== "installed_method" ||
    !Array.isArray(snapshot.installedSkills) ||
    snapshot.installedSkills.length > bmadProjectionLimits.installedSkills ||
    !Array.isArray(snapshot.helpActions) ||
    snapshot.helpActions.length > bmadProjectionLimits.helpActions ||
    !Array.isArray(snapshot.methodAgents) ||
    snapshot.methodAgents.length > bmadProjectionLimits.methodAgents ||
    !Array.isArray(snapshot.builderPackages) ||
    snapshot.builderPackages.length > 8
  ) {
    return fail();
  }
  const installedSkills = snapshot.installedSkills.map(parseBmadInstalledSkill);
  const helpActions = snapshot.helpActions.map(parseBmadHelpAction);
  const methodAgents = snapshot.methodAgents.map(parseBmadMethodAgent);
  const builderPackages = snapshot.builderPackages.map(parseBmadBuilderPackage);
  assertUniqueIdentities(
    builderPackages.map(({ packageKind, packageName }) => `${packageName}\u001f${packageKind}`),
  );
  assertUniqueIdentities(
    installedSkills.map(
      ({ moduleCode, skillName }) => `${moduleCode}\u001f${skillName}`,
    ),
  );
  assertUniqueIdentities(
    helpActions.map(
      ({ moduleCode, skillName, action }) =>
        `${moduleCode}\u001f${skillName}\u001f${action ?? "\u0000"}`,
    ),
  );
  assertUniqueIdentities(
    helpActions.flatMap(({ menuCode, moduleCode }) =>
      menuCode === null ? [] : [`${moduleCode}\u001f${menuCode}`],
    ),
  );
  assertUniqueIdentities(
    methodAgents.map(
      ({ moduleCode, agentCode }) => `${moduleCode}\u001f${agentCode}`,
    ),
  );
  const projection: BmadLibrarySnapshot = {
    schemaVersion: BMAD_LIBRARY_SCHEMA,
    scope: "installed_method",
    source: parseBmadProjectionSource(snapshot.source),
    installedSkills,
    helpActions,
    methodAgents,
    builderPackages,
    nextCursor: asBmadCursor(snapshot.nextCursor),
  };
  if (
    utf8Length(JSON.stringify(projection)) > bmadProjectionLimits.responseBytes
  ) {
    return fail();
  }
  return projection;
}

export function parseBmadLibrarySnapshotReply(
  value: unknown,
  requestId: string,
): { projection: BmadLibrarySnapshot; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "bmad_library_snapshot") {
    return fail();
  }
  return {
    projection: parseBmadLibrarySnapshot(data.value),
    sequence: parsed.sequence,
  };
}

export function parseBmadHelpRunCreatedReply(
  value: unknown,
  requestId: string,
  workspaceId: string,
): { projection: BmadHelpRunCreatedProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  const data = parsed.data;
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "bmad_help_run_created") {
    return fail();
  }
  const projection = parseBmadHelpRunCreated(data.value, workspaceId);
  if (parsed.receipt.operationId !== projection.runId) {
    return fail();
  }
  return { projection, sequence: parsed.sequence };
}

export function parseModelAuthStatusReply(
  value: unknown,
  requestId: string,
): { projection: ModelAuthStatusProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "model_auth_status") return fail();
  return {
    projection: parseModelAuthStatus(parsed.data.value),
    sequence: parsed.sequence,
  };
}

export function parseBmadHelpReviewReply(
  value: unknown,
  requestId: string,
  workspaceId: string,
): { projection: BmadHelpContextReviewProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "bmad_help_review") return fail();
  return {
    projection: parseBmadHelpReview(parsed.data.value, workspaceId),
    sequence: parsed.sequence,
  };
}

export function parseBmadHelpApprovedReply(
  value: unknown,
  requestId: string,
): { projection: BmadHelpApprovedProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "bmad_help_approved") return fail();
  return {
    projection: parseBmadHelpApproval(parsed.data.value),
    sequence: parsed.sequence,
  };
}

export function parseBmadHelpCancelledReply(
  value: unknown,
  requestId: string,
): { projection: BmadHelpCancelledProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "bmad_help_cancelled") return fail();
  return {
    projection: parseBmadHelpCancellation(parsed.data.value),
    sequence: parsed.sequence,
  };
}

export function parseBmadHelpCompletedReply(
  value: unknown,
  requestId: string,
  workspaceId: string,
): { projection: BmadHelpRunCompletedProjection; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) return fail();
  assertExactKeys(parsed.data, ["kind", "value"]);
  if (parsed.data.kind !== "bmad_help_run_completed") return fail();
  return {
    projection: parseBmadHelpCompleted(parsed.data.value, workspaceId),
    sequence: parsed.sequence,
  };
}

export function parseLatestBmadHelpRunReply(
  value: unknown,
  requestId: string,
  workspaceId: string,
  workspaceGrantEpoch: number,
): { result: LatestBmadHelpRunResult; sequence: number } {
  const parsed = parseDispatchReply(value, requestId);
  if (parsed.receipt.operationId !== null) {
    return fail();
  }
  const data = parsed.data;
  if (data.kind === "no_bmad_help_run") {
    assertExactKeys(data, ["kind"]);
    return { result: { kind: "no_run" }, sequence: parsed.sequence };
  }
  if (data.kind === "bmad_help_projection_unavailable") {
    assertExactKeys(data, ["kind"]);
    return {
      result: { kind: "projection_unavailable" },
      sequence: parsed.sequence,
    };
  }
  if (data.kind === "bmad_help_review") {
    assertExactKeys(data, ["kind", "value"]);
    const review = parseBmadHelpReview(data.value, workspaceId);
    if (review.workspaceGrantEpoch !== workspaceGrantEpoch) return fail();
    return { result: { kind: "review", review }, sequence: parsed.sequence };
  }
  if (data.kind === "bmad_help_approved_lifecycle") {
    assertExactKeys(data, ["kind", "value"]);
    const lifecycle = asRecord(data.value);
    assertExactKeys(lifecycle, ["review", "approval"]);
    const review = parseBmadHelpReview(lifecycle.review, workspaceId);
    const approval = parseBmadHelpApproval(lifecycle.approval);
    if (
      review.workspaceGrantEpoch !== workspaceGrantEpoch ||
      approval.manifestHash !== review.manifestHash ||
      approval.expiresAt > review.expiresAt
    )
      return fail();
    return {
      result: { kind: "approved", review, approval },
      sequence: parsed.sequence,
    };
  }
  if (data.kind === "bmad_help_terminal") {
    assertExactKeys(data, ["kind", "value"]);
    return {
      result: {
        kind: "terminal",
        terminal: parseBmadHelpTerminal(data.value, workspaceId),
      },
      sequence: parsed.sequence,
    };
  }
  if (data.kind === "bmad_help_run_interrupted") {
    assertExactKeys(data, ["kind", "value"]);
    return {
      result: {
        kind: "interrupted",
        run: parseBmadHelpRunCreated(data.value, workspaceId),
      },
      sequence: parsed.sequence,
    };
  }
  if (data.kind === "bmad_help_run_completed") {
    assertExactKeys(data, ["kind", "value"]);
    return {
      result: {
        kind: "completed",
        result: parseBmadHelpCompleted(data.value, workspaceId),
      },
      sequence: parsed.sequence,
    };
  }
  assertExactKeys(data, ["kind", "value"]);
  if (data.kind !== "bmad_help_run_created") {
    return fail();
  }
  return {
    result: {
      kind: "retained",
      run: parseBmadHelpRunCreated(data.value, workspaceId),
    },
    sequence: parsed.sequence,
  };
}
