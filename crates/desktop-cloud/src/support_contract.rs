//! Canonical support-plane transport projection (D2-E Task 1).
//!
//! [`AuthorizedModelRequest`] is local authority and never crosses the wire.
//! The only path from local authority to the canonical [`ModelAccessRequest`]
//! envelope is [`project_model_access_request`], which consumes the request,
//! requires the current registration, verified lease and tenant policy
//! bindings, and an installation consent signer, and emits exactly the public
//! canonical shape — no local refs, decision internals, token estimates, or
//! redaction details.

use std::num::NonZeroU64;
use std::str::FromStr;

use desktop_egress::{ContextClassification, RetentionMode};
use desktop_runtime::generated_contracts as canonical;
use desktop_runtime::{canonical_hash, ContractId, Sha256Digest, UnixMillis};
use serde::Serialize;
use time::OffsetDateTime;

use crate::model::AuthorizedModelRequest;
use crate::CloudError;

pub use canonical::{
    ModelAccessRequest, ModelAccessRequestContextItem, ModelContextConsent,
    ModelContextConsentInstallationProof,
};

const REQUEST_SCHEMA_VERSION: &str = "desktop-model-access-request.v1";
const CONSENT_SCHEMA_VERSION: &str = "sapphirus.model-context-consent.v1";
const DELIVERY_MODEL: &str = "windows_local";
const RETENTION_MODE: &str = "transient_no_store";
const PROOF_TYPE: &str = "installation_signature";
const PROOF_ALGORITHM: &str = "ES256";

/// The current, service-acknowledged device registration this desktop holds.
#[derive(Clone, Debug)]
pub struct RegistrationBinding {
    pub registration_id: ContractId,
    pub installation_public_key_hash: Sha256Digest,
}

/// An entitlement lease that has already passed proof verification.
#[derive(Clone, Debug)]
pub struct LeaseBinding {
    pub lease_id: ContractId,
    pub lease_hash: Sha256Digest,
}

/// A tenant policy that has already passed proof verification. Its hash must
/// match the policy hash bound into the authorized request.
#[derive(Clone, Debug)]
pub struct TenantPolicyBinding {
    pub policy_id: ContractId,
    pub policy_version: NonZeroU64,
    pub policy_hash: Sha256Digest,
}

/// Pseudonymous tenant/subject identity hashes for the signed consent.
#[derive(Clone, Debug)]
pub struct ConsentSubject {
    pub tenant_hash: Sha256Digest,
    pub subject_hash: Sha256Digest,
}

/// Consent validity window and replay nonce.
#[derive(Clone, Debug)]
pub struct ConsentWindow {
    pub issued_at: UnixMillis,
    pub not_before: UnixMillis,
    pub expires_at: UnixMillis,
    pub nonce_hash: Sha256Digest,
}

/// Profile facts bound into the consent that the authorized request does not
/// carry itself.
#[derive(Clone, Debug)]
pub struct ConsentProfile {
    pub model_capability_hash: Sha256Digest,
    pub budget_class: String,
}

/// Everything besides the consumed local request that the canonical
/// projection requires; each part must already be verified by its own
/// authority before it is offered here.
pub struct CanonicalProjectionInputs<'a> {
    pub registration: &'a RegistrationBinding,
    pub lease: &'a LeaseBinding,
    pub policy: &'a TenantPolicyBinding,
    pub subject: &'a ConsentSubject,
    pub window: &'a ConsentWindow,
    pub profile: &'a ConsentProfile,
    pub signer: &'a dyn InstallationConsentSigner,
}

/// Signs the canonical consent envelope hash with the local installation key.
pub trait InstallationConsentSigner: Send + Sync {
    /// Stable identifier of the installation signing key.
    fn key_id(&self) -> &str;

    /// Produces a base64url ES256 signature over the envelope hash.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError`] when the local key is unavailable.
    fn sign(&self, signed_payload_hash: &Sha256Digest) -> Result<String, CloudError>;
}

fn utc_instant(at: UnixMillis) -> Result<canonical::CommonUtcInstant, CloudError> {
    let millis = i128::from(at.0);
    let instant = OffsetDateTime::from_unix_timestamp_nanos(millis * 1_000_000)
        .map_err(|_| CloudError::CanonicalProjectionInvalid)?;
    let rendered = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        instant.year(),
        u8::from(instant.month()),
        instant.day(),
        instant.hour(),
        instant.minute(),
        instant.second(),
        instant.millisecond(),
    );
    canonical::CommonUtcInstant::from_str(&rendered)
        .map_err(|_| CloudError::CanonicalProjectionInvalid)
}

fn parse<T>(value: &str) -> Result<T, CloudError>
where
    T: FromStr,
{
    T::from_str(value).map_err(|_| CloudError::CanonicalProjectionInvalid)
}

