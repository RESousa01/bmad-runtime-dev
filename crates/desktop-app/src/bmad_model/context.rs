use desktop_runtime::{
    canonical_hash, canonical_json_bytes, sha256_bytes, BmadArtifactClassification,
    BmadArtifactReference, BmadCompiledHelpInvocation, BmadHelpAction, BmadHelpEvidenceClass,
    BmadHelpEvidenceToken, BmadHelpIntent, BmadKernelError, BmadKernelErrorCode, ContractId,
    Sha256Digest,
};
use serde::Serialize;
use thiserror::Error;

use desktop_egress::{
    ContextCandidate, ContextClassification, ContextEgressManifest, ContextPreparer, EgressError,
    EgressLimits, PatternSecretScanner, PrepareContextInput,
};

use super::config::HelpModelConfiguration;

const ARCHITECTURE_MODULE: &str = "bmm";
const ARCHITECTURE_SKILL: &str = "bmad-architecture";
const ARCHITECTURE_ACTION: &str = "create";

pub(crate) struct DeterministicHelpPolicy {
    pub evidence_tokens: Vec<BmadHelpEvidenceToken>,
    pub evidence_facts: Vec<String>,
    pub deterministic_fixture: String,
}

pub(crate) struct HelpContextInput<'a> {
    pub compiled: &'a BmadCompiledHelpInvocation,
    pub intent: &'a BmadHelpIntent,
    pub policy: &'a DeterministicHelpPolicy,
    pub configuration: &'a HelpModelConfiguration,
    pub tenant_ref: ContractId,
    pub project_ref: ContractId,
    pub run_ref: ContractId,
    pub created_at: desktop_runtime::UnixMillis,
}

