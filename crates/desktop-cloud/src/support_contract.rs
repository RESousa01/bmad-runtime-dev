//! Canonical support-plane transport projection (D2-E Task 1).
//!
//! [`AuthorizedModelRequest`] is local authority and never crosses the wire.
//! The only path from local authority to the canonical [`ModelAccessRequest`]
//! envelope is [`project_model_access_request`], which consumes the request,
//! requires the current registration, verified lease and tenant policy
//! bindings, and an installation consent signer, and emits exactly the public
//! canonical shape — no local refs, decision internals, token estimates, or
//! redaction details.

use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::str::FromStr;

use desktop_egress::{
    local_egress_manifest_hash_v2, ContextClassification, LocalEgressManifestV2Input,
    LocalEgressManifestV2Item, LocalEgressManifestV2Tool, ModelInvocationBindingV2,
    ModelRequestMode, RetentionMode,
};
use desktop_runtime::generated_contracts as canonical;
use desktop_runtime::{canonical_hash, ContractId, Sha256Digest, UnixMillis};
use serde::Serialize;
use time::OffsetDateTime;

use crate::model::AuthorizedModelRequest;
use crate::model_catalog::{ResolvedModelSelection, VerifiedModelCatalog};
use crate::CloudError;

pub use canonical::{
    ModelAccessRequest, ModelAccessRequestContextItem, ModelAccessRequestV2, ModelContextConsent,
    ModelContextConsentInstallationProof,
};

const REQUEST_SCHEMA_VERSION: &str = "desktop-model-access-request.v1";
const CONSENT_SCHEMA_VERSION: &str = "sapphirus.model-context-consent.v1";
const REQUEST_SCHEMA_VERSION_V2: &str = "desktop-model-access-request.v2";
const CONSENT_SCHEMA_VERSION_V2: &str = "sapphirus.model-context-consent.v2";
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

/// One tool the desktop declares for a version two request.
///
/// A tool names a canonical parameter schema and its hash rather than carrying
/// an inline provider parameter document, so a request cannot smuggle a
/// provider-shaped payload through a tool definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredModelTool {
    pub tool_name: String,
    pub description: String,
    pub parameters_schema_id: ContractId,
    pub parameters_schema_hash: Sha256Digest,
}

/// What one request would actually ask a model to do.
///
/// This is the shape [`validate_model_capabilities`] checks *before* review is
/// shown, so a user is never asked to approve context for a profile that cannot
/// accept it.
#[derive(Clone, Copy, Debug)]
pub struct RequestedModelWork<'a> {
    pub request_mode: ModelRequestMode,
    /// Number of reviewed context parts.
    pub context_item_count: usize,
    /// Exact bytes that would leave the machine as context.
    pub total_outbound_bytes: u64,
    /// The desktop's conservative input-token estimate.
    pub total_token_estimate: u64,
    /// Bytes of non-text attachment, if any.
    pub attachment_bytes: u64,
    pub uses_image_input: bool,
    pub uses_pdf_input: bool,
    pub tools: &'a [DeclaredModelTool],
    /// Whether a canonical output schema must be satisfied.
    pub requires_structured_output: bool,
    /// The output budget the caller asked for, when it asked for one.
    pub requested_output_tokens: Option<NonZeroU64>,
}

/// Why a selected profile cannot serve a request.
///
/// Deliberately its own taxonomy rather than a [`CloudError`]: this is a local
/// pre-review refusal that has a specific, user-explainable cause, and
/// collapsing every cause into one transport-shaped error is what would make the
/// review surface unable to say which limit was hit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelCapabilityRejection {
    /// The selection no longer names a selectable profile in this catalog.
    ProfileNotSelectable,
    /// The selection names a variant the profile no longer offers.
    VariantNotOffered,
    /// The selection was made against a different catalog than the one offered.
    CatalogMismatch,
    /// The profile cannot accept text context at all.
    TextInputUnsupported,
    /// The request carries an image and the profile cannot read one.
    ImageInputUnsupported,
    /// The request carries a PDF and the profile cannot read one.
    PdfInputUnsupported,
    /// The request carries an attachment and the profile takes none.
    AttachmentsUnsupported,
    /// The request declares tools and the profile cannot call them.
    ToolCallingUnsupported,
    /// The request needs a canonical output schema and the profile cannot
    /// produce structured output.
    StructuredOutputUnsupported,
    /// The request asks for streaming chat, which produces no structured output
    /// and therefore may not carry an output schema.
    RequestModeConflict,
    /// The request has no context at all.
    NoContext,
    /// More context than the profile's context window can hold.
    ContextTokensExceeded,
    /// More input tokens than the profile accepts.
    InputTokensExceeded,
    /// A larger output budget than the profile allows.
    OutputTokensExceeded,
    /// A larger attachment than the profile allows.
    AttachmentBytesExceeded,
    /// The declared tool set is malformed: an empty or duplicated name.
    ToolSetInvalid,
}

