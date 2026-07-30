//! One-way version one to version two model access compatibility (FND-013).
//!
//! A desktop that still speaks `desktop-model-access-request.v1` keeps working
//! after version two lands. Its request is lifted onto a single named legacy
//! profile whose deployment stays resolved server side, exactly as it was
//! before profiles existed. The adapter is deliberately one way: there is no
//! path that lowers a version two request to version one, because that would
//! have to drop the profile, catalog epoch, variant, and request mode that
//! version two exists to bind.
//!
//! A version one request that reaches for a version two capability is refused
//! with a named reason rather than partially honoured. Silently ignoring a
//! `modelProfileId` a legacy client did not understand it was not allowed to
//! send is how a user ends up talking to a different model than the one the
//! screen says.
//!
//! The canonical policy this module implements is
//! `packages/contracts/schemas/model-access-compatibility.schema.json`, and
//! the same policy is implemented for the support plane in
//! `services/desktop-support-api/Model/ModelAccessCompatibility.cs`. The three
//! must agree; `packages/contracts/tests/model-access-migration.test.mjs`
//! pins the shared constants.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CloudError;

/// Wire discriminator of the legacy request document.
pub const LEGACY_REQUEST_SCHEMA_VERSION: &str = "desktop-model-access-request.v1";

/// Wire discriminator of the current request document.
pub const CURRENT_REQUEST_SCHEMA_VERSION: &str = "desktop-model-access-request.v2";

/// Wire discriminator of the legacy receipt document.
pub const LEGACY_RECEIPT_SCHEMA_VERSION: &str = "sapphirus.model-access-receipt.v1";

/// Wire discriminator of the current receipt document.
pub const CURRENT_RECEIPT_SCHEMA_VERSION: &str = "sapphirus.model-access-receipt.v2";

/// The single profile identity every adapted legacy request lands on.
///
/// It is an organization profile identifier like every other, so a version two
/// receipt for a legacy call still names a profile. Its deployment is resolved
/// server side and is never projected to the desktop, and it is not selectable
/// from the catalog, so no user can pick it.
pub const LEGACY_PROFILE_ID: &str = "legacy-fixed-deployment";

/// The request mode a legacy request is always adapted to. Version one had no
/// mode; it was structured output or nothing.
pub const LEGACY_REQUEST_MODE: &str = "structured_response";

/// The lowest desktop contract epoch still permitted to send version one.
/// An installation below this is refused and told to update rather than
/// adapted, because its receipt handling predates the fields the adapter emits.
pub const MINIMUM_DESKTOP_CONTRACT_EPOCH: u32 = 1;

/// The contract epoch at which version one stops being accepted. Reaching it
/// is necessary but not sufficient; the removal gate in the canonical policy
/// also requires the catalog, the installed base, and a replay check.
pub const REMOVAL_CONTRACT_EPOCH: u32 = 2;

/// Why a version one request was refused instead of adapted.
///
/// The variants mirror `LegacyRejectionReason` in the canonical compatibility
/// schema one for one, and the wire spelling is the snake case form the
/// support plane and the desktop both render.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyRejectionReason {
    /// The request named a model, profile, or deployment. Version one has no
    /// model selection; the server chooses.
    ModelSelectionAttempted,
    /// The request asked for streaming. Version one is request and response.
    StreamingRequested,
    /// The request named a variant. Legacy calls always run the profile
    /// default operating point.
    VariantRequested,
    /// The request carried an attachment beyond the text context items its own
    /// contract already allowed.
    AttachmentRequested,
    /// The request supplied a catalog epoch. A legacy client cannot have read
    /// a catalog, so a catalog version on a version one document is forged or
    /// replayed.
    CatalogVersionSupplied,
    /// The request declared tools. Version one has no tool lane.
    ToolDefinitionSupplied,
    /// The request carried a member outside the version one contract.
    UnknownMemberPresent,
    /// The sending installation is below the minimum desktop contract epoch.
    ContractEpochTooLow,
}

impl LegacyRejectionReason {
    /// The stable snake case wire spelling of this reason.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::ModelSelectionAttempted => "model_selection_attempted",
            Self::StreamingRequested => "streaming_requested",
            Self::VariantRequested => "variant_requested",
            Self::AttachmentRequested => "attachment_requested",
            Self::CatalogVersionSupplied => "catalog_version_supplied",
            Self::ToolDefinitionSupplied => "tool_definition_supplied",
            Self::UnknownMemberPresent => "unknown_member_present",
            Self::ContractEpochTooLow => "contract_epoch_too_low",
        }
    }
}