fn digest<T>(value: Sha256Digest) -> Result<T, CloudError>
where
    T: FromStr,
{
    parse(&value.to_string())
}

fn classification_label(classification: ContextClassification) -> &'static str {
    match classification {
        ContextClassification::Public => "public",
        ContextClassification::Internal => "internal",
        ContextClassification::Confidential => "confidential",
    }
}

/// Serialized exactly like the canonical consent minus `consentEnvelopeHash`
/// and `proof`; the canonical envelope hash is computed over this draft.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsentEnvelopeDraft<'a> {
    schema_version: &'a str,
    decision_id: &'a str,
    request_id: &'a str,
    invocation_id: &'a str,
    delivery_model: &'a str,
    tenant_hash: String,
    subject_hash: String,
    registration_id: &'a str,
    installation_public_key_hash: String,
    entitlement_lease_id: &'a str,
    entitlement_lease_hash: String,
    tenant_policy_id: &'a str,
    tenant_policy_version: u64,
    tenant_policy_hash: String,
    purpose: &'a str,
    model_role: &'a str,
    canonical_output_schema_id: &'a str,
    canonical_output_schema_hash: String,
    manifest_hash: String,
    invocation_binding_hash: String,
    consumption_hash: String,
    consent_disclosure_hash: String,
    provider_profile_hash: String,
    model_profile_hash: String,
    model_capability_hash: String,
    deployment_hash: String,
    region: &'a str,
    retention_mode: &'a str,
    budget_class: &'a str,
    issued_at: String,
    not_before: String,
    expires_at: String,
    nonce_hash: String,
}

