use desktop_egress::{ContextDecisionEvidence, ContextEgressManifest, ModelInvocationBinding};
use desktop_runtime::{
    canonical_hash, ContractId, MethodContextDecision, MethodExactBinding, Sha256Digest, UnixMillis,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BmadHelpDecisionBridgeExpectation {
    pub(crate) decision_id: ContractId,
    pub(crate) manifest_hash: Sha256Digest,
    pub(crate) d2_binding_hash: Sha256Digest,
    pub(crate) session_authority_hash: Sha256Digest,
    pub(crate) method_binding_hash: Sha256Digest,
    pub(crate) issued_at: UnixMillis,
    pub(crate) expires_at: UnixMillis,
    pub(crate) observed_at: UnixMillis,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum BmadModelBridgeError {
    #[error("the reviewed D2 decision does not match the exact BMAD Help authority")]
    BindingMismatch,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewedContextDigest<'a> {
    items: Vec<ReviewedContextDigestItem<'a>>,
    manifest_hash: Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewedContextDigestItem<'a> {
    client_item_id: &'a ContractId,
    outbound_content_hash: Sha256Digest,
    outbound_byte_count: u64,
}

/// Validates all D2 and Method anchors before creating the Method review
/// decision used by the aggregate.
///
/// # Errors
///
/// Returns [`BmadModelBridgeError::BindingMismatch`] when any sealed value or
/// caller-held expectation is invalid, stale, or substituted.
#[expect(
    clippy::needless_pass_by_value,
    reason = "consuming the one-use evidence view keeps its ledger guard live through bridge validation and prevents reuse"
)]
pub(crate) fn bridge_method_context_decision(
    evidence: ContextDecisionEvidence<'_>,
    manifest: &ContextEgressManifest,
    d2_binding: &ModelInvocationBinding,
    method_binding: &MethodExactBinding,
    expected: &BmadHelpDecisionBridgeExpectation,
) -> Result<MethodContextDecision, BmadModelBridgeError> {
    manifest
        .verify()
        .map_err(|_| BmadModelBridgeError::BindingMismatch)?;
    d2_binding
        .verify_for(manifest)
        .map_err(|_| BmadModelBridgeError::BindingMismatch)?;
    let method_binding_hash = method_binding
        .binding_hash()
        .map_err(|_| BmadModelBridgeError::BindingMismatch)?;

    if evidence.decision_id() != &expected.decision_id
        || evidence.manifest_hash() != manifest.manifest_hash
        || evidence.manifest_hash() != expected.manifest_hash
        || evidence.invocation_binding_hash() != d2_binding.binding_hash
        || evidence.invocation_binding_hash() != expected.d2_binding_hash
        || evidence.consent_disclosure_hash() != d2_binding.draft.consent_disclosure_hash
        || evidence.policy_hash() != d2_binding.draft.policy_hash
        || evidence.installation_id() != &d2_binding.draft.installation_id
        || evidence.session_authority_hash() != d2_binding.draft.session_authority_hash
        || evidence.session_authority_hash() != expected.session_authority_hash
        || method_binding_hash != expected.method_binding_hash
        || evidence.issued_at() != expected.issued_at
        || evidence.expires_at() != expected.expires_at
        || evidence.observed_at() != expected.observed_at
        || evidence.issued_at() < manifest.draft.created_at
        || evidence.expires_at() > manifest.draft.expires_at
        || evidence.observed_at() < evidence.issued_at()
        || evidence.observed_at() > evidence.expires_at()
    {
        return Err(BmadModelBridgeError::BindingMismatch);
    }

    let context_digest = canonical_hash(
        "bmad-help-reviewed-context",
        1,
        &ReviewedContextDigest {
            items: manifest
                .draft
                .items
                .iter()
                .map(|item| ReviewedContextDigestItem {
                    client_item_id: &item.client_item_id,
                    outbound_content_hash: item.outbound_content_hash,
                    outbound_byte_count: item.outbound_byte_count,
                })
                .collect(),
            manifest_hash: manifest.manifest_hash,
        },
    )
    .map_err(|_| BmadModelBridgeError::BindingMismatch)?;

    Ok(MethodContextDecision {
        decision_id: evidence.decision_id().clone(),
        manifest_hash: evidence.manifest_hash(),
        consent_hash: evidence.consent_disclosure_hash(),
        context_digest,
        binding_hash: method_binding_hash,
        reviewed_at: evidence.issued_at(),
    })
}