impl From<LegacyRejectionReason> for CloudError {
    /// Every refusal maps onto the crate's existing secret-free error surface.
    ///
    /// A refused legacy request is exactly a canonical projection that could
    /// not be constructed, which is what
    /// [`CloudError::CanonicalProjectionInvalid`] already means. The precise
    /// reason travels separately as a [`LegacyRejectionReason`] so the desktop
    /// can say which capability it reached for, without widening the error
    /// enum every consumer matches exhaustively.
    fn from(_reason: LegacyRejectionReason) -> Self {
        Self::CanonicalProjectionInvalid
    }
}

/// A version one request lifted into the shape version two resolution needs.
///
/// This is deliberately not the generated `ModelAccessRequestV2`: the adapter
/// cannot fill the catalog epoch or the consent decision, because a legacy
/// client never read a catalog and its consent was signed over the version one
/// envelope. The support plane stamps the current catalog epoch and re-binds
/// consent after the adapter has proved the request is adaptable at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdaptedLegacyRequest {
    /// Identifier carried through from the legacy request.
    pub request_id: String,
    /// Device registration carried through from the legacy request.
    pub registration_id: String,
    /// Declared purpose carried through from the legacy request.
    pub purpose: String,
    /// Declared model role carried through from the legacy request.
    pub model_role: String,
    /// Always [`LEGACY_PROFILE_ID`].
    pub model_profile_id: &'static str,
    /// Always [`LEGACY_REQUEST_MODE`].
    pub request_mode: &'static str,
    /// Always `None`: a legacy call runs the profile default operating point.
    pub requested_variant_id: Option<String>,
    /// Always `None`: the support plane stamps the current catalog epoch,
    /// because the desktop that sent this request never saw one.
    pub catalog_version: Option<String>,
    /// Canonical output schema carried through; version one always had one.
    pub canonical_output_schema_id: String,
    /// Canonical output schema hash carried through.
    pub canonical_output_schema_hash: String,
    /// Local egress manifest hash carried through.
    pub local_egress_manifest_hash: String,
    /// Budget class carried through.
    pub budget_class: String,
}

/// Members a legacy request document is allowed to carry.
const LEGACY_MEMBERS: [&str; 13] = [
    "schemaVersion",
    "requestId",
    "deliveryModel",
    "registrationId",
    "purpose",
    "modelRole",
    "canonicalOutputSchemaId",
    "canonicalOutputSchemaHash",
    "localEgressManifestHash",
    "consent",
    "items",
    "retentionMode",
    "budgetClass",
];

/// Members that name a specific refusal, checked before the catch-all so a
/// refusal always says which version two capability was reached for.
const NAMED_REFUSALS: [(&str, LegacyRejectionReason); 11] = [
    (
        "modelProfileId",
        LegacyRejectionReason::ModelSelectionAttempted,
    ),
    ("profileId", LegacyRejectionReason::ModelSelectionAttempted),
    (
        "deploymentName",
        LegacyRejectionReason::ModelSelectionAttempted,
    ),
    (
        "modelProfileHash",
        LegacyRejectionReason::ModelSelectionAttempted,
    ),
    (
        "requestedVariantId",
        LegacyRejectionReason::VariantRequested,
    ),
    ("variantId", LegacyRejectionReason::VariantRequested),
    (
        "catalogVersion",
        LegacyRejectionReason::CatalogVersionSupplied,
    ),
    ("tools", LegacyRejectionReason::ToolDefinitionSupplied),
    ("attachments", LegacyRejectionReason::AttachmentRequested),
    ("attachmentRefs", LegacyRejectionReason::AttachmentRequested),
    ("stream", LegacyRejectionReason::StreamingRequested),
];

fn required_str(document: &Value, member: &str) -> Result<String, LegacyRejectionReason> {
    document
        .get(member)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(LegacyRejectionReason::UnknownMemberPresent)
}

fn classify_streaming(document: &Value) -> Option<LegacyRejectionReason> {
    let mode = document.get("requestMode")?;
    if mode.as_str() == Some(LEGACY_REQUEST_MODE) {
        // A legacy document has no request mode at all. Naming the one mode it
        // could have had is still outside its contract, but it is not an
        // attempt to stream, so it is reported as what it is.
        return Some(LegacyRejectionReason::UnknownMemberPresent);
    }
    Some(LegacyRejectionReason::StreamingRequested)
}

/// Rejects a legacy request that reaches beyond the version one contract.
///
/// Returns the first named refusal, preferring a specific reason over the
/// catch-all so the desktop can explain which capability was refused.
fn classify_refusal(document: &Value) -> Option<LegacyRejectionReason> {
    if let Some(reason) = classify_streaming(document) {
        return Some(reason);
    }
    for (member, reason) in NAMED_REFUSALS {
        if document.get(member).is_some() {
            return Some(reason);
        }
    }
    let members = document.as_object()?;
    for member in members.keys() {
        if !LEGACY_MEMBERS.contains(&member.as_str()) {
            return Some(LegacyRejectionReason::UnknownMemberPresent);
        }
    }
    None
}

