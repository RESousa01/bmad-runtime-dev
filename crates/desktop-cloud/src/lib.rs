#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[cfg(feature = "deterministic-fake")]
use sha2::{Digest, Sha256};
use thiserror::Error;

mod identity;

pub use identity::{BrokerToken, CloudAccess, CloudSession, IdentityBroker, SessionSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Connectivity {
    Offline,
    Limited,
    Online,
}

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
#[serde(rename_all = "camelCase")]
pub struct EntitlementLease {
    pub lease_id: String,
    pub registration_id: String,
    pub subject_hash: String,
    pub issued_at: String,
    pub expires_at: String,
    pub offline_grace_ends_at: String,
    pub features: Vec<String>,
    pub tenant_policy_hash: String,
    pub minimum_client_version: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextItem {
    pub client_item_id: String,
    pub relative_label: String,
    pub semantic_role: String,
    pub content_hash: String,
    pub byte_count: u64,
    pub token_estimate: u64,
    pub classification: String,
    pub redactions: Vec<String>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAccessRequest {
    pub request_id: String,
    pub purpose: String,
    pub model_role: String,
    pub canonical_output_schema_id: String,
    pub canonical_output_schema_hash: String,
    pub local_egress_manifest_hash: String,
    pub consent_receipt_hash: String,
    pub provider_profile_hash: String,
    pub region: String,
    pub items: Vec<ContextItem>,
    pub retention_mode: RetentionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionMode {
    TransientNoStore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedModelOutput {
    pub request_id: String,
    pub schema_id: String,
    pub payload_json: String,
    pub payload_hash: String,
    pub receipt_hash: String,
    pub model_profile_hash: String,
}

#[derive(Debug, Eq, Error, PartialEq)]
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
}

#[async_trait]
pub trait DesktopCloudClient: Send + Sync {
    async fn connectivity(&self) -> Connectivity;
    async fn auth_status(&self) -> AuthStatus;
    async fn entitlement(&self) -> Result<EntitlementLease, CloudError>;
    async fn complete_model_call(
        &self,
        request: &ModelAccessRequest,
    ) -> Result<TypedModelOutput, CloudError>;
}

#[derive(Debug, Default)]
pub struct OfflineCloudClient;

#[async_trait]
impl DesktopCloudClient for OfflineCloudClient {
    async fn connectivity(&self) -> Connectivity {
        Connectivity::Offline
    }

    async fn auth_status(&self) -> AuthStatus {
        AuthStatus::SignedOut
    }

    async fn entitlement(&self) -> Result<EntitlementLease, CloudError> {
        Err(CloudError::Offline)
    }

    async fn complete_model_call(
        &self,
        _request: &ModelAccessRequest,
    ) -> Result<TypedModelOutput, CloudError> {
        Err(CloudError::Offline)
    }
}

#[cfg(feature = "deterministic-fake")]
#[derive(Debug, Default)]
pub struct DeterministicModelClient;

#[cfg(feature = "deterministic-fake")]
#[async_trait]
impl DesktopCloudClient for DeterministicModelClient {
    async fn connectivity(&self) -> Connectivity {
        Connectivity::Online
    }

    async fn auth_status(&self) -> AuthStatus {
        AuthStatus::SignedIn
    }

    async fn entitlement(&self) -> Result<EntitlementLease, CloudError> {
        Err(CloudError::FeatureDisabled)
    }

    async fn complete_model_call(
        &self,
        request: &ModelAccessRequest,
    ) -> Result<TypedModelOutput, CloudError> {
        let context_hash = canonical_context_hash(&request.items);
        if context_hash != request.local_egress_manifest_hash {
            return Err(CloudError::ContextDrift);
        }
        let payload_json = serde_json::json!({
            "summary": "Deterministic planning preview",
            "steps": [
                "Review the selected context",
                "Prepare a bounded change proposal",
                "Verify host-observed postimages"
            ]
        })
        .to_string();
        let payload_hash = sha256(payload_json.as_bytes());
        Ok(TypedModelOutput {
            request_id: request.request_id.clone(),
            schema_id: request.canonical_output_schema_id.clone(),
            payload_json,
            receipt_hash: sha256(
                format!("receipt:{}:{payload_hash}", request.request_id).as_bytes(),
            ),
            payload_hash,
            model_profile_hash: request.provider_profile_hash.clone(),
        })
    }
}

#[cfg(feature = "deterministic-fake")]
fn canonical_context_hash(items: &[ContextItem]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"sapphirus:context-egress:v1\n");
    for item in items {
        hasher.update(item.client_item_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(item.content_hash.as_bytes());
        hasher.update(b"\0");
        hasher.update(item.content.as_bytes());
        hasher.update(b"\n");
    }
    format!("sha256:{:x}", hasher.finalize())
}

#[cfg(feature = "deterministic-fake")]
fn sha256(value: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn offline_client_never_silently_falls_back() {
        let client = OfflineCloudClient;
        assert_eq!(client.connectivity().await, Connectivity::Offline);
        assert!(matches!(
            client.entitlement().await,
            Err(CloudError::Offline)
        ));
    }
}
