use desktop_egress::{
    ContextClassification, ContextEgressManifest, DecisionConsumption, EgressError,
    ModelInvocationBinding, RedactionRecord, RetentionMode,
};
use desktop_runtime::{
    canonical_hash, sha256_bytes, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CloudError;

const REQUEST_SCHEMA: &str = "sapphirus.authorized-model-request.v1";
const RECEIPT_SCHEMA: &str = "sapphirus.model-access-receipt.v1";
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedContextItem {
    pub client_item_id: ContractId,
    pub relative_label: RelativeWorkspacePath,
    pub semantic_role: String,
    pub language: Option<String>,
    pub content_hash: Sha256Digest,
    pub byte_count: u64,
    pub token_estimate: u64,
    pub classification: ContextClassification,
    pub redactions: Vec<RedactionRecord>,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizedModelRequestDraft {
    schema_version: String,
    request_id: ContractId,
    tenant_ref: ContractId,
    project_ref: ContractId,
    run_ref: ContractId,
    installation_id: ContractId,
    invocation_id: ContractId,
    decision_id: ContractId,
    decision_hash: Sha256Digest,
    manifest_hash: Sha256Digest,
    binding_hash: Sha256Digest,
    consumption_hash: Sha256Digest,
    consent_disclosure_hash: Sha256Digest,
    purpose: String,
    model_role: String,
    canonical_output_schema_id: ContractId,
    canonical_output_schema_hash: Sha256Digest,
    provider_profile_hash: Sha256Digest,
    model_profile_hash: Sha256Digest,
    deployment_hash: Sha256Digest,
    policy_hash: Sha256Digest,
    region: String,
    retention_mode: RetentionMode,
    items: Vec<AuthorizedContextItem>,
    total_outbound_bytes: u64,
    total_token_estimate: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedModelRequest {
    pub schema_version: String,
    pub request_id: ContractId,
    pub tenant_ref: ContractId,
    pub project_ref: ContractId,
    pub run_ref: ContractId,
    pub installation_id: ContractId,
    pub invocation_id: ContractId,
    pub decision_id: ContractId,
    pub decision_hash: Sha256Digest,
    pub manifest_hash: Sha256Digest,
    pub binding_hash: Sha256Digest,
    pub consumption_hash: Sha256Digest,
    pub consent_disclosure_hash: Sha256Digest,
    pub purpose: String,
    pub model_role: String,
    pub canonical_output_schema_id: ContractId,
    pub canonical_output_schema_hash: Sha256Digest,
    pub provider_profile_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub policy_hash: Sha256Digest,
    pub region: String,
    pub retention_mode: RetentionMode,
    pub items: Vec<AuthorizedContextItem>,
    pub total_outbound_bytes: u64,
    pub total_token_estimate: u64,
    pub request_hash: Sha256Digest,
}

impl AuthorizedModelRequest {
    /// Maps only consumed, exactly reviewed context into a transport request.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError`] when manifest, binding, or consumption authority
    /// is invalid or inconsistent.
    pub fn new(
        manifest: &ContextEgressManifest,
        binding: &ModelInvocationBinding,
        consumption: &DecisionConsumption,
    ) -> Result<Self, CloudError> {
        binding.verify_for(manifest).map_err(map_egress_error)?;
        consumption.verify().map_err(map_egress_error)?;
        if consumption.manifest_hash != manifest.manifest_hash
            || consumption.binding_hash != binding.binding_hash
            || consumption.consent_disclosure_hash != binding.draft.consent_disclosure_hash
            || consumption.policy_hash != binding.draft.policy_hash
            || consumption.installation_id != binding.draft.installation_id
            || consumption.session_authority_hash != binding.draft.session_authority_hash
        {
            return Err(CloudError::ConsentBindingMismatch);
        }
        let items = manifest
            .draft
            .items
            .iter()
            .map(|item| AuthorizedContextItem {
                client_item_id: item.client_item_id.clone(),
                relative_label: item.relative_label.clone(),
                semantic_role: item.semantic_role.clone(),
                language: item.language.clone(),
                content_hash: item.outbound_content_hash,
                byte_count: item.outbound_byte_count,
                token_estimate: item.token_estimate,
                classification: item.classification,
                redactions: item.redactions.clone(),
                content: item.outbound_content.clone(),
            })
            .collect();
        let draft = AuthorizedModelRequestDraft {
            schema_version: REQUEST_SCHEMA.to_owned(),
            request_id: binding.draft.request_id.clone(),
            tenant_ref: binding.draft.tenant_ref.clone(),
            project_ref: binding.draft.project_ref.clone(),
            run_ref: binding.draft.run_ref.clone(),
            installation_id: binding.draft.installation_id.clone(),
            invocation_id: consumption.invocation_id.clone(),
            decision_id: consumption.decision_id.clone(),
            decision_hash: consumption.decision_hash,
            manifest_hash: manifest.manifest_hash,
            binding_hash: binding.binding_hash,
            consumption_hash: consumption.consumption_hash,
            consent_disclosure_hash: binding.draft.consent_disclosure_hash,
            purpose: binding.draft.purpose.clone(),
            model_role: binding.draft.model_role.clone(),
            canonical_output_schema_id: binding.draft.canonical_output_schema_id.clone(),
            canonical_output_schema_hash: binding.draft.canonical_output_schema_hash,
            provider_profile_hash: binding.draft.provider_profile_hash,
            model_profile_hash: binding.draft.model_profile_hash,
            deployment_hash: binding.draft.deployment_hash,
            policy_hash: binding.draft.policy_hash,
            region: binding.draft.region.clone(),
            retention_mode: binding.draft.retention_mode,
            items,
            total_outbound_bytes: manifest.draft.total_outbound_bytes,
            total_token_estimate: manifest.draft.total_token_estimate,
        };
        Self::seal(draft)
    }

    fn seal(draft: AuthorizedModelRequestDraft) -> Result<Self, CloudError> {
        validate_request_draft(&draft)?;
        let request_hash = canonical_hash("authorized-model-request", 1, &draft)
            .map_err(|_| CloudError::ConsentBindingMismatch)?;
        Ok(Self {
            schema_version: draft.schema_version,
            request_id: draft.request_id,
            tenant_ref: draft.tenant_ref,
            project_ref: draft.project_ref,
            run_ref: draft.run_ref,
            installation_id: draft.installation_id,
            invocation_id: draft.invocation_id,
            decision_id: draft.decision_id,
            decision_hash: draft.decision_hash,
            manifest_hash: draft.manifest_hash,
            binding_hash: draft.binding_hash,
            consumption_hash: draft.consumption_hash,
            consent_disclosure_hash: draft.consent_disclosure_hash,
            purpose: draft.purpose,
            model_role: draft.model_role,
            canonical_output_schema_id: draft.canonical_output_schema_id,
            canonical_output_schema_hash: draft.canonical_output_schema_hash,
            provider_profile_hash: draft.provider_profile_hash,
            model_profile_hash: draft.model_profile_hash,
            deployment_hash: draft.deployment_hash,
            policy_hash: draft.policy_hash,
            region: draft.region,
            retention_mode: draft.retention_mode,
            items: draft.items,
            total_outbound_bytes: draft.total_outbound_bytes,
            total_token_estimate: draft.total_token_estimate,
            request_hash,
        })
    }

    fn draft(&self) -> AuthorizedModelRequestDraft {
        AuthorizedModelRequestDraft {
            schema_version: self.schema_version.clone(),
            request_id: self.request_id.clone(),
            tenant_ref: self.tenant_ref.clone(),
            project_ref: self.project_ref.clone(),
            run_ref: self.run_ref.clone(),
            installation_id: self.installation_id.clone(),
            invocation_id: self.invocation_id.clone(),
            decision_id: self.decision_id.clone(),
            decision_hash: self.decision_hash,
            manifest_hash: self.manifest_hash,
            binding_hash: self.binding_hash,
            consumption_hash: self.consumption_hash,
            consent_disclosure_hash: self.consent_disclosure_hash,
            purpose: self.purpose.clone(),
            model_role: self.model_role.clone(),
            canonical_output_schema_id: self.canonical_output_schema_id.clone(),
            canonical_output_schema_hash: self.canonical_output_schema_hash,
            provider_profile_hash: self.provider_profile_hash,
            model_profile_hash: self.model_profile_hash,
            deployment_hash: self.deployment_hash,
            policy_hash: self.policy_hash,
            region: self.region.clone(),
            retention_mode: self.retention_mode,
            items: self.items.clone(),
            total_outbound_bytes: self.total_outbound_bytes,
            total_token_estimate: self.total_token_estimate,
        }
    }

    fn verify(&self) -> Result<(), CloudError> {
        let draft = self.draft();
        validate_request_draft(&draft)?;
        let actual = canonical_hash("authorized-model-request", 1, &draft)
            .map_err(|_| CloudError::ConsentBindingMismatch)?;
        if actual != self.request_hash {
            return Err(CloudError::ConsentBindingMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelReceiptStatus {
    Succeeded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelAccessReceipt {
    pub schema_version: String,
    pub receipt_id: ContractId,
    pub request_id: ContractId,
    pub request_hash: Sha256Digest,
    pub result_hash: Sha256Digest,
    pub manifest_hash: Sha256Digest,
    pub binding_hash: Sha256Digest,
    pub consumption_hash: Sha256Digest,
    pub consent_disclosure_hash: Sha256Digest,
    pub provider_profile_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub retention_mode: RetentionMode,
    pub region: String,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub started_at: UnixMillis,
    pub completed_at: UnixMillis,
    pub status: ModelReceiptStatus,
    pub proof: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RawModelOutput {
    pub request_id: ContractId,
    pub output_schema_id: ContractId,
    pub payload_json: String,
    pub payload_hash: Sha256Digest,
    pub receipt: ModelAccessReceipt,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedModelOutput {
    pub request_id: ContractId,
    pub output_schema_id: ContractId,
    pub payload: Value,
    pub payload_hash: Sha256Digest,
    pub receipt: ModelAccessReceipt,
}

pub trait CanonicalOutputValidator: Send + Sync {
    /// Validates the parsed payload against one exact canonical schema.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::InvalidModelOutput`] when validation fails.
    fn validate(
        &self,
        schema_id: &ContractId,
        schema_hash: Sha256Digest,
        payload: &Value,
    ) -> Result<(), CloudError>;
}

pub trait ReceiptVerifier: Send + Sync {
    /// Verifies service receipt proof and trust policy.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::ReceiptInvalid`] when proof or trust fails.
    fn verify(&self, receipt: &ModelAccessReceipt) -> Result<(), CloudError>;
}

/// Verifies untrusted model output and receipt bindings before projection.
///
/// # Errors
///
/// Returns [`CloudError`] for request/schema substitution, malformed or drifted
/// payloads, receipt-binding drift, or failed schema/receipt trust checks.
pub fn verify_model_response<V, R>(
    request: &AuthorizedModelRequest,
    response: RawModelOutput,
    output_validator: &V,
    receipt_verifier: &R,
) -> Result<VerifiedModelOutput, CloudError>
where
    V: CanonicalOutputValidator,
    R: ReceiptVerifier,
{
    request.verify()?;
    if response.request_id != request.request_id
        || response.output_schema_id != request.canonical_output_schema_id
    {
        return Err(CloudError::ResponseBindingMismatch);
    }
    if response.payload_json.len() > MAX_OUTPUT_BYTES
        || sha256_bytes(response.payload_json.as_bytes()) != response.payload_hash
    {
        return Err(CloudError::InvalidModelOutput);
    }
    let payload: Value =
        serde_json::from_str(&response.payload_json).map_err(|_| CloudError::InvalidModelOutput)?;
    output_validator.validate(
        &request.canonical_output_schema_id,
        request.canonical_output_schema_hash,
        &payload,
    )?;
    validate_receipt(request, &response)?;
    receipt_verifier.verify(&response.receipt)?;
    Ok(VerifiedModelOutput {
        request_id: response.request_id,
        output_schema_id: response.output_schema_id,
        payload,
        payload_hash: response.payload_hash,
        receipt: response.receipt,
    })
}

fn validate_request_draft(draft: &AuthorizedModelRequestDraft) -> Result<(), CloudError> {
    if draft.schema_version != REQUEST_SCHEMA || draft.items.is_empty() {
        return Err(CloudError::ConsentBindingMismatch);
    }
    let mut total_bytes = 0_u64;
    let mut total_tokens = 0_u64;
    for item in &draft.items {
        let byte_count =
            u64::try_from(item.content.len()).map_err(|_| CloudError::ConsentBindingMismatch)?;
        if byte_count != item.byte_count
            || sha256_bytes(item.content.as_bytes()) != item.content_hash
        {
            return Err(CloudError::ContextDrift);
        }
        total_bytes = total_bytes
            .checked_add(item.byte_count)
            .ok_or(CloudError::ConsentBindingMismatch)?;
        total_tokens = total_tokens
            .checked_add(item.token_estimate)
            .ok_or(CloudError::ConsentBindingMismatch)?;
    }
    if total_bytes != draft.total_outbound_bytes || total_tokens != draft.total_token_estimate {
        return Err(CloudError::ContextDrift);
    }
    Ok(())
}

fn validate_receipt(
    request: &AuthorizedModelRequest,
    response: &RawModelOutput,
) -> Result<(), CloudError> {
    let receipt = &response.receipt;
    let output_bytes =
        u64::try_from(response.payload_json.len()).map_err(|_| CloudError::InvalidModelOutput)?;
    if receipt.schema_version != RECEIPT_SCHEMA
        || receipt.request_id != request.request_id
        || receipt.request_hash != request.request_hash
        || receipt.result_hash != response.payload_hash
        || receipt.manifest_hash != request.manifest_hash
        || receipt.binding_hash != request.binding_hash
        || receipt.consumption_hash != request.consumption_hash
        || receipt.consent_disclosure_hash != request.consent_disclosure_hash
        || receipt.provider_profile_hash != request.provider_profile_hash
        || receipt.model_profile_hash != request.model_profile_hash
        || receipt.deployment_hash != request.deployment_hash
        || receipt.retention_mode != request.retention_mode
        || receipt.region != request.region
        || receipt.input_bytes != request.total_outbound_bytes
        || receipt.output_bytes != output_bytes
        || receipt.started_at > receipt.completed_at
        || receipt.status != ModelReceiptStatus::Succeeded
    {
        return Err(CloudError::ResponseBindingMismatch);
    }
    Ok(())
}

fn map_egress_error(error: EgressError) -> CloudError {
    match error {
        EgressError::ContextDrift | EgressError::ManifestIntegrity => CloudError::ContextDrift,
        _ => CloudError::ConsentBindingMismatch,
    }
}