/// Lifts a version one request document onto the named legacy profile.
///
/// `contract_epoch` is the contract epoch the sending installation declared.
/// The document is the loosely parsed legacy envelope as received, not a
/// strictly validated one: the point of this adapter is to refuse the members
/// strict validation would only report as unknown, with a reason that says
/// which version two capability the caller reached for.
///
/// # Errors
///
/// Returns [`LegacyRejectionReason`] when the request is not adaptable:
/// when it attempts model selection, streaming, a variant, an attachment, a
/// catalog epoch, or tools; when it carries a member outside the version one
/// contract or omits a required one; when its schema version is not
/// [`LEGACY_REQUEST_SCHEMA_VERSION`]; or when the installation is below
/// [`MINIMUM_DESKTOP_CONTRACT_EPOCH`].
pub fn adapt_legacy_request(
    document: &Value,
    contract_epoch: u32,
) -> Result<AdaptedLegacyRequest, LegacyRejectionReason> {
    if contract_epoch < MINIMUM_DESKTOP_CONTRACT_EPOCH {
        return Err(LegacyRejectionReason::ContractEpochTooLow);
    }
    if document.get("schemaVersion").and_then(Value::as_str) != Some(LEGACY_REQUEST_SCHEMA_VERSION)
    {
        return Err(LegacyRejectionReason::UnknownMemberPresent);
    }
    if let Some(reason) = classify_refusal(document) {
        return Err(reason);
    }
    Ok(AdaptedLegacyRequest {
        request_id: required_str(document, "requestId")?,
        registration_id: required_str(document, "registrationId")?,
        purpose: required_str(document, "purpose")?,
        model_role: required_str(document, "modelRole")?,
        model_profile_id: LEGACY_PROFILE_ID,
        request_mode: LEGACY_REQUEST_MODE,
        requested_variant_id: None,
        catalog_version: None,
        canonical_output_schema_id: required_str(document, "canonicalOutputSchemaId")?,
        canonical_output_schema_hash: required_str(document, "canonicalOutputSchemaHash")?,
        local_egress_manifest_hash: required_str(document, "localEgressManifestHash")?,
        budget_class: required_str(document, "budgetClass")?,
    })
}

/// Reports whether a receipt of this schema version is still verifiable.
///
/// Version two adds a second receipt family; it does not restate or re-sign
/// the first. A receipt signed before version two landed keeps verifying with
/// the digest rule it was signed under, so nothing has to be re-issued.
#[must_use]
pub fn is_historical_receipt_verifiable(receipt_schema_version: &str) -> bool {
    matches!(
        receipt_schema_version,
        LEGACY_RECEIPT_SCHEMA_VERSION | CURRENT_RECEIPT_SCHEMA_VERSION
    )
}

/// Reports whether a request document version is still accepted.
///
/// Removing [`LEGACY_REQUEST_SCHEMA_VERSION`] from this answer is the
/// observable act that closes the legacy lane, and it is gated on
/// [`REMOVAL_CONTRACT_EPOCH`] plus the catalog and installed-base conditions
/// in the canonical compatibility policy.
#[must_use]
pub fn is_request_schema_version_accepted(request_schema_version: &str) -> bool {
    matches!(
        request_schema_version,
        LEGACY_REQUEST_SCHEMA_VERSION | CURRENT_REQUEST_SCHEMA_VERSION
    )
}

#[cfg(test)]
mod tests {
    use super::{
        adapt_legacy_request, is_historical_receipt_verifiable, is_request_schema_version_accepted,
        AdaptedLegacyRequest, LegacyRejectionReason, CURRENT_RECEIPT_SCHEMA_VERSION,
        CURRENT_REQUEST_SCHEMA_VERSION, LEGACY_PROFILE_ID, LEGACY_RECEIPT_SCHEMA_VERSION,
        LEGACY_REQUEST_MODE, MINIMUM_DESKTOP_CONTRACT_EPOCH, REMOVAL_CONTRACT_EPOCH,
    };
    use crate::CloudError;
    use serde_json::{json, Value};

    fn legacy_request() -> Value {
        json!({
            "schemaVersion": "desktop-model-access-request.v1",
            "requestId": "request_01J00000000000000000000000",
            "deliveryModel": "windows_local",
            "registrationId": "dreg_01J00000000000000000000000",
            "purpose": "bmad_help",
            "modelRole": "planner",
            "canonicalOutputSchemaId": "sapphirus.bmad-method-help-proposal.v1",
            "canonicalOutputSchemaHash": format!("sha256:{}", "6".repeat(64)),
            "localEgressManifestHash": format!("sha256:{}", "7".repeat(64)),
            "consent": { "schemaVersion": "sapphirus.model-context-consent.v1" },
            "items": [],
            "retentionMode": "transient_no_store",
            "budgetClass": "interactive-standard",
        })
    }

