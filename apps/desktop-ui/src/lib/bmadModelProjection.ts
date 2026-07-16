import type { BmadHelpRunCreatedProjection } from "./bmadProjection";

export const bmadModelCommands = {
  authStatus: "model.auth.status",
  authSignIn: "model.auth.sign_in",
  authSignOut: "model.auth.sign_out",
  prepare: "bmad.help.prepare",
  approve: "bmad.help.approve",
  cancel: "bmad.help.cancel",
  submit: "bmad.help.submit",
  latest: "bmad.help.latest",
} as const;

export type BmadModelCommand = (typeof bmadModelCommands)[keyof typeof bmadModelCommands];

export interface ModelAuthStatusProjection {
  readonly status: "unavailable" | "development_ready";
  readonly mode: "offline" | "deterministic_development";
  readonly authEpoch: number;
  readonly developmentOnly: boolean;
  readonly destinationLabel: string;
  readonly signInAvailable: false;
  readonly signOutAvailable: true;
}

export interface BmadHelpReviewRedactionProjection {
  readonly kind: string;
  readonly occurrenceCount: number;
}

export interface BmadHelpReviewItemProjection {
  readonly relativeLabel: string;
  readonly semanticRole: string;
  readonly language: string | null;
  readonly outboundByteCount: number;
  readonly tokenEstimate: number;
  readonly classification: "public" | "internal" | "confidential";
  readonly redactions: readonly BmadHelpReviewRedactionProjection[];
  readonly outboundContent: string;
}

export interface BmadHelpReviewExclusionProjection {
  readonly relativeLabel: string;
  readonly reason: string;
}

export interface BmadHelpReviewFindingProjection {
  readonly relativeLabel: string;
  readonly kind: string;
  readonly occurrenceCount: number;
}

export interface BmadHelpContextReviewProjection {
  readonly workspaceId: string;
  readonly workspaceGrantEpoch: number;
  readonly runId: string;
  readonly sessionId: string;
  readonly destinationLabel: string;
  readonly developmentOnly: boolean;
  readonly consentDisclosure: string;
  readonly manifestHash: string;
  readonly purpose: string;
  readonly region: string;
  readonly retentionMode: "transient_no_store";
  readonly expiresAt: number;
  readonly items: readonly BmadHelpReviewItemProjection[];
  readonly exclusions: readonly BmadHelpReviewExclusionProjection[];
  readonly secretFindings: readonly BmadHelpReviewFindingProjection[];
  readonly totalOutboundBytes: number;
  readonly totalTokenEstimate: number;
  readonly redactionLimitation: string;
}

export interface BmadHelpApprovedProjection {
  readonly manifestHash: string;
  readonly decisionId: string;
  readonly expiresAt: number;
  readonly sendEligible: true;
}

export interface BmadHelpCancelledProjection {
  readonly manifestHash: string;
  readonly decisionId: string;
}

export interface BmadHelpTerminalProjection {
  readonly workspaceId: string;
  readonly reason: "cancelled" | "consent_expired" | "consent_consumed" | "failed";
  readonly resumable: false;
  readonly sendEligible: false;
}

export type BmadHelpEvidenceClass =
  | "authoritative"
  | "user_asserted"
  | "heuristic"
  | "contextual";

export type BmadHelpNoRecommendationReason =
  | "catalog_evidence_absent"
  | "completion_evidence_ambiguous"
  | "dependency_unavailable";

export type BmadHelpCompletedRecommendationProjection =
  | {
    readonly recommendationKind: "recommended_capability";
    readonly displayName: string;
    readonly moduleCode: string;
    readonly skillName: string;
    readonly action: string | null;
    readonly evidenceClass: BmadHelpEvidenceClass;
    readonly guidanceRequired: boolean;
    readonly rationaleSummary: string;
    readonly createdAt: number;
  }
  | {
    readonly recommendationKind: "no_recommendation";
    readonly reasonCode: BmadHelpNoRecommendationReason;
    readonly createdAt: number;
  };

export interface BmadHelpReceiptSummaryProjection {
  readonly schemaVersion: "bmad-model-receipt-summary.v1";
  readonly receiptId: string;
  readonly status: "succeeded";
  readonly retentionMode: "transient_no_store";
  readonly region: string;
  readonly inputBytes: number;
  readonly outputBytes: number;
  readonly startedAt: number;
  readonly completedAt: number;
}

