#![allow(clippy::expect_used, clippy::panic)]

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use desktop_runtime::{
    generated_contracts::{BmadArtifact, BmadArtifactState, BmadSession, BmadSessionState},
    BmadArtifactIntent, BmadCompletionIntent, BmadDomainRuntime, BmadRuntimePort,
    BmadSessionIntent,
};
use serde_json::json;

struct DomainBoundary {
    module: &'static str,
    allowed_uses: &'static [&'static str],
    allowed_body_path_roots: &'static [&'static str],
}

const DOMAIN_BOUNDARIES: &[DomainBoundary] = &[
    DomainBoundary {
        module: "source",
        allowed_uses: &[
            "usesuper::{kernel_error::BmadKernelError,package::{BmadSourceEntry,BmadSourceSnapshot},};",
        ],
        allowed_body_path_roots: &[],
    },
    DomainBoundary {
        module: "catalog",
        allowed_uses: &[
            "usestd::collections::{BTreeMap,BTreeSet};",
            "useserde::Serialize;",
            "useserde_json::{Map,Value};",
            "usecrate::{canonical_hash,generated_contracts,sha256_bytes,ContractId,RelativeWorkspacePath,Sha256Digest,};",
            "usesuper::{kernel_error::{BmadKernelError,BmadKernelErrorCode},package::{BmadEntrypointKind,BmadLoadedPackage},};",
        ],
        allowed_body_path_roots: &[
            "collect",
            "from_slice",
            "generated_contracts",
            "mem",
            "serde_json",
            "std",
            "str",
            "u64",
        ],
    },
    DomainBoundary {
        module: "config",
        allowed_uses: &[
            "usestd::collections::BTreeMap;",
            "useserde::Serialize;",
            "useserde_json::{Map,Value};",
            "usecrate::{canonical_hash,Sha256Digest};",
            "usesuper::kernel_error::{BmadKernelError,BmadKernelErrorCode};",
        ],
        allowed_body_path_roots: &["char", "collect", "serde_json", "u32", "usize"],
    },
    DomainBoundary {
        module: "resources",
        allowed_uses: &[
            "usestd::collections::{BTreeMap,BTreeSet};",
            "usestd::sync::Arc;",
            "usethiserror::Error;",
            "usecrate::{sha256_bytes,RelativeWorkspacePath,Sha256Digest};",
            "usesuper::kernel_error::{BmadKernelError,BmadKernelErrorCode};",
        ],
        allowed_body_path_roots: &["char", "clippy", "collect", "ffi", "path", "std", "str"],
    },
    DomainBoundary {
        module: "session",
        allowed_uses: &[
            "usecrate::generated_contracts::{BmadSession,BmadSessionSessionControl,BmadSessionState};",
        ],
        allowed_body_path_roots: &[],
    },
    DomainBoundary {
        module: "state_machine",
        allowed_uses: &[
            "useserde::Serialize;",
            "useserde_json::json;",
            "usecrate::generated_contracts::{BmadSession,BmadSessionApprovalRequest,BmadSessionInputRequest,BmadSessionModelCall,BmadSessionOutcome,BmadSessionOutcomeKind,BmadSessionRecordedAnswer,BmadSessionSessionControl,BmadSessionState,BmadSessionToolProposal,BmadSessionToolProposalStatus,};",
            "usecrate::{canonical_hash,Sha256Digest};",
        ],
        allowed_body_path_roots: &["crate", "generated_contracts", "num", "std"],
    },
    DomainBoundary {
        module: "action_plan",
        allowed_uses: &["useserde_json::Value;", "usethiserror::Error;", "usecrate::generated_contracts::{BmadActionPlan,BmadActionPlanAuthorityClass,BmadActionPlanBudget,BmadActionPlanEffect,BmadActionPlanKind,BmadActionPlanModelCall,BmadActionPlanProvenance,BmadActionPlanReadsItem,CommonContractId,CommonSha256,};", "usecrate::{canonical_hash_without_field,Sha256Digest};"],
        allowed_body_path_roots: &["serde_json"],
    },
    DomainBoundary {
        module: "model_port",
        allowed_uses: &["useserde::de::DeserializeOwned;", "useserde_json::Value;", "usethiserror::Error;", "usecrate::generated_contracts::{BmadActionPlan,BmadActionPlanAuthorityClass,BmadActionPlanKind,};"],
        allowed_body_path_roots: &["crate", "generated_contracts", "serde_json"],
    },
    DomainBoundary {
        module: "orchestrator",
        allowed_uses: &["useserde_json::json;", "usethiserror::Error;", "usecrate::canonical_hash;", "usecrate::generated_contracts::{BmadActionPlan,BmadActionPlanBudget,BmadActionPlanEffect,BmadActionPlanEffectEffectClass,BmadActionPlanKind,BmadActionPlanModelCall,BmadActionPlanModelCallTrustMode,};", "usesuper::action_plan::{build_plan,BmadActionPlanError,BmadActionPlanRequest};"],
        allowed_body_path_roots: &["clippy", "crate"],
    },
    DomainBoundary {
        module: "memory",
        allowed_uses: &["usecrate::generated_contracts::{BmadMemoryMemoryEntry,BmadMemoryMemoryEntryMemoryClass,BmadMemoryMemoryEntryMutability,BmadMemoryMemoryEntryRetention,BmadMemoryMemoryEntrySensitivity,BmadMemoryMemoryProvenanceOrigin,BmadMemoryMemoryScope,BmadMemoryMemoryScopeKind,CommonContractId,CommonSha256,};", "usecrate::{canonical_hash,Sha256Digest};"],
        allowed_body_path_roots: &["serde"],
    },
    DomainBoundary {
        module: "review",
        allowed_uses: &["usestd::{collections::BTreeSet,num::NonZeroU64};", "useserde_json::Value;", "usethiserror::Error;", "usecrate::generated_contracts::{BmadReviewResult,BmadReviewResultDisposition,BmadReviewResultEvidenceLocation,BmadReviewResultFinding,BmadReviewResultFindingOrigin,BmadReviewResultFindingSeverity,BmadReviewResultFindingVerificationState,BmadReviewResultLens,BmadReviewResultTargetKind,BmadReviewResultVerification,CommonContractId,CommonSha256,};", "usecrate::{canonical_hash_without_field,sha256_bytes,Sha256Digest};"],
        allowed_body_path_roots: &["serde_json"],
    },
    DomainBoundary {
        module: "failure",
        allowed_uses: &["useserde_json::json;", "usecrate::generated_contracts::{BmadProblem,BmadProblemCancellation,BmadProblemCode,BmadProblemDetail,BmadProblemFailureClass,BmadProblemPreservedState,BmadProblemRecoveryAction,BmadProblemRecoveryActionAction,BmadProblemRecoveryActionReason,BmadProblemRetry,CommonContractId,CommonSha256,};", "usecrate::{canonical_hash,Sha256Digest};", "useBmadProblemRecoveryActionAction::{Abandon,Reauthorize,Repair,Resume,Retry,Rollback};"],
        allowed_body_path_roots: &["parse", "str"],
    },
    DomainBoundary {
        module: "context_projection",
        allowed_uses: &["useserde::Serialize;", "useserde_json::json;", "usecrate::generated_contracts::{BmadContextContribution,BmadContextContributionContribution,BmadContextContributionContributionInclusionReason,BmadContextContributionContributionKind,BmadContextContributionContributionTransformation,BmadContextContributionIdentities,BmadContextContributionPurpose,BmadContextContributionReportedUsage,CommonContractId,CommonSha256,};", "usecrate::{canonical_hash_without_field,Sha256Digest};"],
        allowed_body_path_roots: &["collect", "i64", "serde_json"],
    },
    DomainBoundary {
        module: "invocation",
        allowed_uses: &[
            "useserde_json::Value;",
            "usethiserror::Error;",
            "usecrate::canonical_hash;",
            "usecrate::generated_contracts::{BmadInvocation,BmadInvocationBudget,BmadInvocationDeclaredAction,BmadInvocationDeclaredInput,BmadInvocationDeclaredInputValueKind,BmadInvocationInputRequirement,BmadInvocationInputRequirementAllowedValuesItem,BmadInvocationInputRequirementCode,BmadInvocationInputRequirementName,BmadInvocationInputRequirementPrompt,BmadInvocationInputRequirementValueKind,BmadInvocationMode,BmadInvocationResolvedInput,BmadInvocationResolvedInputName,BmadInvocationResolvedInputOrigin,CommonContractId,};",
        ],
        allowed_body_path_roots: &["clippy", "collect"],
    },
    DomainBoundary {
        module: "artifacts",
        allowed_uses: &[
            "usestd::collections::BTreeSet;",
            "usecrate::generated_contracts::{BmadArtifact,BmadArtifactCompletionHookRun,BmadArtifactCompletionHookRunHook,BmadArtifactCompletionHookRunStatus,BmadArtifactState,BmadArtifactValidationPassedChecksItem,BmadArtifactValidationStatus,CommonSha256,};",
            "usecrate::{canonical_hash,CanonicalHashError,Sha256Digest};",
        ],
        allowed_body_path_roots: &["bmad", "collect", "num", "serde", "serde_json", "std"],
    },
    DomainBoundary {
        module: "completion",
        allowed_uses: &[
            "usecrate::generated_contracts::{BmadArtifact,BmadArtifactState,BmadSession,BmadSessionCompletionHookHook,BmadSessionState,CommonContractId,};",
        ],
        allowed_body_path_roots: &[],
    },
];