/// Validates one selected profile against the work a request would ask of it.
///
/// # Why this runs before review
///
/// Review is the moment a user authorizes exact bytes to leave the machine. A
/// refusal that arrives after that authorization has already spent the user's
/// attention on a decision that could never have been honored, and -- because
/// the consent decision is single-use -- spending it costs a whole approval
/// cycle. Every check here is answerable from the verified catalog and the
/// request itself, so all of them belong in front of the review surface.
///
/// The profile's *deployment* is deliberately not consulted: the desktop does
/// not know one. Everything checked here comes from the sanitized catalog
/// projection.
///
/// # Errors
///
/// Returns the specific [`ModelCapabilityRejection`] for the first limit or
/// capability the request violates.
pub fn validate_model_capabilities(
    catalog: &VerifiedModelCatalog,
    selection: &ResolvedModelSelection,
    work: &RequestedModelWork<'_>,
) -> Result<(), ModelCapabilityRejection> {
    if selection.catalog_version() != catalog.catalog_version()
        || selection.catalog_hash() != catalog.catalog_hash()
    {
        return Err(ModelCapabilityRejection::CatalogMismatch);
    }
    let profile = catalog
        .catalog()
        .profiles
        .iter()
        .find(|profile| profile.profile_id.as_str() == selection.profile_id())
        .ok_or(ModelCapabilityRejection::ProfileNotSelectable)?;
    let variant = profile
        .variants
        .iter()
        .find(|variant| variant.variant_id.as_str() == selection.variant_id())
        .ok_or(ModelCapabilityRejection::VariantNotOffered)?;

    if work.context_item_count == 0 || work.total_outbound_bytes == 0 {
        return Err(ModelCapabilityRejection::NoContext);
    }

    let capabilities = &profile.capabilities;
    if !capabilities.text_input {
        return Err(ModelCapabilityRejection::TextInputUnsupported);
    }
    if work.uses_image_input && !capabilities.image_input {
        return Err(ModelCapabilityRejection::ImageInputUnsupported);
    }
    if work.uses_pdf_input && !capabilities.pdf_input {
        return Err(ModelCapabilityRejection::PdfInputUnsupported);
    }
    if work.attachment_bytes > 0 && !capabilities.attachments {
        return Err(ModelCapabilityRejection::AttachmentsUnsupported);
    }
    if !work.tools.is_empty() && !capabilities.tool_calling {
        return Err(ModelCapabilityRejection::ToolCallingUnsupported);
    }
    if work.requires_structured_output && !capabilities.structured_output {
        return Err(ModelCapabilityRejection::StructuredOutputUnsupported);
    }
    // Streaming chat produces plain text. A request that both streams and
    // demands a canonical output schema is asking for two different results, and
    // the support plane would have to pick one silently.
    if work.requires_structured_output && work.request_mode == ModelRequestMode::StreamingChat {
        return Err(ModelCapabilityRejection::RequestModeConflict);
    }

    let mut seen_tools = BTreeSet::new();
    for tool in work.tools {
        if tool.tool_name.is_empty() || !seen_tools.insert(tool.tool_name.as_str()) {
            return Err(ModelCapabilityRejection::ToolSetInvalid);
        }
    }

    let limits = &profile.limits;
    if let Some(context_tokens) = limits.context_tokens {
        if work.total_token_estimate > context_tokens.get() {
            return Err(ModelCapabilityRejection::ContextTokensExceeded);
        }
    }
    if let Some(max_input_tokens) = limits.max_input_tokens {
        if work.total_token_estimate > max_input_tokens.get() {
            return Err(ModelCapabilityRejection::InputTokensExceeded);
        }
    }
    if let Some(max_attachment_bytes) = limits.max_attachment_bytes {
        let allowed = u64::try_from(max_attachment_bytes).unwrap_or(0);
        if work.attachment_bytes > allowed {
            return Err(ModelCapabilityRejection::AttachmentBytesExceeded);
        }
    }
    if let Some(requested) = work.requested_output_tokens {
        // The variant's operating point is the tighter of the two ceilings and
        // is the one the support plane will actually apply, so both are checked.
        if requested > variant.max_output_tokens {
            return Err(ModelCapabilityRejection::OutputTokensExceeded);
        }
        if let Some(profile_ceiling) = limits.max_output_tokens {
            if requested > profile_ceiling {
                return Err(ModelCapabilityRejection::OutputTokensExceeded);
            }
        }
    }
    Ok(())
}

