#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod broker_protocol;
mod composition;
#[cfg(feature = "direct-provider")]
mod direct_provider;
mod entitlement;
mod identity;
mod installation_identity;
mod model;
mod production;
mod production_round_trip;
// Not gated on `direct-provider`. This is the taxonomy that says *why* a model
// call failed -- host request invalid, provider refused, transport broke -- and
// desktop-app uses it as the error type of the `ModelTransport::dispatch`
// trait, which every trust mode goes through. Gating it meant the crate only
// compiled when the direct provider was on, which the removed default feature
// hid. Its own dependencies are `model`, `transport` and `CloudError`, none of
// which are gated.
mod provider_failure;
mod support_contract;
mod transport;
#[cfg(windows)]
mod windows_broker;

pub use broker_protocol::{BrokerExchange, BrokerOutcome, BrokerProtocol};
#[cfg(feature = "deterministic-fake")]
pub use composition::DeterministicModelTransport;
pub use composition::OfflineModelTransport;
#[cfg(feature = "direct-provider")]
pub use direct_provider::{
    DirectProviderConfig, DirectProviderKind, DirectProviderTransport, ProviderAuth,
    DIRECT_PROVIDER_DEVELOPMENT_PROOF,
};
pub use entitlement::{EntitlementProofVerifier, EntitlementVerifier, VerifiedEntitlement};
pub use identity::{BrokerToken, CloudAccess, CloudSession, IdentityBroker, SessionSnapshot};
#[cfg(windows)]
pub use installation_identity::WindowsInstallationIdentity;
pub use installation_identity::{
    base64url_no_pad, consent_signature_payload, installation_key_id, p256_spki_from_point,
};
pub use model::{
    verify_dispatched_model_response, AuthorizedContextItem, AuthorizedModelRequest,
    CanonicalOutputValidator, DispatchedModelRequest, ModelAccessReceipt, ModelReceiptStatus,
    RawModelOutput, ReceiptClock, ReceiptProofVerifier, ReceiptVerifier, ReplaySafeReceiptVerifier,
    SystemReceiptClock, VerifiedModelOutput,
};
pub use production::{
    pinned_key_from_point, verify_canonical_receipt_proof, CanonicalReceiptProof, PinnedProofKey,
    ProductionSessionProjection, ProductionSupportClient, ProductionSupportConfig, ProofKeyRing,
    SignedDesktopPolicyDoc, SignedEntitlementLeaseDoc, SignedStateStore, VerifiedLease,
    VerifiedSignedPolicy,
};
pub use production_round_trip::{ProductionRoundTrip, RegistrationMaterial};
pub use provider_failure::{ProviderFailure, ProviderFailureKind, MAX_RETRY_AFTER};
pub use support_contract::{
    project_model_access_request, CanonicalProjectionInputs, ConsentProfile, ConsentSubject,
    ConsentWindow, InstallationConsentSigner, LeaseBinding,
    ModelAccessRequest as CanonicalModelAccessRequest,
    ModelAccessRequestContextItem as CanonicalModelAccessRequestContextItem,
    ModelContextConsent as CanonicalModelContextConsent, RegistrationBinding, TenantPolicyBinding,
};
pub use transport::{
    HttpExecutor, HttpMethod, HttpResponse, OutboundHttpRequest, ReqwestHttpExecutor,
    SupportApiOrigin, SupportApiTransport, SupportStageRoute,
};
#[cfg(windows)]
pub use windows_broker::{WindowsBrokerConfig, WindowsIdentityBroker};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    SignedOut,
    SigningIn,
    SignedIn,
    ReauthenticationRequired,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntitlementLease {
    pub schema_version: String,
    pub lease_id: String,
    pub registration_id: String,
    pub subject_hash: String,
    pub delivery_model: String,
    pub issued_at: String,
    pub not_before: String,
    pub expires_at: String,
    pub offline_grace_ends_at: String,
    pub features: Vec<String>,
    pub tenant_policy_hash: String,
    pub minimum_client_version: String,
    pub key_id: String,
    pub signature: String,
}

/// The stable, secret-free error surface of this crate.
///
/// `Clone` is derived so a classified provider failure can carry the exact
/// variant its lane already reads (see `ProviderFailure`), rather than
/// re-deriving one and changing what the BMAD help and capability coordinators
/// see. `Copy` is deliberately *not* derived: the existing consumers take
/// `&CloudError`, and a `Copy` type behind a reference is a clippy `pedantic`
/// finding at every one of those call sites.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CloudError {
    #[error("the desktop support plane is offline")]
    Offline,
    #[error("authentication is required")]
    AuthenticationRequired,
    #[error("the entitlement lease is unavailable or expired")]
    EntitlementUnavailable,
    #[error("the context manifest no longer matches the approved bytes")]
    ContextDrift,
    #[error("the typed model response failed canonical validation")]
    InvalidModelOutput,
    #[error("the requested connected capability is disabled")]
    FeatureDisabled,
    #[error("the identity broker is unavailable")]
    IdentityUnavailable,
    #[error("reauthentication is required")]
    ReauthenticationRequired,
    #[error("the authenticated tenant does not match the configured tenant")]
    TenantMismatch,
    #[error("the cloud session was invalidated")]
    SessionInvalidated,
    #[error("the consumed context decision does not match the model request")]
    ConsentBindingMismatch,
    #[error("the model response does not match the authorized request")]
    ResponseBindingMismatch,
    #[error("the model access receipt is invalid")]
    ReceiptInvalid,
    #[error("the configured support API origin is invalid")]
    InvalidSupportOrigin,
    #[error("the support API transport failed")]
    TransportFailed,
    #[error("the canonical transport projection could not be constructed")]
    CanonicalProjectionInvalid,
    #[error("the installation signing key is unavailable")]
    InstallationKeyUnavailable,
}