fn bmad_source_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bmad")
}

fn production_source(name: &str) -> String {
    let path = bmad_source_dir().join(format!("{name}.rs"));
    let source = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("BMAD module {} is missing: {error}", path.display()));
    source
        .split_once("#[cfg(test)]")
        .map_or(source.clone(), |(production, _)| production.to_owned())
}

fn normalized_use_statements(source: &str) -> Vec<String> {
    let mut uses = Vec::new();
    let mut statement = String::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let starts_use = statement.is_empty()
            && (trimmed.starts_with("use ") || trimmed.starts_with("pub use "));
        if starts_use || !statement.is_empty() {
            statement.push_str(trimmed);
        }
        if !statement.is_empty() && trimmed.contains(';') {
            uses.push(
                statement
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .collect(),
            );
            statement.clear();
        }
    }
    assert!(statement.is_empty(), "unterminated use declaration");
    uses
}

fn body_without_use_statements(source: &str) -> String {
    let mut body = String::new();
    let mut in_use = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if !in_use && (trimmed.starts_with("use ") || trimmed.starts_with("pub use ")) {
            in_use = true;
        }
        if !in_use {
            body.push_str(line);
            body.push('\n');
        }
        if in_use && trimmed.contains(';') {
            in_use = false;
        }
    }
    body
}