/// Everything besides the consumed local request that the version two canonical
/// projection requires.
pub struct CanonicalProjectionInputsV2<'a> {
    pub registration: &'a RegistrationBinding,
    pub lease: &'a LeaseBinding,
    pub policy: &'a TenantPolicyBinding,
    pub subject: &'a ConsentSubject,
    pub window: &'a ConsentWindow,
    /// The sealed version two selection layer. It carries the profile, catalog
    /// epoch, variant, request mode, context snapshot, capability hash, and the
    /// version two local egress manifest hash the support plane can recompute.
    pub selection: &'a ModelInvocationBindingV2,
    /// The version two consumption hash, derived from the single-use consumption
    /// that authorized this request. See
    /// `ModelInvocationBindingV2::consumption_hash_for`.
    pub consumption_hash: Sha256Digest,
    /// Tools the model may call.
    pub tools: &'a [DeclaredModelTool],
    pub signer: &'a dyn InstallationConsentSigner,
}

/// Serialized exactly like the canonical version two consent minus
/// `consentEnvelopeHash` and `proof`.
///
/// The three nullable members are `Option`s without `skip_serializing_if`, so
/// they are written as JSON `null` rather than omitted -- matching
/// `InstallationModelStreamConsentVerifier.ComputeEnvelopeHash` exactly. A
/// decision that names no variant and one that names the literal string `"null"`
/// are therefore different preimages.
///
/// There is no `deploymentHash`: the desktop no longer knows a deployment.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConsentEnvelopeDraftV2<'a> {
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
    request_mode: &'a str,
    model_profile_id: &'a str,
    requested_variant_id: Option<&'a str>,
    catalog_version: &'a str,
    context_snapshot_id: &'a str,
    canonical_output_schema_id: Option<&'a str>,
    canonical_output_schema_hash: Option<String>,
    manifest_hash: String,
    invocation_binding_hash: String,
    consumption_hash: String,
    consent_disclosure_hash: String,
    provider_profile_hash: String,
    model_profile_hash: String,
    model_capability_hash: String,
    region: &'a str,
    retention_mode: &'a str,
    budget_class: &'a str,
    issued_at: String,
    not_before: String,
    expires_at: String,
    nonce_hash: String,
}

const fn request_mode_label(mode: ModelRequestMode) -> &'static str {
    match mode {
        ModelRequestMode::StructuredResponse => "structured_response",
        ModelRequestMode::StreamingChat => "streaming_chat",
    }
}