#[derive(Debug, Error)]
pub(crate) enum HelpContextError {
    #[error("the exact BMAD Help context could not be constructed")]
    Invalid,
    #[error(transparent)]
    Egress(#[from] EgressError),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProposalCapabilityKey<'a> {
    package_version_id: &'a ContractId,
    module_code: &'a str,
    skill_name: &'a str,
    normalized_action: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArchitectureEvidenceFact<'a> {
    schema_version: &'static str,
    fact_kind: &'static str,
    signal: &'static str,
    evidence_class: &'static str,
    intent_hash: Sha256Digest,
    capability_key: ProposalCapabilityKey<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecommendedCapabilityProposal<'a> {
    proposal_kind: &'static str,
    capability_key: ProposalCapabilityKey<'a>,
    evidence_token_ids: [&'a ContractId; 1],
    rationale_summary: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NoRecommendationProposal {
    proposal_kind: &'static str,
    reason_code: &'static str,
}

pub(crate) fn classify_architecture_signal(intent: &str) -> bool {
    intent
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .any(|token| {
            token.eq_ignore_ascii_case("architecture")
                || token.eq_ignore_ascii_case("architect")
                || token.eq_ignore_ascii_case("readiness")
        })
}

pub(crate) fn derive_deterministic_policy(
    compiled: &BmadCompiledHelpInvocation,
    intent: &BmadHelpIntent,
) -> Result<DeterministicHelpPolicy, BmadKernelError> {
    if !classify_architecture_signal(intent.as_str()) {
        return Ok(DeterministicHelpPolicy {
            evidence_tokens: Vec::new(),
            evidence_facts: Vec::new(),
            deterministic_fixture: canonical_string(&NoRecommendationProposal {
                proposal_kind: "no_recommendation",
                reason_code: "catalog_evidence_absent",
            })?,
        });
    }

    let action = architecture_action(compiled)?;
    let intent_hash = canonical_hash("bmad-help-user-asserted-intent", 1, &intent.as_str())
        .map_err(|_| policy_error())?;
    let fact = ArchitectureEvidenceFact {
        schema_version: "sapphirus.bmad-help-evidence-fact.v1",
        fact_kind: "user_asserted_intent_signal",
        signal: "architecture",
        evidence_class: "user_asserted",
        intent_hash,
        capability_key: proposal_key(action),
    };
    let fact_bytes = canonical_json_bytes(&fact).map_err(|_| policy_error())?;
    let fact_hash = sha256_bytes(&fact_bytes);
    let artifact_id = derived_contract_id("artifact", fact_hash)?;
    let token_hash = canonical_hash(
        "bmad-help-user-asserted-evidence-token",
        1,
        &(intent_hash, &action.key, fact_hash),
    )
    .map_err(|_| policy_error())?;
    let token_id = derived_contract_id("evidence", token_hash)?;
    let artifact_ref = BmadArtifactReference::new(
        artifact_id,
        format!("cas://sha256/{}", fact_hash.hex_value()),
        fact_hash,
        u64::try_from(fact_bytes.len()).map_err(|_| policy_error())?,
        "application/json",
        BmadArtifactClassification::Internal,
    )?;
    let token = BmadHelpEvidenceToken::from_host_fact(
        token_id,
        action.key.clone(),
        BmadHelpEvidenceClass::UserAsserted,
        artifact_ref,
    )?;
    let deterministic_fixture = canonical_string(&RecommendedCapabilityProposal {
        proposal_kind: "recommended_capability",
        capability_key: proposal_key(action),
        evidence_token_ids: [token.token_id()],
        rationale_summary:
            "The reviewed architecture intent matches this exact catalog capability.",
    })?;
    let fact = String::from_utf8(fact_bytes).map_err(|_| policy_error())?;

    Ok(DeterministicHelpPolicy {
        evidence_tokens: vec![token],
        evidence_facts: vec![fact],
        deterministic_fixture,
    })
}

pub(crate) fn prepare_help_context(
    input: HelpContextInput<'_>,
) -> Result<ContextEgressManifest, HelpContextError> {
    let HelpContextInput {
        compiled,
        intent,
        policy,
        configuration,
        tenant_ref,
        project_ref,
        run_ref,
        created_at,
    } = input;
    let instruction = std::str::from_utf8(compiled.instruction_bytes())
        .map_err(|_| HelpContextError::Invalid)?
        .to_owned();
    let architecture = architecture_action(compiled).map_err(|_| HelpContextError::Invalid)?;
    let catalog_candidate =
        canonical_string(architecture).map_err(|_| HelpContextError::Invalid)?;
    let mut candidates = vec![
        candidate(
            "context_01J00000000000000000000001",
            "review/bmad-help/instruction.md",
            "sealed_instruction",
            Some("markdown"),
            ContextClassification::Internal,
            instruction,
        )?,
        candidate(
            "context_01J00000000000000000000002",
            "review/bmad-help/current-intent.txt",
            "current_intent",
            Some("text"),
            ContextClassification::Confidential,
            intent.as_str().to_owned(),
        )?,
        candidate(
            "context_01J00000000000000000000003",
            "review/bmad-help/catalog-candidate.json",
            "catalog_candidate",
            Some("json"),
            ContextClassification::Internal,
            catalog_candidate,
        )?,
    ];
    if let Some(fact) = policy.evidence_facts.first() {
        if policy.evidence_facts.len() != 1 || policy.evidence_tokens.len() != 1 {
            return Err(HelpContextError::Invalid);
        }
        candidates.push(candidate(
            "context_01J00000000000000000000004",
            "review/bmad-help/evidence-fact.json",
            "evidence_fact",
            Some("json"),
            ContextClassification::Internal,
            fact.clone(),
        )?);
    } else if !policy.evidence_tokens.is_empty() {
        return Err(HelpContextError::Invalid);
    }
    let expires_at = desktop_runtime::UnixMillis(
        created_at
            .0
            .checked_add(5 * 60 * 1_000)
            .ok_or(HelpContextError::Invalid)?,
    );
    ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref,
            project_ref,
            run_ref,
            purpose: "bmad_help".to_owned(),
            model_role: "method_help".to_owned(),
            canonical_output_schema_id: configuration.canonical_output_schema_id.clone(),
            canonical_output_schema_hash: compiled.proposal_schema_closure_hash(),
            provider_profile_hash: configuration.provider_profile_hash,
            model_profile_hash: compiled
                .exact_binding()
                .model_binding
                .data
                .model_profile_hash,
            deployment_hash: configuration.deployment_hash,
            policy_hash: configuration.policy_hash,
            region: configuration.region.to_owned(),
            retention_mode: configuration.retention_mode,
            created_at,
            expires_at,
            limits: EgressLimits {
                maximum_context_items: 4,
                maximum_context_bytes: 64 * 1024,
                maximum_token_estimate: 16_000,
            },
            candidates,
            exclusions: Vec::new(),
        })
        .map_err(Into::into)
}

fn candidate(
    client_item_id: &str,
    relative_label: &str,
    semantic_role: &str,
    language: Option<&str>,
    classification: ContextClassification,
    content: String,
) -> Result<ContextCandidate, HelpContextError> {
    Ok(ContextCandidate {
        client_item_id: ContractId::new(client_item_id).map_err(|_| HelpContextError::Invalid)?,
        relative_label: desktop_runtime::RelativeWorkspacePath::new(relative_label)
            .map_err(|_| HelpContextError::Invalid)?,
        semantic_role: semantic_role.to_owned(),
        language: language.map(str::to_owned),
        classification,
        content,
    })
}

fn architecture_action(
    compiled: &BmadCompiledHelpInvocation,
) -> Result<&BmadHelpAction, BmadKernelError> {
    let mut matches = compiled.catalog_candidates().iter().filter(|action| {
        action.module_code == ARCHITECTURE_MODULE
            && action.skill_name == ARCHITECTURE_SKILL
            && action.action.as_deref() == Some(ARCHITECTURE_ACTION)
            && action.skill_name != "_meta"
    });
    let action = matches.next().ok_or_else(policy_error)?;
    if matches.next().is_some() {
        return Err(policy_error());
    }
    Ok(action)
}

fn proposal_key(action: &BmadHelpAction) -> ProposalCapabilityKey<'_> {
    ProposalCapabilityKey {
        package_version_id: &action.key.package_version_id,
        module_code: &action.module_code,
        skill_name: &action.skill_name,
        normalized_action: action.action.as_deref(),
    }
}