fn lowercase_path_roots(source: &str) -> BTreeSet<String> {
    let bytes = source.as_bytes();
    let mut roots = BTreeSet::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let starts_identifier = byte == b'_' || byte.is_ascii_lowercase();
        let follows_identifier =
            index > 0 && (bytes[index - 1] == b'_' || bytes[index - 1].is_ascii_alphanumeric());
        if starts_identifier && !follows_identifier {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
            {
                index += 1;
            }
            if index + 1 < bytes.len() && bytes[index] == b':' && bytes[index + 1] == b':' {
                roots.insert(source[start..index].to_owned());
            }
        } else {
            index += 1;
        }
    }
    roots
}

fn declared_type_names(source: &str) -> BTreeSet<String> {
    let words = source
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    words
        .windows(2)
        .filter(|pair| matches!(pair[0], "struct" | "enum" | "trait"))
        .map(|pair| pair[1].to_owned())
        .collect()
}

#[test]
fn bmad_architecture_enforces_explicit_domain_dependency_edges() {
    for boundary in DOMAIN_BOUNDARIES {
        let source = production_source(boundary.module);
        assert!(
            !source
                .lines()
                .any(|line| line.trim_start().starts_with("pub use ")),
            "{} must not bypass its boundary with re-exports",
            boundary.module
        );
        assert_eq!(
            normalized_use_statements(&source),
            boundary.allowed_uses,
            "{} dependency imports changed; review and update the explicit boundary",
            boundary.module
        );
        assert_eq!(
            lowercase_path_roots(&body_without_use_statements(&source)),
            boundary
                .allowed_body_path_roots
                .iter()
                .map(|root| (*root).to_owned())
                .collect(),
            "{} introduced an unreviewed fully-qualified dependency",
            boundary.module
        );
    }

    let facade = production_source("mod");
    assert!(
        facade.contains("pub struct BmadDomainRuntime;"),
        "runtime facade must remain an explicit zero-state boundary"
    );
    assert_eq!(std::mem::size_of::<BmadDomainRuntime>(), 0);
    assert_eq!(
        declared_type_names(&facade),
        ["BmadDomainRuntime", "BmadRuntimePort"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "the facade may not accumulate a differently named coordinator or state aggregate"
    );

    let library = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("library facade");
    for bypass in [
        "apply_artifact_intent",
        "apply_session_intent",
        "plan_completion",
    ] {
        assert!(
            !library.contains(bypass),
            "raw domain operation {bypass} must not bypass BmadRuntimePort"
        );
    }
}

fn session() -> BmadSession {
    serde_json::from_value(json!({
        "agentProfile": "stateless",
        "artifacts": [],
        "capabilityId": "capability_01J00000000000000000000000",
        "completionHooks": [{"enabled": true, "hook": "emit_receipt"}],
        "control": {
            "cancelAllowed": true,
            "pauseAllowed": false,
            "resumeAllowed": false
        },
        "evidence": [],
        "mode": "interactive",
        "schemaVersion": "sapphirus.bmad-session.v1",
        "sessionId": "session_01J00000000000000000000000",
        "state": "created",
        "version": 1,
        "identities": {
            "sourceId": "source_01J00000000000000000000000",
            "packageId": "package_01J00000000000000000000000",
            "configHash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "customizationHash": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            "modelId": "model_01J00000000000000000000000",
            "taskId": "task_01J00000000000000000000000",
            "workspaceId": "workspace_01J00000000000000000000000",
            "policyHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222"
        },
        "evidenceHash": "sha256:3333333333333333333333333333333333333333333333333333333333333333"
    }))
    .expect("generated session")
}

fn artifact() -> BmadArtifact {
    serde_json::from_value(json!({
        "artifactId": "artifact_01J00000000000000000000000",
        "contentHash": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "evidenceHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "kind": "document",
        "schemaVersion": "sapphirus.bmad-artifact.v1",
        "sessionId": "session_01J00000000000000000000000",
        "state": "draft",
        "revision": 1,
        "lineage": {
            "sourceId": "source_01J00000000000000000000000",
            "templateHash": "sha256:4444444444444444444444444444444444444444444444444444444444444444",
            "inputsHash": "sha256:5555555555555555555555555555555555555555555555555555555555555555"
        },
        "validation": {
            "status": "passed",
            "declaredChecks": ["schema_shape"],
            "passedChecks": ["schema_shape"]
        }
    }))
    .expect("generated artifact")
}

#[test]
fn bmad_architecture_migrates_intent_callers_through_the_runtime_port() {
    let runtime = BmadDomainRuntime;
    let port: &dyn BmadRuntimePort = &runtime;
    let mut session = session();
    port.apply_session_intent(&mut session, BmadSessionIntent::Start)
        .expect("start");
    port.apply_session_intent(&mut session, BmadSessionIntent::Complete)
        .expect("complete");
    assert_eq!(session.state, BmadSessionState::Completed);

    let mut artifact = artifact();
    port.apply_artifact_intent(
        &mut artifact,
        BmadArtifactIntent::Validate {
            passed_checks: vec!["schema_shape".to_owned()],
        },
    )
    .expect("validate");
    port.apply_artifact_intent(&mut artifact, BmadArtifactIntent::Approve)
        .expect("approve");
    port.apply_artifact_intent(&mut artifact, BmadArtifactIntent::Write)
        .expect("write");
    assert_eq!(artifact.state, BmadArtifactState::Written);

    let intents = port
        .plan_completion(&session, &[artifact])
        .expect("completion plan");
    assert!(matches!(
        intents.as_slice(),
        [BmadCompletionIntent::EmitReceipt(_)]
    ));
}