/// Consumes one authorized local request into the canonical support-plane
/// [`ModelAccessRequest`].
///
/// The projection fails closed when the tenant policy binding does not match
/// the policy hash sealed into the request, when any context item lacks a
/// language, when the retention mode is not the canonical transient mode, or
/// when any value cannot satisfy the canonical contract constraints.
///
/// # Errors
///
/// Returns [`CloudError::CanonicalProjectionInvalid`] on any mismatch;
/// the consumed request is dropped and cannot be retried.
#[expect(clippy::too_many_lines, reason = "one field-complete linear mapping")]
pub fn project_model_access_request(
    request: AuthorizedModelRequest,
    inputs: &CanonicalProjectionInputs<'_>,
) -> Result<ModelAccessRequest, CloudError> {
    let CanonicalProjectionInputs {
        registration,
        lease,
        policy,
        subject,
        window,
        profile,
        signer,
    } = inputs;
    let parts = request.into_transport_parts();
    if policy.policy_hash != parts.policy_hash {
        return Err(CloudError::CanonicalProjectionInvalid);
    }
    if parts.retention_mode != RetentionMode::TransientNoStore {
        return Err(CloudError::CanonicalProjectionInvalid);
    }
    if window.not_before < window.issued_at || window.expires_at <= window.not_before {
        return Err(CloudError::CanonicalProjectionInvalid);
    }

    let envelope_draft = ConsentEnvelopeDraft {
        schema_version: CONSENT_SCHEMA_VERSION,
        decision_id: parts.decision_id.as_str(),
        request_id: parts.request_id.as_str(),
        invocation_id: parts.invocation_id.as_str(),
        delivery_model: DELIVERY_MODEL,
        tenant_hash: subject.tenant_hash.to_string(),
        subject_hash: subject.subject_hash.to_string(),
        registration_id: registration.registration_id.as_str(),
        installation_public_key_hash: registration.installation_public_key_hash.to_string(),
        entitlement_lease_id: lease.lease_id.as_str(),
        entitlement_lease_hash: lease.lease_hash.to_string(),
        tenant_policy_id: policy.policy_id.as_str(),
        tenant_policy_version: policy.policy_version.get(),
        tenant_policy_hash: policy.policy_hash.to_string(),
        purpose: &parts.purpose,
        model_role: &parts.model_role,
        canonical_output_schema_id: parts.canonical_output_schema_id.as_str(),
        canonical_output_schema_hash: parts.canonical_output_schema_hash.to_string(),
        manifest_hash: parts.manifest_hash.to_string(),
        invocation_binding_hash: parts.binding_hash.to_string(),
        consumption_hash: parts.consumption_hash.to_string(),
        consent_disclosure_hash: parts.consent_disclosure_hash.to_string(),
        provider_profile_hash: parts.provider_profile_hash.to_string(),
        model_profile_hash: parts.model_profile_hash.to_string(),
        model_capability_hash: profile.model_capability_hash.to_string(),
        deployment_hash: parts.deployment_hash.to_string(),
        region: &parts.region,
        retention_mode: RETENTION_MODE,
        budget_class: &profile.budget_class,
        issued_at: String::from(&*utc_instant(window.issued_at)?),
        not_before: String::from(&*utc_instant(window.not_before)?),
        expires_at: String::from(&*utc_instant(window.expires_at)?),
        nonce_hash: window.nonce_hash.to_string(),
    };
    let envelope_hash = canonical_hash("model-context-consent", 1, &envelope_draft)
        .map_err(|_| CloudError::CanonicalProjectionInvalid)?;
    let signature = signer.sign(&envelope_hash)?;

    let proof = ModelContextConsentInstallationProof {
        proof_type: serde_json::Value::String(PROOF_TYPE.to_owned()),
        algorithm: serde_json::Value::String(PROOF_ALGORITHM.to_owned()),
        key_id: parse(signer.key_id())?,
        signed_payload_hash: digest(envelope_hash)?,
        signature: parse(&signature)?,
    };

    let consent = ModelContextConsent {
        schema_version: serde_json::Value::String(CONSENT_SCHEMA_VERSION.to_owned()),
        decision_id: parse(parts.decision_id.as_str())?,
        request_id: parse(parts.request_id.as_str())?,
        invocation_id: parse(parts.invocation_id.as_str())?,
        delivery_model: serde_json::Value::String(DELIVERY_MODEL.to_owned()),
        tenant_hash: digest(subject.tenant_hash)?,
        subject_hash: digest(subject.subject_hash)?,
        registration_id: parse(registration.registration_id.as_str())?,
        installation_public_key_hash: digest(registration.installation_public_key_hash)?,
        entitlement_lease_id: parse(lease.lease_id.as_str())?,
        entitlement_lease_hash: digest(lease.lease_hash)?,
        tenant_policy_id: parse(policy.policy_id.as_str())?,
        tenant_policy_version: NonZeroU64::new(policy.policy_version.get())
            .ok_or(CloudError::CanonicalProjectionInvalid)?,
        tenant_policy_hash: digest(policy.policy_hash)?,
        purpose: parse(&parts.purpose)?,
        model_role: parse(&parts.model_role)?,
        canonical_output_schema_id: parse(parts.canonical_output_schema_id.as_str())?,
        canonical_output_schema_hash: digest(parts.canonical_output_schema_hash)?,
        manifest_hash: digest(parts.manifest_hash)?,
        invocation_binding_hash: digest(parts.binding_hash)?,
        consumption_hash: digest(parts.consumption_hash)?,
        consent_disclosure_hash: digest(parts.consent_disclosure_hash)?,
        provider_profile_hash: digest(parts.provider_profile_hash)?,
        model_profile_hash: digest(parts.model_profile_hash)?,
        model_capability_hash: digest(profile.model_capability_hash)?,
        deployment_hash: digest(parts.deployment_hash)?,
        region: parse(&parts.region)?,
        retention_mode: serde_json::Value::String(RETENTION_MODE.to_owned()),
        budget_class: parse(&profile.budget_class)?,
        issued_at: utc_instant(window.issued_at)?,
        not_before: utc_instant(window.not_before)?,
        expires_at: utc_instant(window.expires_at)?,
        nonce_hash: digest(window.nonce_hash)?,
        consent_envelope_hash: digest(envelope_hash)?,
        proof,
    };

    let items = parts
        .items
        .into_iter()
        .map(|item| {
            let language = item
                .language
                .as_deref()
                .ok_or(CloudError::CanonicalProjectionInvalid)?;
            Ok(ModelAccessRequestContextItem {
                client_item_id: parse(item.client_item_id.as_str())?,
                relative_label: parse(item.relative_label.as_str())?,
                semantic_role: parse(&item.semantic_role)?,
                language: parse(language)?,
                content_hash: digest(item.content_hash)?,
                byte_count: i64::try_from(item.byte_count)
                    .map_err(|_| CloudError::CanonicalProjectionInvalid)?,
                classification: parse(classification_label(item.classification))?,
                content: parse(&item.content)?,
            })
        })
        .collect::<Result<Vec<_>, CloudError>>()?;

    Ok(ModelAccessRequest {
        schema_version: serde_json::Value::String(REQUEST_SCHEMA_VERSION.to_owned()),
        request_id: parse(parts.request_id.as_str())?,
        delivery_model: serde_json::Value::String(DELIVERY_MODEL.to_owned()),
        registration_id: parse(registration.registration_id.as_str())?,
        purpose: parse(&parts.purpose)?,
        model_role: parse(&parts.model_role)?,
        canonical_output_schema_id: parse(parts.canonical_output_schema_id.as_str())?,
        canonical_output_schema_hash: digest(parts.canonical_output_schema_hash)?,
        local_egress_manifest_hash: digest(parts.manifest_hash)?,
        consent,
        items,
        retention_mode: serde_json::Value::String(RETENTION_MODE.to_owned()),
        budget_class: parse(&profile.budget_class)?,
    })
}
