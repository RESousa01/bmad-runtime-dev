use crate::{AuthorizedModelRequest, CloudError, DispatchedModelRequest, RawModelOutput};

/// Explicit fail-closed composition for installations without connected model
/// access. It consumes the one-shot request without attempting transport.
#[cfg_attr(
    not(feature = "deterministic-fake"),
    doc = "\nDeterministic output is unavailable in default/production builds:\n\n```compile_fail\nuse desktop_cloud::DeterministicModelTransport;\n```"
)]
#[derive(Debug, Default)]
pub struct OfflineModelTransport;

impl OfflineModelTransport {
    /// Rejects a model request without exposing any context bytes.
    ///
    /// # Errors
    ///
    /// Always returns [`CloudError::Offline`].
    pub fn send(
        &self,
        _request: AuthorizedModelRequest,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        Err(CloudError::Offline)
    }
}

#[cfg(feature = "deterministic-fake")]
mod deterministic {
    use desktop_runtime::{sha256_bytes, ContractId, UnixMillis};

    use crate::{
        AuthorizedModelRequest, CloudError, DispatchedModelRequest, ModelAccessReceipt,
        ModelReceiptStatus, RawModelOutput,
    };

    /// Explicit test/demo transport. This type is absent from default and
    /// production builds and never acts as a fallback for the HTTPS transport.
    #[derive(Debug, Default)]
    pub struct DeterministicModelTransport;

    impl DeterministicModelTransport {
        /// Produces deterministic, locally marked output for tests and demos.
        ///
        /// # Errors
        ///
        /// Returns [`CloudError`] when the request capability is invalid or the
        /// deterministic fixture cannot be represented by bounded contracts.
        pub fn send(
            &self,
            request: AuthorizedModelRequest,
            now: UnixMillis,
        ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
            request.verify()?;
            let payload_json = serde_json::json!({
                "summary": "Deterministic planning preview",
                "purpose": request.purpose(),
                "modelRole": request.model_role(),
            })
            .to_string();
            let payload_hash = sha256_bytes(payload_json.as_bytes());
            let output_bytes =
                u64::try_from(payload_json.len()).map_err(|_| CloudError::InvalidModelOutput)?;
            let receipt_id = ContractId::new(format!(
                "fake_receipt_{}",
                request.request_hash().to_string().replace(':', "_")
            ))
            .map_err(|_| CloudError::InvalidModelOutput)?;
            let response = RawModelOutput {
                request_id: request.request_id().clone(),
                output_schema_id: request.canonical_output_schema_id().clone(),
                payload_json,
                payload_hash,
                receipt: ModelAccessReceipt {
                    schema_version: "sapphirus.model-access-receipt.v1".to_owned(),
                    receipt_id,
                    request_id: request.request_id().clone(),
                    request_hash: request.request_hash(),
                    result_hash: payload_hash,
                    manifest_hash: request.manifest_hash(),
                    binding_hash: request.binding_hash(),
                    consumption_hash: request.consumption_hash(),
                    consent_disclosure_hash: request.consent_disclosure_hash(),
                    provider_profile_hash: request.provider_profile_hash(),
                    model_profile_hash: request.model_profile_hash(),
                    deployment_hash: request.deployment_hash(),
                    retention_mode: request.retention_mode(),
                    region: request.region().to_owned(),
                    input_bytes: request.total_outbound_bytes(),
                    output_bytes,
                    started_at: now,
                    completed_at: now,
                    status: ModelReceiptStatus::Succeeded,
                    proof: "deterministic-fake-no-trust".to_owned(),
                },
            };
            Ok((DispatchedModelRequest::new(request), response))
        }
    }
}

#[cfg(feature = "deterministic-fake")]
pub use deterministic::DeterministicModelTransport;
