import type {
  BmadAvailability,
  BmadBlockerCode,
  BmadEntrypointKind,
  BmadHelpConfidence,
  BmadMenuTargetKind,
} from "../bmadProjection";

export const bmadProjectionLimits = {
  responseBytes: 256 * 1024,
  installedSkills: 64,
  helpActions: 64,
  methodAgents: 16,
  menusPerAgent: 32,
  actionsPerSkill: 16,
  expectedArtifacts: 16,
  identifierBytes: 256,
  descriptionBytes: 2_048,
  iconBytes: 64,
  cursorBytes: 256,
  helpIntentBytes: 4_096,
  helpReasonBytes: 4_096,
  helpRunResponseBytes: 64 * 1_024 + 1_024,
  modelResponseBytes: 5 * 1_024 * 1_024,
  reviewItems: 16,
  reviewExclusions: 32,
  reviewSecretFindings: 64,
  reviewTextBytes: 64 * 1024,
  reviewProjectionBytes: 96 * 1024,
  reviewLabelBytes: 1_024,
  receiptInputBytes: 4 * 1024 * 1024,
  receiptOutputBytes: 1024 * 1024,
} as const;

export const bmadAvailabilities = new Set<BmadAvailability>([
  "available",
  "capability_disabled",
  "dependency_unavailable",
  "orphan_skill",
  "network_unavailable",
  "source_prompt_unavailable",
]);

export const bmadBlockerCodes = new Set<BmadBlockerCode>([
  "bmad_capability_disabled",
  "bmad_dependency_unavailable",
  "bmad_help_catalog_orphan",
  "bmad_network_reference_unavailable",
  "bmad_source_prompt_unavailable",
]);

export const bmadEntrypointKinds = new Set<BmadEntrypointKind>([
  "direct",
  "inline",
  "step_jit",
  "script_rendered",
  "compatibility_shim",
]);

export const bmadMenuTargetKinds = new Set<BmadMenuTargetKind>([
  "skill_target",
  "prompt_reference",
]);

export const bmadHelpConfidences = new Set<BmadHelpConfidence>([
  "authoritative",
  "user_asserted",
  "heuristic",
  "contextual",
  "unknown",
]);
