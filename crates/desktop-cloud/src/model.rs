use std::{collections::HashMap, fmt, sync::Arc};

use desktop_egress::{
    ContextClassification, ContextEgressManifest, DecisionConsumption, EgressError,
    ModelInvocationBinding, RedactionRecord, RetentionMode,
};
use desktop_runtime::{
    canonical_hash, sha256_bytes, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::CloudError;

const REQUEST_SCHEMA: &str = "sapphirus.authorized-model-request.v1";
const RECEIPT_SCHEMA: &str = "sapphirus.model-access-receipt.v1";
pub(crate) const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_ACCEPTED_RECEIPT_IDS: usize = 100_000;

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

#[derive(Debug, Eq, PartialEq)]
struct InvocationAuthority;

/// A request contains one consumed invocation capability and cannot be cloned
/// for a local replay.
///
/// ```compile_fail
/// fn duplicate(request: desktop_cloud::AuthorizedModelRequest) {
///     let replay = request.clone();
/// }
/// ```
///
/// ```compile_fail
/// # use desktop_cloud::AuthorizedModelRequest;
/// fn retarget(mut request: AuthorizedModelRequest) -> AuthorizedModelRequest {
///     request.items[0].content = "unreviewed context".to_owned();
///     request.request_hash = desktop_runtime::sha256_bytes(b"rehashed");
///     request
/// }
/// ```
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedModelRequest {
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
    request_hash: Sha256Digest,
    #[serde(skip)]
    authority: InvocationAuthority,
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
        consumption: DecisionConsumption,
    ) -> Result<Self, CloudError> {
        binding.verify_for(manifest).map_err(map_egress_error)?;
        consumption.verify().map_err(map_egress_error)?;
        if consumption.manifest_hash() != manifest.manifest_hash
            || consumption.binding_hash() != binding.binding_hash
            || consumption.consent_disclosure_hash() != binding.draft.consent_disclosure_hash
            || consumption.policy_hash() != binding.draft.policy_hash
            || consumption.installation_id() != &binding.draft.installation_id
            || consumption.session_authority_hash() != binding.draft.session_authority_hash
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
            invocation_id: consumption.invocation_id().clone(),
            decision_id: consumption.decision_id().clone(),
            decision_hash: consumption.decision_hash(),
            manifest_hash: manifest.manifest_hash,
            binding_hash: binding.binding_hash,
            consumption_hash: consumption.consumption_hash(),
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
        drop(consumption);
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
            authority: InvocationAuthority,
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

    /// Consumes the request into the parts needed by the canonical
    /// support-plane transport projection. Consumption prevents any second
    /// projection or dispatch of the same authorized request.
    pub(crate) fn into_transport_parts(self) -> TransportRequestParts {
        TransportRequestParts {
            request_id: self.request_id,
            invocation_id: self.invocation_id,
            decision_id: self.decision_id,
            manifest_hash: self.manifest_hash,
            binding_hash: self.binding_hash,
            consumption_hash: self.consumption_hash,
            consent_disclosure_hash: self.consent_disclosure_hash,
            purpose: self.purpose,
            model_role: self.model_role,
            canonical_output_schema_id: self.canonical_output_schema_id,
            canonical_output_schema_hash: self.canonical_output_schema_hash,
            provider_profile_hash: self.provider_profile_hash,
            model_profile_hash: self.model_profile_hash,
            deployment_hash: self.deployment_hash,
            policy_hash: self.policy_hash,
            region: self.region,
            retention_mode: self.retention_mode,
            items: self.items,
        }
    }

    #[must_use]
    pub fn request_id(&self) -> &ContractId {
        &self.request_id
    }

    #[must_use]
    pub fn purpose(&self) -> &str {
        &self.purpose
    }

    #[must_use]
    pub fn model_role(&self) -> &str {
        &self.model_role
    }

    #[must_use]
    pub fn canonical_output_schema_id(&self) -> &ContractId {
        &self.canonical_output_schema_id
    }

    #[must_use]
    pub const fn canonical_output_schema_hash(&self) -> Sha256Digest {
        self.canonical_output_schema_hash
    }

    #[must_use]
    pub const fn manifest_hash(&self) -> Sha256Digest {
        self.manifest_hash
    }

    #[must_use]
    pub const fn binding_hash(&self) -> Sha256Digest {
        self.binding_hash
    }

    #[must_use]
    pub const fn consumption_hash(&self) -> Sha256Digest {
        self.consumption_hash
    }

    #[must_use]
    pub const fn consent_disclosure_hash(&self) -> Sha256Digest {
        self.consent_disclosure_hash
    }

    #[must_use]
    pub const fn provider_profile_hash(&self) -> Sha256Digest {
        self.provider_profile_hash
    }

    #[must_use]
    pub const fn model_profile_hash(&self) -> Sha256Digest {
        self.model_profile_hash
    }

    #[must_use]
    pub const fn deployment_hash(&self) -> Sha256Digest {
        self.deployment_hash
    }

    #[must_use]
    pub const fn policy_hash(&self) -> Sha256Digest {
        self.policy_hash
    }

    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }

    #[must_use]
    pub const fn retention_mode(&self) -> RetentionMode {
        self.retention_mode
    }

    #[must_use]
    pub fn items(&self) -> &[AuthorizedContextItem] {
        &self.items
    }

    #[must_use]
    pub const fn total_outbound_bytes(&self) -> u64 {
        self.total_outbound_bytes
    }

    #[must_use]
    pub const fn total_token_estimate(&self) -> u64 {
        self.total_token_estimate
    }

    #[must_use]
    pub const fn request_hash(&self) -> Sha256Digest {
        self.request_hash
    }

    pub(crate) fn verify(&self) -> Result<(), CloudError> {
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

/// Consumed request parts released to the canonical support-plane transport
/// projection. Local-only authority fields (tenant/project/run refs, decision
/// hash, policy-internal totals) are deliberately not carried.
pub(crate) struct TransportRequestParts {
    pub(crate) request_id: ContractId,
    pub(crate) invocation_id: ContractId,
    pub(crate) decision_id: ContractId,
    pub(crate) manifest_hash: Sha256Digest,
    pub(crate) binding_hash: Sha256Digest,
    pub(crate) consumption_hash: Sha256Digest,
    pub(crate) consent_disclosure_hash: Sha256Digest,
    pub(crate) purpose: String,
    pub(crate) model_role: String,
    pub(crate) canonical_output_schema_id: ContractId,
    pub(crate) canonical_output_schema_hash: Sha256Digest,
    pub(crate) provider_profile_hash: Sha256Digest,
    pub(crate) model_profile_hash: Sha256Digest,
    pub(crate) deployment_hash: Sha256Digest,
    pub(crate) policy_hash: Sha256Digest,
    pub(crate) region: String,
    pub(crate) retention_mode: RetentionMode,
    pub(crate) items: Vec<AuthorizedContextItem>,
}

/// A request that has crossed the one-shot transport boundary. It retains the
/// trusted request bindings needed for response verification but cannot be
/// submitted again.
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct DispatchedModelRequest(AuthorizedModelRequest);

impl DispatchedModelRequest {
    pub(crate) const fn new(request: AuthorizedModelRequest) -> Self {
        Self(request)
    }
}

/// Verifies a response against a request that was consumed by the transport.
///
/// # Errors
///
/// Returns [`CloudError`] for any schema, receipt, payload, or binding drift.
pub fn verify_dispatched_model_response<S, R>(
    request: DispatchedModelRequest,
    response: RawModelOutput,
    schema_validator: &S,
    receipt_verifier: &R,
) -> Result<VerifiedModelOutput, CloudError>
where
    S: CanonicalOutputValidator,
    R: ReceiptVerifier,
{
    let DispatchedModelRequest(request) = request;
    verify_model_response(&request, response, schema_validator, receipt_verifier)
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

/// Trusted-host output whose verified payload and receipt evidence must not be
/// serialized across an untrusted boundary.
///
/// ```compile_fail
/// fn assert_serializable<T: serde::Serialize>() {}
/// assert_serializable::<desktop_cloud::VerifiedModelOutput>();
/// ```
///
/// ```compile_fail
/// fn expose_payload(output: &desktop_cloud::VerifiedModelOutput) {
///     let _ = &output.payload;
/// }
/// ```
#[derive(Clone, PartialEq)]
pub struct VerifiedModelOutput {
    request_id: ContractId,
    output_schema_id: ContractId,
    payload: Value,
    payload_bytes: Arc<[u8]>,
    payload_hash: Sha256Digest,
    receipt: ModelAccessReceipt,
}

impl VerifiedModelOutput {
    #[must_use]
    pub fn request_id(&self) -> &ContractId {
        &self.request_id
    }

    #[must_use]
    pub fn output_schema_id(&self) -> &ContractId {
        &self.output_schema_id
    }

    #[must_use]
    pub fn payload(&self) -> &Value {
        &self.payload
    }

    #[must_use]
    pub fn payload_bytes(&self) -> &[u8] {
        self.payload_bytes.as_ref()
    }

    #[must_use]
    pub const fn payload_hash(&self) -> Sha256Digest {
        self.payload_hash
    }

    #[must_use]
    pub const fn receipt(&self) -> &ModelAccessReceipt {
        &self.receipt
    }
}

impl fmt::Debug for VerifiedModelOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedModelOutput")
            .field("request_id", &self.request_id)
            .field("output_schema_id", &self.output_schema_id)
            .field("payload", &"[REDACTED]")
            .field("payload_bytes", &"[REDACTED]")
            .field("payload_hash", &self.payload_hash)
            .field("receipt", &"[REDACTED]")
            .finish()
    }
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

pub trait ReceiptProofVerifier: Send + Sync {
    /// Verifies the receipt signature, issuer, audience, and key policy.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::ReceiptInvalid`] when proof or trust fails.
    fn verify_proof(&self, receipt: &ModelAccessReceipt) -> Result<(), CloudError>;
}

pub trait ReceiptClock: Send + Sync {
    /// Returns the trusted host time used for receipt freshness checks.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::ReceiptInvalid`] when trusted time is unavailable.
    fn now(&self) -> Result<UnixMillis, CloudError>;
}

#[derive(Debug, Default)]
pub struct SystemReceiptClock;

impl ReceiptClock for SystemReceiptClock {
    fn now(&self) -> Result<UnixMillis, CloudError> {
        let millis = OffsetDateTime::now_utc()
            .unix_timestamp_nanos()
            .checked_div(1_000_000)
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(CloudError::ReceiptInvalid)?;
        Ok(UnixMillis(millis))
    }
}

#[derive(Debug)]
pub struct ReplaySafeReceiptVerifier<P, C> {
    proof_verifier: P,
    clock: C,
    maximum_age_ms: u64,
    maximum_future_skew_ms: u64,
    accepted_receipt_ids: Mutex<HashMap<String, u64>>,
}

impl<P, C> ReplaySafeReceiptVerifier<P, C>
where
    P: ReceiptProofVerifier,
    C: ReceiptClock,
{
    /// Creates an in-process receipt freshness and replay guard.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::ReceiptInvalid`] for unbounded policy windows.
    pub fn new(
        proof_verifier: P,
        clock: C,
        maximum_age_ms: u64,
        maximum_future_skew_ms: u64,
    ) -> Result<Self, CloudError> {
        if maximum_age_ms == 0
            || maximum_age_ms > 24 * 60 * 60 * 1_000
            || maximum_future_skew_ms > 5 * 60 * 1_000
        {
            return Err(CloudError::ReceiptInvalid);
        }
        Ok(Self {
            proof_verifier,
            clock,
            maximum_age_ms,
            maximum_future_skew_ms,
            accepted_receipt_ids: Mutex::new(HashMap::new()),
        })
    }
}

impl<P, C> ReceiptVerifier for ReplaySafeReceiptVerifier<P, C>
where
    P: ReceiptProofVerifier,
    C: ReceiptClock,
{
    fn verify(&self, receipt: &ModelAccessReceipt) -> Result<(), CloudError> {
        if receipt.started_at > receipt.completed_at {
            return Err(CloudError::ReceiptInvalid);
        }
        let now = self.clock.now()?;
        let latest_allowed = now
            .0
            .checked_add(self.maximum_future_skew_ms)
            .ok_or(CloudError::ReceiptInvalid)?;
        if receipt.completed_at.0 > latest_allowed
            || now.0.saturating_sub(receipt.completed_at.0) > self.maximum_age_ms
        {
            return Err(CloudError::ReceiptInvalid);
        }
        self.proof_verifier.verify_proof(receipt)?;
        let mut accepted = self.accepted_receipt_ids.lock();
        let oldest_relevant = now.0.saturating_sub(self.maximum_age_ms);
        accepted.retain(|_, completed_at| *completed_at >= oldest_relevant);
        if accepted.contains_key(receipt.receipt_id.as_str())
            || accepted.len() >= MAX_ACCEPTED_RECEIPT_IDS
        {
            return Err(CloudError::ReceiptInvalid);
        }
        accepted.insert(
            receipt.receipt_id.as_str().to_owned(),
            receipt.completed_at.0,
        );
        Ok(())
    }
}

/// Verifies untrusted model output and receipt bindings before projection.
///
/// # Errors
///
/// Returns [`CloudError`] for request/schema substitution, malformed or drifted
/// payloads, receipt-binding drift, or failed schema/receipt trust checks.
fn verify_model_response<V, R>(
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
    let payload_bytes = Arc::from(response.payload_json.into_bytes());
    Ok(VerifiedModelOutput {
        request_id: response.request_id,
        output_schema_id: response.output_schema_id,
        payload,
        payload_bytes,
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
