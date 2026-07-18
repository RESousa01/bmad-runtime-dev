export type BmadAvailability =
  | "available"
  | "capability_disabled"
  | "dependency_unavailable"
  | "orphan_skill"
  | "network_unavailable"
  | "source_prompt_unavailable";

export type BmadBlockerCode =
  | "bmad_capability_disabled"
  | "bmad_dependency_unavailable"
  | "bmad_help_catalog_orphan"
  | "bmad_network_reference_unavailable"
  | "bmad_source_prompt_unavailable";

export type BmadEntrypointKind =
  | "direct"
  | "inline"
  | "step_jit"
  | "script_rendered"
  | "compatibility_shim";

export type BmadMenuTargetKind = "skill_target" | "prompt_reference";

export type BmadHelpConfidence =
  | "authoritative"
  | "user_asserted"
  | "heuristic"
  | "contextual"
  | "unknown";

export interface BmadProjectionSource {
  readonly sourceKind: "sealed_foundation";
  readonly packageName: string;
  readonly packageVersion: string;
}

export interface BmadInstalledSkillProjection {
  readonly moduleCode: string;
  readonly skillName: string;
  readonly displayName: string;
  readonly description: string;
  readonly actions: readonly string[];
  readonly entrypointKind: BmadEntrypointKind;
  readonly distributionProfile: string;
  readonly installProfile: string;
  readonly validationProfile: string;
  readonly availability: BmadAvailability;
  readonly blockerCodes: readonly BmadBlockerCode[];
  readonly hiddenFromHelp: boolean;
}

export interface BmadHelpActionProjection {
  readonly moduleCode: string;
  readonly skillName: string;
  readonly action: string | null;
  readonly displayName: string;
  readonly menuCode: string | null;
  readonly description: string;
  readonly requiredGuidance: boolean;
  readonly expectedArtifacts: readonly string[];
  readonly availability: BmadAvailability;
  readonly blockerCodes: readonly BmadBlockerCode[];
}

export interface BmadAgentMenuProjection {
  readonly code: string;
  readonly description: string;
  readonly targetKind: BmadMenuTargetKind;
  readonly displayLabel: string;
  readonly availability: BmadAvailability;
  readonly availabilityReason: BmadBlockerCode | null;
}

export interface BmadMethodAgentProjection {
  readonly moduleCode: string;
  readonly agentCode: string;
  readonly name: string;
  readonly title: string;
  readonly icon: string;
  readonly team: string;
  readonly description: string;
  readonly availability: BmadAvailability;
  readonly blockerCodes: readonly BmadBlockerCode[];
  readonly menus: readonly BmadAgentMenuProjection[];
}

export type BmadBuilderPackageKind = "agent" | "workflow";

export interface BmadBuilderPackageProjection {
  readonly packageName: string;
  readonly packageVersion: string;
  readonly packageKind: BmadBuilderPackageKind;
  readonly displayName: string;
  readonly activationState: "installed_inactive";
  readonly resourceCount: number;
  readonly descriptorDigest: string;
  readonly blockerCodes: readonly ["builder_engine_gated"];
}

export interface BmadLibraryProjection {
  readonly schemaVersion: "bmad-library-snapshot.v2";
  readonly scope: "installed_method";
  readonly source: BmadProjectionSource;
  readonly installedSkills: readonly BmadInstalledSkillProjection[];
  readonly helpActions: readonly BmadHelpActionProjection[];
  readonly methodAgents: readonly BmadMethodAgentProjection[];
  readonly builderPackages: readonly BmadBuilderPackageProjection[];
  readonly nextCursor: string | null;
}

export type BmadLibrarySnapshot = BmadLibraryProjection;

export type BmadLibraryUiState =
  | { readonly kind: "idle" }
  | { readonly kind: "loading" }
  | { readonly kind: "ready"; readonly projection: BmadLibraryProjection }
  | {
    readonly kind: "unavailable";
    readonly message: string;
    readonly retryable: boolean;
  };

export interface BmadHelpRecommendationProjection {
  readonly schemaVersion: "bmad-help-recommendation.v1";
  readonly displayName: string;
  readonly moduleCode: string;
  readonly skillName: string;
  readonly action: string | null;
  readonly confidence: BmadHelpConfidence;
  readonly source: BmadProjectionSource;
  readonly reason: string;
  readonly requiredGuidance: boolean;
  readonly expectedArtifacts: readonly string[];
  readonly availability: BmadAvailability;
  readonly blockerCodes: readonly BmadBlockerCode[];
  readonly completionClaimed: false;
}

export interface BmadHelpRunCreatedProjection {
  readonly schemaVersion: "bmad-help-run.v1";
  readonly runKind: "bmad_help";
  readonly lifecycle: "created_unbound";
  readonly workspaceId: string;
  readonly runId: string;
  readonly sessionId: string;
  readonly currentIntent: string;
  readonly runnable: false;
  readonly completionClaimed: false;
  readonly recommendation: BmadHelpRecommendationProjection;
}

export type BmadHelpUiState =
  | { readonly kind: "no_evidence" }
  | { readonly kind: "loading" }
  | {
    readonly kind: "legacy_projection_unavailable";
    readonly message: string;
  }
  | {
    readonly kind: "ready";
    readonly run: BmadHelpRunCreatedProjection;
  }
  | { readonly kind: "unavailable"; readonly message: string };