export interface BmadHelpRunCompletedProjection {
  readonly schemaVersion: "bmad-help-completed.v1";
  readonly runKind: "bmad_help";
  readonly lifecycle: "completed";
  readonly workspaceId: string;
  readonly runId: string;
  readonly sessionId: string;
  readonly runnable: false;
  readonly completionClaimed: true;
  readonly recommendation: BmadHelpCompletedRecommendationProjection;
  readonly receipt: BmadHelpReceiptSummaryProjection;
}

export type BmadHelpInterruptedProjection = BmadHelpRunCreatedProjection;

export interface BmadAuthoritySnapshot {
  readonly workspaceId: string;
  readonly workspaceGrantEpoch: number;
  readonly runId: string;
  readonly authEpoch: number;
  readonly rendererGeneration: number;
  readonly now: number;
}

interface BmadRequestAuthority {
  readonly workspaceId: string;
  readonly workspaceGrantEpoch: number;
  readonly runId: string;
  readonly sessionId: string;
  readonly authEpoch: number;
  readonly rendererGeneration: number;
  readonly manifestHash: string;
  readonly expiresAt: number;
}

interface BmadHelpRunIdentity {
  readonly workspaceId: string;
  readonly runId: string;
  readonly sessionId: string;
}

export type BmadTerminalReason =
  | "cancelled"
  | "consent_expired"
  | "consent_consumed"
  | "authority_changed"
  | "failed";

export type BmadRequestState =
  | { readonly kind: "idle"; readonly run: BmadHelpRunCreatedProjection | null }
  | { readonly kind: "creating"; readonly activity: "recovering" | "creating" }
  | {
    readonly kind: "review_required";
    readonly run: BmadHelpRunIdentity;
    readonly runProjection: BmadHelpRunCreatedProjection | null;
    readonly review: BmadHelpContextReviewProjection;
    readonly authority: BmadRequestAuthority;
  }
  | {
    readonly kind: "approving";
    readonly run: BmadHelpRunIdentity;
    readonly runProjection: BmadHelpRunCreatedProjection | null;
    readonly review: BmadHelpContextReviewProjection;
    readonly authority: BmadRequestAuthority;
  }
  | {
    readonly kind: "approved";
    readonly run: BmadHelpRunIdentity;
    readonly runProjection: BmadHelpRunCreatedProjection | null;
    readonly review: BmadHelpContextReviewProjection;
    readonly authority: BmadRequestAuthority;
    readonly approval: BmadHelpApprovedProjection;
  }
  | {
    readonly kind: "submitting";
    readonly run: BmadHelpRunIdentity;
    readonly runProjection: BmadHelpRunCreatedProjection | null;
    readonly review: BmadHelpContextReviewProjection;
    readonly authority: BmadRequestAuthority;
  }
  | { readonly kind: "completed"; readonly result: BmadHelpRunCompletedProjection }
  | { readonly kind: "interrupted"; readonly result: BmadHelpInterruptedProjection }
  | { readonly kind: "terminal"; readonly reason: BmadTerminalReason }
  | {
    readonly kind: "unavailable";
    readonly message: string;
    readonly run: BmadHelpRunCreatedProjection | null;
  };

export const initialBmadRequestState: BmadRequestState = { kind: "idle", run: null };

export type BmadRequestEvent =
  | { readonly type: "recover_started" }
  | { readonly type: "recovered"; readonly run: BmadHelpRunCreatedProjection | null }
  | { readonly type: "create_started" }
  | {
    readonly type: "review_ready";
    readonly run: BmadHelpRunCreatedProjection;
    readonly review: BmadHelpContextReviewProjection;
    readonly authority: BmadAuthoritySnapshot;
  }
  | {
    readonly type: "review_recovered";
    readonly review: BmadHelpContextReviewProjection;
    readonly authority: BmadAuthoritySnapshot;
  }
  | { readonly type: "approve_started" }
  | { readonly type: "approved"; readonly approval: BmadHelpApprovedProjection }
  | { readonly type: "submit_started" }
  | { readonly type: "completed"; readonly result: BmadHelpRunCompletedProjection }
  | { readonly type: "interrupted"; readonly result: BmadHelpInterruptedProjection }
  | { readonly type: "terminal"; readonly reason: BmadTerminalReason }
  | { readonly type: "authority_invalidated"; readonly reason: "authority_changed" | "consent_expired" }
  | {
    readonly type: "unavailable";
    readonly message: string;
    readonly run?: BmadHelpRunCreatedProjection | null;
  };

