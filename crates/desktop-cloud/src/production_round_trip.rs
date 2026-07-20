//! The deployed production round trip (readiness Task 9): one reviewed
//! sequence from session bearer to verified receipt.
//!
//! Every server document is untrusted until the corresponding verifier
//! accepts it: the bootstrap contract gate, the registration shape check,
//! `accept_policy`, `accept_lease`, the transport's bounded model
//! exchange, and `accept_receipt_proof`. The orchestration is linear by
//! construction — a later stage cannot run without the earlier stage's
//! verified value — so reordering is structurally impossible on the
//! client, and each stage failure surfaces as a stable [`CloudError`].

use serde::{Deserialize, Serialize};

use crate::identity::{CloudSession, IdentityBroker};
use crate::model::{AuthorizedModelRequest, DispatchedModelRequest, RawModelOutput};
use crate::production::{
    CanonicalReceiptProof, ProductionSupportClient, SignedDesktopPolicyDoc,
    SignedEntitlementLeaseDoc,
};
use crate::transport::{HttpExecutor, SupportApiTransport, SupportStageRoute};
use crate::CloudError;
use desktop_runtime::{canonical_hash_without_field, ContractId, UnixMillis};

const BOOTSTRAP_SCHEMA: &str = "sapphirus.desktop-bootstrap.v1";
const REGISTRATION_SCHEMA: &str = "desktop-device-registration.v1";
const CONTRACT_EPOCH: &str = "1";