    #[test]
    fn legacy_request_lands_on_the_named_legacy_profile() {
        let expected = AdaptedLegacyRequest {
            request_id: "request_01J00000000000000000000000".to_owned(),
            registration_id: "dreg_01J00000000000000000000000".to_owned(),
            purpose: "bmad_help".to_owned(),
            model_role: "planner".to_owned(),
            model_profile_id: LEGACY_PROFILE_ID,
            request_mode: LEGACY_REQUEST_MODE,
            requested_variant_id: None,
            catalog_version: None,
            canonical_output_schema_id: "sapphirus.bmad-method-help-proposal.v1".to_owned(),
            canonical_output_schema_hash: format!("sha256:{}", "6".repeat(64)),
            local_egress_manifest_hash: format!("sha256:{}", "7".repeat(64)),
            budget_class: "interactive-standard".to_owned(),
        };
        assert_eq!(adapt_legacy_request(&legacy_request(), 1), Ok(expected));
    }

    #[test]
    fn the_adapter_never_lets_a_legacy_request_choose_a_model_or_a_variant() {
        for (member, value, expected) in [
            (
                "modelProfileId",
                json!("coding-balanced"),
                LegacyRejectionReason::ModelSelectionAttempted,
            ),
            (
                "deploymentName",
                json!("some-deployment"),
                LegacyRejectionReason::ModelSelectionAttempted,
            ),
            (
                "requestedVariantId",
                json!("deep"),
                LegacyRejectionReason::VariantRequested,
            ),
            (
                "catalogVersion",
                json!("v1.0123456789abcdef"),
                LegacyRejectionReason::CatalogVersionSupplied,
            ),
            (
                "tools",
                json!([{ "toolName": "read_workspace_file" }]),
                LegacyRejectionReason::ToolDefinitionSupplied,
            ),
            (
                "attachments",
                json!([{ "ref": "blob" }]),
                LegacyRejectionReason::AttachmentRequested,
            ),
            (
                "requestMode",
                json!("streaming_chat"),
                LegacyRejectionReason::StreamingRequested,
            ),
            (
                "stream",
                json!(true),
                LegacyRejectionReason::StreamingRequested,
            ),
            (
                "somethingElse",
                json!(1),
                LegacyRejectionReason::UnknownMemberPresent,
            ),
        ] {
            let mut document = legacy_request();
            document[member] = value;
            assert_eq!(
                adapt_legacy_request(&document, 1),
                Err(expected),
                "{member}"
            );
        }
    }

    #[test]
    fn a_version_two_request_is_never_lowered_to_version_one() {
        let mut document = legacy_request();
        document["schemaVersion"] = json!(CURRENT_REQUEST_SCHEMA_VERSION);

        assert_eq!(
            adapt_legacy_request(&document, 1),
            Err(LegacyRejectionReason::UnknownMemberPresent),
        );
    }

    #[test]
    fn an_installation_below_the_minimum_epoch_is_told_to_update() {
        assert_eq!(
            adapt_legacy_request(&legacy_request(), MINIMUM_DESKTOP_CONTRACT_EPOCH - 1),
            Err(LegacyRejectionReason::ContractEpochTooLow),
        );
        const { assert!(REMOVAL_CONTRACT_EPOCH > MINIMUM_DESKTOP_CONTRACT_EPOCH) }
    }

    #[test]
    fn historical_receipts_stay_verifiable_and_both_request_versions_are_accepted() {
        assert!(is_historical_receipt_verifiable(
            LEGACY_RECEIPT_SCHEMA_VERSION
        ));
        assert!(is_historical_receipt_verifiable(
            CURRENT_RECEIPT_SCHEMA_VERSION
        ));
        assert!(!is_historical_receipt_verifiable(
            "sapphirus.model-access-receipt.v3"
        ));
        assert!(is_request_schema_version_accepted(
            CURRENT_REQUEST_SCHEMA_VERSION
        ));
        assert!(!is_request_schema_version_accepted(
            "desktop-model-access-request.v3"
        ));
    }

    #[test]
    fn every_refusal_maps_onto_the_stable_secret_free_error_surface() {
        for reason in [
            LegacyRejectionReason::ModelSelectionAttempted,
            LegacyRejectionReason::StreamingRequested,
            LegacyRejectionReason::VariantRequested,
            LegacyRejectionReason::AttachmentRequested,
            LegacyRejectionReason::CatalogVersionSupplied,
            LegacyRejectionReason::ToolDefinitionSupplied,
            LegacyRejectionReason::UnknownMemberPresent,
            LegacyRejectionReason::ContractEpochTooLow,
        ] {
            assert_eq!(
                CloudError::from(reason),
                CloudError::CanonicalProjectionInvalid
            );
            assert!(!reason.as_wire_str().is_empty());
        }
    }
}