function unavailable(message: string, run: BmadHelpRunCreatedProjection | null = null): BmadRequestState {
  return { kind: "unavailable", message, run };
}

export function transitionBmadRequest(
  state: BmadRequestState,
  event: BmadRequestEvent,
): BmadRequestState {
  switch (event.type) {
    case "recover_started":
      return { kind: "creating", activity: "recovering" };
    case "recovered":
      return state.kind === "creating"
        ? { kind: "idle", run: event.run }
        : state;
    case "create_started":
      return { kind: "creating", activity: "creating" };
    case "review_ready":
    case "review_recovered": {
      if (state.kind !== "creating") return state;
      const { authority, review } = event;
      const run: BmadHelpRunIdentity = event.type === "review_ready"
        ? event.run
        : {
          workspaceId: review.workspaceId,
          runId: review.runId,
          sessionId: review.sessionId,
        };
      if (
        run.workspaceId !== review.workspaceId
        || run.runId !== review.runId
        || run.sessionId !== review.sessionId
        || authority.workspaceId !== review.workspaceId
        || authority.workspaceGrantEpoch !== review.workspaceGrantEpoch
        || authority.runId !== review.runId
        || authority.now >= review.expiresAt
      ) {
        return unavailable(
          "The prepared request no longer matches the active Method run.",
          event.type === "review_ready" ? event.run : null,
        );
      }
      return {
        kind: "review_required",
        run,
        runProjection: event.type === "review_ready" ? event.run : null,
        review,
        authority: {
          workspaceId: authority.workspaceId,
          workspaceGrantEpoch: authority.workspaceGrantEpoch,
          runId: authority.runId,
          sessionId: review.sessionId,
          authEpoch: authority.authEpoch,
          rendererGeneration: authority.rendererGeneration,
          manifestHash: review.manifestHash,
          expiresAt: review.expiresAt,
        },
      };
    }
    case "approve_started":
      return state.kind === "review_required" ? { ...state, kind: "approving" } : state;
    case "approved":
      if (state.kind !== "approving") return state;
      if (
        event.approval.manifestHash !== state.authority.manifestHash
        || !event.approval.sendEligible
        || event.approval.expiresAt > state.review.expiresAt
      ) {
        return unavailable("The approval did not match the displayed request.", state.runProjection);
      }
      return {
        ...state,
        kind: "approved",
        authority: { ...state.authority, expiresAt: event.approval.expiresAt },
        approval: event.approval,
      };
    case "submit_started":
      if (state.kind !== "approved") return state;
      return {
        kind: "submitting",
        run: state.run,
        runProjection: state.runProjection,
        review: state.review,
        authority: state.authority,
      };
    case "completed":
      if (state.kind !== "submitting") return state;
      if (
        event.result.workspaceId !== state.authority.workspaceId
        || event.result.runId !== state.authority.runId
        || event.result.sessionId !== state.authority.sessionId
      ) {
        return unavailable("The completed result did not match the submitted Method run.");
      }
      return { kind: "completed", result: event.result };
    case "interrupted":
      return { kind: "interrupted", result: event.result };
    case "terminal":
      return { kind: "terminal", reason: event.reason };
    case "authority_invalidated":
      return state.kind === "review_required"
        || state.kind === "approving"
        || state.kind === "approved"
        || state.kind === "submitting"
        ? { kind: "terminal", reason: event.reason }
        : state;
    case "unavailable":
      return unavailable(event.message, event.run ?? null);
  }
}

export function bmadRequestAuthorityIsCurrent(
  state: BmadRequestState,
  current: BmadAuthoritySnapshot,
): boolean {
  if (
    state.kind !== "review_required"
    && state.kind !== "approving"
    && state.kind !== "approved"
    && state.kind !== "submitting"
  ) {
    return false;
  }
  return state.authority.workspaceId === current.workspaceId
    && state.authority.workspaceGrantEpoch === current.workspaceGrantEpoch
    && state.authority.runId === current.runId
    && state.authority.authEpoch === current.authEpoch
    && state.authority.rendererGeneration === current.rendererGeneration
    && current.now < state.authority.expiresAt;
}