pub(super) fn derived_contract_id(
    prefix: &str,
    digest: Sha256Digest,
) -> Result<ContractId, BmadKernelError> {
    const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let suffix = digest
        .as_bytes()
        .iter()
        .map(|byte| char::from(CROCKFORD[usize::from(*byte & 0x1f)]))
        .collect::<String>();
    ContractId::new(format!("{prefix}_{suffix}")).map_err(|_| policy_error())
}

fn canonical_string(value: &impl Serialize) -> Result<String, BmadKernelError> {
    String::from_utf8(canonical_json_bytes(value).map_err(|_| policy_error())?)
        .map_err(|_| policy_error())
}

fn policy_error() -> BmadKernelError {
    BmadKernelErrorCode::HelpProposalInvalid.into()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use desktop_runtime::{
        canonical_hash, canonical_json_bytes, sha256_bytes, BmadHelpBindingCompiler,
        BmadHelpIntent, BmadTrustedHelpModelProfile, BmadTrustedHelpModelProfileData, ContractId,
        UnixMillis,
    };
    use serde_json::{json, Value};

    use super::{
        classify_architecture_signal, derive_deterministic_policy, prepare_help_context,
        HelpContextInput,
    };
    use crate::bmad_foundation::load_bmad_foundation;
    use crate::bmad_model::config::current_help_model_configuration;

    fn foundation_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/bmad-foundation")
    }

    fn compiled_help() -> desktop_runtime::BmadCompiledHelpInvocation {
        let foundation = load_bmad_foundation(foundation_path()).expect("sealed foundation");
        let profile =
            BmadTrustedHelpModelProfile::from_host_assertion(BmadTrustedHelpModelProfileData {
                provider_id: "deterministic-local".to_owned(),
                model_id: "bmad-help-fixture-v1".to_owned(),
                deployment_id: "localdev".to_owned(),
                model_profile_hash: canonical_hash(
                    "bmad-help-deterministic-model-profile",
                    1,
                    &json!({"mode": "deterministic_development"}),
                )
                .expect("profile hash"),
                model_capability_hash: canonical_hash(
                    "bmad-help-deterministic-model-capability",
                    1,
                    &json!({"proposalSchema": "sapphirus.bmad-method-help-proposal.v1"}),
                )
                .expect("capability hash"),
                context_window_profile_hash: canonical_hash(
                    "bmad-help-deterministic-context-window",
                    1,
                    &json!({"maximumContextBytes": 65536, "maximumContextItems": 4}),
                )
                .expect("context hash"),
                egress_profile_hash: canonical_hash(
                    "bmad-help-deterministic-egress-profile",
                    1,
                    &json!({"region": "localdev", "retention": "transient_no_store"}),
                )
                .expect("egress hash"),
                request_schema_hash: canonical_hash(
                    "bmad-help-model-request-schema",
                    1,
                    &json!({"schema": "sapphirus.bmad-help-model-request.v1"}),
                )
                .expect("request schema hash"),
            })
            .expect("trusted deterministic profile");
        BmadHelpBindingCompiler::compile(
            foundation.help_invocation(),
            foundation.catalog(),
            &profile,
        )
        .expect("compiled Help")
    }

    fn id(value: &str) -> ContractId {
        ContractId::new(value).expect("qualified fixture identifier")
    }

    #[test]
    fn complete_ascii_tokens_are_the_only_architecture_signal() {
        for intent in [
            "Review the architecture",
            "Act as an architect",
            "Is this readiness sufficient?",
            "ARCHITECTURE readiness",
        ] {
            assert!(classify_architecture_signal(intent), "{intent}");
        }

        for intent in [
            "microarchitecture review",
            "architectural notes",
            "readiness2",
            "architecture_ready",
            "pick the next planning step",
        ] {
            assert!(!classify_architecture_signal(intent), "{intent}");
        }
    }

    #[test]
    fn explicit_signal_yields_one_user_asserted_architecture_token() {
        let compiled = compiled_help();
        let intent =
            BmadHelpIntent::new("Please review architecture readiness").expect("bounded intent");
        let policy = derive_deterministic_policy(&compiled, &intent).expect("closed policy");

        assert_eq!(policy.evidence_tokens.len(), 1);
        assert_eq!(policy.evidence_facts.len(), 1);
        let fact: Value = serde_json::from_str(&policy.evidence_facts[0]).expect("evidence fact");
        assert_eq!(fact["factKind"], "user_asserted_intent_signal");
        assert_eq!(fact["signal"], "architecture");
        assert_eq!(fact["evidenceClass"], "user_asserted");
        assert_eq!(fact["capabilityKey"]["moduleCode"], "bmm");
        assert_eq!(fact["capabilityKey"]["skillName"], "bmad-architecture");
        assert_eq!(fact["capabilityKey"]["normalizedAction"], "create");

        let proposal: Value =
            serde_json::from_str(&policy.deterministic_fixture).expect("proposal fixture");
        assert_eq!(proposal["proposalKind"], "recommended_capability");
        assert_eq!(proposal["capabilityKey"], fact["capabilityKey"]);
        assert_eq!(
            proposal["evidenceTokenIds"].as_array().map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn absent_signal_yields_no_evidence_and_the_only_provable_absence() {
        let compiled = compiled_help();
        let intent =
            BmadHelpIntent::new("Help me choose the next planning step").expect("bounded intent");
        let policy = derive_deterministic_policy(&compiled, &intent).expect("closed policy");

        assert!(policy.evidence_tokens.is_empty());
        assert!(policy.evidence_facts.is_empty());
        assert_eq!(
            serde_json::from_str::<Value>(&policy.deterministic_fixture).expect("proposal fixture"),
            json!({
                "proposalKind": "no_recommendation",
                "reasonCode": "catalog_evidence_absent"
            })
        );
    }

    #[test]
    fn exact_context_is_ordered_bounded_and_closed() {
        let compiled = compiled_help();
        let intent = BmadHelpIntent::new("Review architecture readiness").expect("bounded intent");
        let policy = derive_deterministic_policy(&compiled, &intent).expect("closed policy");
        let configuration = current_help_model_configuration().expect("host configuration");
        let manifest = prepare_help_context(HelpContextInput {
            compiled: &compiled,
            intent: &intent,
            policy: &policy,
            configuration: &configuration,
            tenant_ref: id("tenant_01J00000000000000000000000"),
            project_ref: id("project_01J00000000000000000000000"),
            run_ref: id("run_01J000000000000000000000000"),
            created_at: UnixMillis(1_000),
        })
        .expect("exact prepared context");

        assert_eq!(manifest.draft.expires_at, UnixMillis(301_000));
        assert_eq!(manifest.draft.limits.maximum_context_items, 4);
        assert_eq!(manifest.draft.limits.maximum_context_bytes, 64 * 1024);
        assert_eq!(manifest.draft.limits.maximum_token_estimate, 16_000);
        assert_eq!(manifest.draft.items.len(), 4);

        let items = &manifest.draft.items;
        assert_eq!(
            items
                .iter()
                .map(|item| item.client_item_id.as_str())
                .collect::<Vec<_>>(),
            [
                "context_01J00000000000000000000001",
                "context_01J00000000000000000000002",
                "context_01J00000000000000000000003",
                "context_01J00000000000000000000004",
            ]
        );
        assert_eq!(
            items
                .iter()
                .map(|item| item.relative_label.as_str())
                .collect::<Vec<_>>(),
            [
                "review/bmad-help/instruction.md",
                "review/bmad-help/current-intent.txt",
                "review/bmad-help/catalog-candidate.json",
                "review/bmad-help/evidence-fact.json",
            ]
        );
        assert_eq!(
            items
                .iter()
                .map(|item| item.semantic_role.as_str())
                .collect::<Vec<_>>(),
            [
                "sealed_instruction",
                "current_intent",
                "catalog_candidate",
                "evidence_fact",
            ]
        );
        assert_eq!(
            items[0].outbound_content.as_bytes(),
            compiled.instruction_bytes()
        );
        assert_eq!(items[1].outbound_content, intent.as_str());
        let architecture = compiled
            .catalog_candidates()
            .iter()
            .find(|action| action.skill_name == "bmad-architecture")
            .expect("architecture candidate");
        assert_eq!(
            items[2].outbound_content.as_bytes(),
            canonical_json_bytes(architecture).expect("canonical catalog candidate")
        );
        assert_eq!(items[3].outbound_content, policy.evidence_facts[0]);
        assert!(items.iter().all(
            |item| item.outbound_content_hash == sha256_bytes(item.outbound_content.as_bytes())
        ));
        assert!(!items[2].outbound_content.contains("_meta"));
        assert!(!items
            .iter()
            .any(|item| item.relative_label.as_str().contains("workspace")));

        let mut reordered = manifest.draft.clone();
        reordered.items.swap(0, 1);
        let reordered = reordered.seal().expect("valid reordered manifest");
        assert_ne!(reordered.manifest_hash, manifest.manifest_hash);

        let changed_intent =
            BmadHelpIntent::new("Review the architecture").expect("changed bounded intent");
        let changed_policy =
            derive_deterministic_policy(&compiled, &changed_intent).expect("changed policy");
        let changed = prepare_help_context(HelpContextInput {
            compiled: &compiled,
            intent: &changed_intent,
            policy: &changed_policy,
            configuration: &configuration,
            tenant_ref: id("tenant_01J00000000000000000000000"),
            project_ref: id("project_01J00000000000000000000000"),
            run_ref: id("run_01J000000000000000000000000"),
            created_at: UnixMillis(1_000),
        })
        .expect("changed exact context");
        assert_ne!(changed.manifest_hash, manifest.manifest_hash);
    }

    #[test]
    fn scanner_redacts_intent_secrets_without_admitting_extra_context() {
        let compiled = compiled_help();
        let intent = BmadHelpIntent::new(concat!(
            "Review architecture with API_KEY=",
            "super-secret-development-value",
        ))
        .expect("bounded intent");
        let policy = derive_deterministic_policy(&compiled, &intent).expect("closed policy");
        let configuration = current_help_model_configuration().expect("host configuration");
        let manifest = prepare_help_context(HelpContextInput {
            compiled: &compiled,
            intent: &intent,
            policy: &policy,
            configuration: &configuration,
            tenant_ref: id("tenant_01J00000000000000000000000"),
            project_ref: id("project_01J00000000000000000000000"),
            run_ref: id("run_01J000000000000000000000000"),
            created_at: UnixMillis(1_000),
        })
        .expect("redacted exact context");

        assert_eq!(
            manifest.draft.items[1].outbound_content,
            "Review architecture with API_KEY=[REDACTED:credential]"
        );
        assert_eq!(manifest.draft.secret_findings.len(), 1);
        assert_eq!(manifest.draft.secret_findings[0].kind, "credential");
        assert_eq!(manifest.draft.items.len(), 4);
    }
}