/// The reviewed registration material for this installation. The public
/// key identifies the installation signing key; nothing here is secret.
#[derive(Clone, Debug)]
pub struct RegistrationMaterial {
    pub installation_public_key: String,
    pub installation_public_key_hash: String,
    pub client_release: String,
    pub platform: String,
    pub architecture: String,
    pub tenant_policy_version: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BootstrapDoc {
    schema_version: String,
    region: String,
    contract_epoch: String,
    minimum_client_contract_epoch: String,
    capabilities: Vec<String>,
    #[serde(rename = "serverTime")]
    _server_time: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RegistrationRequestDoc<'a> {
    schema_version: &'static str,
    installation_public_key: &'a str,
    installation_public_key_hash: &'a str,
    client_release: &'a str,
    platform: &'a str,
    architecture: &'a str,
    tenant_policy_version: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistrationResponseDoc {
    schema_version: String,
    registration_id: String,
    status: String,
    #[serde(rename = "createdAt")]
    _created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LeaseRequestDoc<'a> {
    registration_id: &'a str,
}

/// The composed production round trip. Construction requires the exact
/// reviewed configuration to already exist in each component; nothing
/// here can degrade to deterministic or unsigned behavior.
pub struct ProductionRoundTrip<B, E> {
    session: CloudSession<B>,
    client: ProductionSupportClient,
    transport: SupportApiTransport<E>,
    registration: RegistrationMaterial,
}

impl<B, E> ProductionRoundTrip<B, E>
where
    B: IdentityBroker,
    E: HttpExecutor,
{
    #[must_use]
    pub const fn new(
        session: CloudSession<B>,
        client: ProductionSupportClient,
        transport: SupportApiTransport<E>,
        registration: RegistrationMaterial,
    ) -> Self {
        Self {
            session,
            client,
            transport,
            registration,
        }
    }

    #[must_use]
    pub const fn client(&self) -> &ProductionSupportClient {
        &self.client
    }

    /// Test-support accessor for scripted transports.
    #[doc(hidden)]
    #[must_use]
    pub const fn transport_for_test(&self) -> &SupportApiTransport<E> {
        &self.transport
    }

    /// Runs the complete reviewed sequence for one authorized request:
    /// bootstrap, registration, signed policy, signed lease, the single
    /// no-store model call, and receipt-proof verification.
    ///
    /// # Errors
    ///
    /// Fails closed with a stable [`CloudError`] at the first stage whose
    /// document cannot be verified; nothing later runs after a failure.
    pub async fn send(
        &self,
        request: AuthorizedModelRequest,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        let access = self.session.acquire_access(now).await?;
        let request_key = request.request_id().to_string();
        let registration_id = self.bootstrap_and_register(&access, &request_key).await?;
        self.policy_lease_and_call(&access, &request_key, registration_id, request, now)
            .await
    }

    async fn bootstrap_and_register(
        &self,
        access: &crate::identity::CloudAccess,
        request_key: &str,
    ) -> Result<ContractId, CloudError> {
        // Stage 1: bootstrap contract gate.
        let bootstrap_bytes = self
            .transport
            .exchange_stage(
                SupportStageRoute::Bootstrap,
                access.bearer_copy(),
                Vec::new(),
                &format!("{request_key}:bootstrap"),
            )
            .await?;
        let bootstrap: BootstrapDoc =
            serde_json::from_slice(&bootstrap_bytes).map_err(|_| CloudError::TransportFailed)?;
        if bootstrap.schema_version != BOOTSTRAP_SCHEMA
            || bootstrap.region != self.client.config().region
            || bootstrap.contract_epoch != CONTRACT_EPOCH
            || bootstrap.minimum_client_contract_epoch != CONTRACT_EPOCH
            || !bootstrap
                .capabilities
                .iter()
                .any(|capability| capability == "transient_no_store")
        {
            return Err(CloudError::TransportFailed);
        }

        // Stage 2: installation registration.
        let registration_body = serde_json::to_vec(&RegistrationRequestDoc {
            schema_version: REGISTRATION_SCHEMA,
            installation_public_key: &self.registration.installation_public_key,
            installation_public_key_hash: &self.registration.installation_public_key_hash,
            client_release: &self.registration.client_release,
            platform: &self.registration.platform,
            architecture: &self.registration.architecture,
            tenant_policy_version: self.registration.tenant_policy_version,
        })
        .map_err(|_| CloudError::TransportFailed)?;
        let registration_bytes = self
            .transport
            .exchange_stage(
                SupportStageRoute::DeviceRegistrations,
                access.bearer_copy(),
                registration_body,
                &format!("{request_key}:registration"),
            )
            .await?;
        let registration: RegistrationResponseDoc =
            serde_json::from_slice(&registration_bytes).map_err(|_| CloudError::TransportFailed)?;
        if registration.schema_version != REGISTRATION_SCHEMA || registration.status != "active" {
            return Err(CloudError::TransportFailed);
        }
        ContractId::new(registration.registration_id).map_err(|_| CloudError::TransportFailed)
    }

    async fn policy_lease_and_call(
        &self,
        access: &crate::identity::CloudAccess,
        request_key: &str,
        registration_id: ContractId,
        request: AuthorizedModelRequest,
        now: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        // Stage 3: signed policy.
        let policy_bytes = self
            .transport
            .exchange_stage(
                SupportStageRoute::PolicyCurrent,
                access.bearer_copy(),
                Vec::new(),
                &format!("{request_key}:policy"),
            )
            .await?;
        let policy_doc: SignedDesktopPolicyDoc =
            serde_json::from_slice(&policy_bytes).map_err(|_| CloudError::TransportFailed)?;
        let policy = self.client.accept_policy(policy_doc)?;

        // Stage 4: signed lease bound to the registration and policy.
        let lease_body = serde_json::to_vec(&LeaseRequestDoc {
            registration_id: registration_id.as_str(),
        })
        .map_err(|_| CloudError::TransportFailed)?;
        let lease_bytes = self
            .transport
            .exchange_stage(
                SupportStageRoute::EntitlementLeases,
                access.bearer_copy(),
                lease_body,
                &format!("{request_key}:lease"),
            )
            .await?;
        let lease_doc: SignedEntitlementLeaseDoc =
            serde_json::from_slice(&lease_bytes).map_err(|_| CloudError::TransportFailed)?;
        let lease = self
            .client
            .accept_lease(lease_doc, &policy, &registration_id, now)?;
        let entitlement = lease.entitlement(&policy)?;

        // Stage 5: the single bounded no-store model call.
        let (dispatched, output) = self
            .transport
            .send(&self.session, access, &entitlement, request, now)
            .await?;

        // Stage 6: receipt binding and proof verification with replay
        // protection.
        dispatched.verify_receipt_binding(&output)?;
        let proof: CanonicalReceiptProof =
            serde_json::from_str(&output.receipt.proof).map_err(|_| CloudError::ReceiptInvalid)?;
        let expected_receipt_hash =
            canonical_hash_without_field("model-access-receipt", 1, &output.receipt, "proof")
                .map_err(|_| CloudError::ReceiptInvalid)?
                .to_string();
        self.client.accept_receipt_proof(
            output.receipt.receipt_id.as_str(),
            &proof,
            &expected_receipt_hash,
        )?;

        Ok((dispatched, output))
    }
}