/// Consumes one authorized local request into the canonical version two
/// [`ModelAccessRequestV2`].
///
/// # What version two changes
///
/// The desktop names a profile and a catalog epoch; it no longer names a
/// deployment, and there is no property on the wire that could carry one. The
/// wire hashes -- invocation binding, consent disclosure, consumption, and the
/// local egress manifest -- are all re-derived over the selection, so a profile,
/// catalog, variant, request-mode, context-snapshot, or capability change
/// invalidates an approval that was given for something else.
///
/// The local egress manifest hash is *recomputed here* from the exact projected
/// items and the selection, and compared against the one sealed into the
/// selection layer. That is what makes it a hash the support plane can verify
/// rather than a number the client asserted.
///
/// # Errors
///
/// Returns [`CloudError::CanonicalProjectionInvalid`] when the tenant policy
/// binding does not match the policy sealed into the request, the retention mode
/// is not the canonical transient mode, the consent window is not ordered, any
/// context item lacks a language, the recomputed version two manifest hash does
/// not match the sealed one, or any value cannot satisfy the canonical contract
/// constraints. The consumed request is dropped and cannot be retried.
#[expect(clippy::too_many_lines, reason = "one field-complete linear mapping")]
pub fn project_model_access_request_v2(
    request: AuthorizedModelRequest,
    inputs: &CanonicalProjectionInputsV2<'_>,
) -> Result<ModelAccessRequestV2, CloudError> {
    let CanonicalProjectionInputsV2 {
        registration,
        lease,
        policy,
        subject,
        window,
        selection,
        consumption_hash,
        tools,
        signer,
    } = inputs;
    let parts = request.into_transport_parts();
    if policy.policy_hash != parts.policy_hash
        || parts.retention_mode != RetentionMode::TransientNoStore
        || window.not_before < window.issued_at
        || window.expires_at <= window.not_before
    {
        return Err(CloudError::CanonicalProjectionInvalid);
    }
    let facts = selection.selection();

    // Streaming chat produces no structured output, so the schema pair is
    // absent; a structured response must carry both halves.
    let structured = facts.request_mode == ModelRequestMode::StructuredResponse;
    let (schema_id, schema_hash) = if structured {
        (
            Some(parts.canonical_output_schema_id.clone()),
            Some(parts.canonical_output_schema_hash),
        )
    } else {
        (None, None)
    };

    let manifest_items = parts
        .items
        .iter()
        .map(|item| {
            let language = item
                .language
                .clone()
                .ok_or(CloudError::CanonicalProjectionInvalid)?;
            Ok(LocalEgressManifestV2Item {
                client_item_id: item.client_item_id.clone(),
                relative_label: item.relative_label.clone(),
                semantic_role: item.semantic_role.clone(),
                language,
                content_hash: item.content_hash,
                byte_count: item.byte_count,
                classification: item.classification,
            })
        })
        .collect::<Result<Vec<_>, CloudError>>()?;
    let recomputed_manifest_hash = local_egress_manifest_hash_v2(&LocalEgressManifestV2Input {
        request_id: parts.request_id.clone(),
        registration_id: registration.registration_id.clone(),
        purpose: parts.purpose.clone(),
        model_role: parts.model_role.clone(),
        request_mode: facts.request_mode,
        model_profile_id: facts.model_profile_id.clone(),
        requested_variant_id: facts.requested_variant_id.clone(),
        catalog_version: facts.catalog_version.clone(),
        model_profile_hash: facts.model_profile_hash,
        model_capability_hash: facts.model_capability_hash,
        context_snapshot_id: facts.context_snapshot_id.clone(),
        canonical_output_schema_id: schema_id.clone(),
        canonical_output_schema_hash: schema_hash,
        retention_mode: RetentionMode::TransientNoStore,
        budget_class: facts.budget_class.clone(),
        items: manifest_items,
        tools: tools
            .iter()
            .map(|tool| LocalEgressManifestV2Tool {
                tool_name: tool.tool_name.clone(),
                description: tool.description.clone(),
                parameters_schema_id: tool.parameters_schema_id.clone(),
                parameters_schema_hash: tool.parameters_schema_hash,
            })
            .collect(),
    })
    .map_err(|_| CloudError::CanonicalProjectionInvalid)?;
    if recomputed_manifest_hash != selection.local_egress_manifest_hash() {
        return Err(CloudError::CanonicalProjectionInvalid);
    }

    let envelope_draft = ConsentEnvelopeDraftV2 {
        schema_version: CONSENT_SCHEMA_VERSION_V2,
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
        request_mode: request_mode_label(facts.request_mode),
        model_profile_id: &facts.model_profile_id,
        requested_variant_id: facts.requested_variant_id.as_deref(),
        catalog_version: &facts.catalog_version,
        context_snapshot_id: facts.context_snapshot_id.as_str(),
        canonical_output_schema_id: schema_id.as_ref().map(ContractId::as_str),
        canonical_output_schema_hash: schema_hash.map(|hash| hash.to_string()),
        manifest_hash: recomputed_manifest_hash.to_string(),
        invocation_binding_hash: selection.invocation_binding_hash().to_string(),
        consumption_hash: consumption_hash.to_string(),
        consent_disclosure_hash: selection.consent_disclosure_hash().to_string(),
        provider_profile_hash: parts.provider_profile_hash.to_string(),
        model_profile_hash: facts.model_profile_hash.to_string(),
        model_capability_hash: facts.model_capability_hash.to_string(),
        region: &parts.region,
        retention_mode: RETENTION_MODE,
        budget_class: &facts.budget_class,
        issued_at: String::from(&*utc_instant(window.issued_at)?),
        not_before: String::from(&*utc_instant(window.not_before)?),
        expires_at: String::from(&*utc_instant(window.expires_at)?),
        nonce_hash: window.nonce_hash.to_string(),
    };
    let envelope_hash = canonical_hash("model-context-consent", 2, &envelope_draft)
        .map_err(|_| CloudError::CanonicalProjectionInvalid)?;
    let signature = signer.sign(&envelope_hash)?;

    let consent = canonical::ModelAccessRequestV2ModelContextConsentV2 {
        schema_version: serde_json::Value::String(CONSENT_SCHEMA_VERSION_V2.to_owned()),
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
        request_mode: parse(request_mode_label(facts.request_mode))?,
        model_profile_id: parse(&facts.model_profile_id)?,
        requested_variant_id: facts
            .requested_variant_id
            .as_deref()
            .map(parse)
            .transpose()?,
        catalog_version: parse(&facts.catalog_version)?,
        context_snapshot_id: parse(facts.context_snapshot_id.as_str())?,
        canonical_output_schema_id: schema_id
            .as_ref()
            .map(|id| parse(id.as_str()))
            .transpose()?,
        canonical_output_schema_hash: schema_hash.map(digest).transpose()?,
        manifest_hash: digest(recomputed_manifest_hash)?,
        invocation_binding_hash: digest(selection.invocation_binding_hash())?,
        consumption_hash: digest(*consumption_hash)?,
        consent_disclosure_hash: digest(selection.consent_disclosure_hash())?,
        provider_profile_hash: digest(parts.provider_profile_hash)?,
        model_profile_hash: digest(facts.model_profile_hash)?,
        model_capability_hash: digest(facts.model_capability_hash)?,
        region: parse(&parts.region)?,
        retention_mode: serde_json::Value::String(RETENTION_MODE.to_owned()),
        budget_class: parse(&facts.budget_class)?,
        issued_at: utc_instant(window.issued_at)?,
        not_before: utc_instant(window.not_before)?,
        expires_at: utc_instant(window.expires_at)?,
        nonce_hash: digest(window.nonce_hash)?,
        consent_envelope_hash: digest(envelope_hash)?,
        proof: canonical::ModelAccessRequestV2ConsentInstallationProof {
            proof_type: serde_json::Value::String(PROOF_TYPE.to_owned()),
            algorithm: serde_json::Value::String(PROOF_ALGORITHM.to_owned()),
            key_id: parse(signer.key_id())?,
            signed_payload_hash: digest(envelope_hash)?,
            signature: parse(&signature)?,
        },
    };

    let items = parts
        .items
        .into_iter()
        .map(|item| {
            let language = item
                .language
                .as_deref()
                .ok_or(CloudError::CanonicalProjectionInvalid)?;
            Ok(canonical::ModelAccessRequestV2ModelContextItem {
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

    Ok(ModelAccessRequestV2 {
        schema_version: serde_json::Value::String(REQUEST_SCHEMA_VERSION_V2.to_owned()),
        request_id: parse(parts.request_id.as_str())?,
        delivery_model: serde_json::Value::String(DELIVERY_MODEL.to_owned()),
        registration_id: parse(registration.registration_id.as_str())?,
        purpose: parse(&parts.purpose)?,
        model_role: parse(&parts.model_role)?,
        request_mode: parse(request_mode_label(facts.request_mode))?,
        model_profile_id: parse(&facts.model_profile_id)?,
        requested_variant_id: facts
            .requested_variant_id
            .as_deref()
            .map(parse)
            .transpose()?,
        catalog_version: parse(&facts.catalog_version)?,
        model_profile_hash: digest(facts.model_profile_hash)?,
        model_capability_hash: digest(facts.model_capability_hash)?,
        context_snapshot_id: parse(facts.context_snapshot_id.as_str())?,
        canonical_output_schema_id: schema_id
            .as_ref()
            .map(|id| parse(id.as_str()))
            .transpose()?,
        canonical_output_schema_hash: schema_hash.map(digest).transpose()?,
        local_egress_manifest_hash: digest(recomputed_manifest_hash)?,
        consent,
        items,
        tools: tools
            .iter()
            .map(|tool| {
                Ok(canonical::ModelAccessRequestV2ModelToolDefinition {
                    tool_name: parse(&tool.tool_name)?,
                    description: parse(&tool.description)?,
                    parameters_schema_id: parse(tool.parameters_schema_id.as_str())?,
                    parameters_schema_hash: digest(tool.parameters_schema_hash)?,
                })
            })
            .collect::<Result<Vec<_>, CloudError>>()?,
        retention_mode: serde_json::Value::String(RETENTION_MODE.to_owned()),
        budget_class: parse(&facts.budget_class)?,
    })
}
