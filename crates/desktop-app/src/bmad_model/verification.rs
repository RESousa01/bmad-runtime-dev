use desktop_cloud::{CanonicalOutputValidator, CloudError};
#[cfg(feature = "deterministic-help")]
use desktop_cloud::{
    ModelAccessReceipt, ReceiptProofVerifier, ReplaySafeReceiptVerifier, SystemReceiptClock,
};
use desktop_runtime::{validate_bmad_help_proposal_schema, ContractId, Sha256Digest};
use serde_json::Value;

#[derive(Clone, Debug)]
pub(super) struct BmadHelpProposalValidator {
    schema_id: ContractId,
    schema_hash: Sha256Digest,
}

impl BmadHelpProposalValidator {
    #[must_use]
    pub const fn new(schema_id: ContractId, schema_hash: Sha256Digest) -> Self {
        Self {
            schema_id,
            schema_hash,
        }
    }
}

impl CanonicalOutputValidator for BmadHelpProposalValidator {
    fn validate(
        &self,
        schema_id: &ContractId,
        schema_hash: Sha256Digest,
        payload: &Value,
    ) -> Result<(), CloudError> {
        if schema_id != &self.schema_id
            || schema_hash != self.schema_hash
            || validate_bmad_help_proposal_schema(payload).is_err()
        {
            return Err(CloudError::InvalidModelOutput);
        }
        Ok(())
    }
}

#[cfg(feature = "deterministic-help")]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct DeterministicReceiptProofVerifier;

#[cfg(feature = "deterministic-help")]
impl ReceiptProofVerifier for DeterministicReceiptProofVerifier {
    fn verify_proof(&self, receipt: &ModelAccessReceipt) -> Result<(), CloudError> {
        if receipt.proof != "deterministic-fake-no-trust" {
            return Err(CloudError::ReceiptInvalid);
        }
        Ok(())
    }
}

#[cfg(feature = "deterministic-help")]
pub(super) type DeterministicReceiptVerifier =
    ReplaySafeReceiptVerifier<DeterministicReceiptProofVerifier, SystemReceiptClock>;

#[cfg(feature = "deterministic-help")]
pub(super) fn deterministic_receipt_verifier() -> Result<DeterministicReceiptVerifier, CloudError> {
    ReplaySafeReceiptVerifier::new(
        DeterministicReceiptProofVerifier,
        SystemReceiptClock,
        5 * 60 * 1_000,
        30 * 1_000,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use desktop_cloud::{CanonicalOutputValidator, CloudError};
    #[cfg(feature = "deterministic-help")]
    use desktop_cloud::{ModelAccessReceipt, ReceiptProofVerifier};
    #[cfg(feature = "deterministic-help")]
    use desktop_egress::RetentionMode;
    #[cfg(feature = "deterministic-help")]
    use desktop_runtime::UnixMillis;
    use desktop_runtime::{sha256_bytes, ContractId};
    use serde_json::json;

    use super::BmadHelpProposalValidator;
    #[cfg(feature = "deterministic-help")]
    use super::DeterministicReceiptProofVerifier;

    fn id(value: &str) -> ContractId {
        ContractId::new(value).expect("qualified fixture identifier")
    }

    #[test]
    fn proposal_validator_accepts_only_the_exact_schema_identity_and_closed_shape() {
        let schema_id = id("sapphirus.bmad-method-help-proposal.v1");
        let schema_hash = sha256_bytes(b"exact schema closure");
        let validator = BmadHelpProposalValidator::new(schema_id.clone(), schema_hash);
        assert_eq!(
            validator.validate(
                &schema_id,
                schema_hash,
                &json!({
                    "proposalKind": "no_recommendation",
                    "reasonCode": "catalog_evidence_absent"
                }),
            ),
            Ok(())
        );
        assert_eq!(
            validator.validate(&id("wrong.schema.v1"), schema_hash, &json!({})),
            Err(CloudError::InvalidModelOutput)
        );
        assert_eq!(
            validator.validate(
                &schema_id,
                sha256_bytes(b"substituted closure"),
                &json!({"proposalKind": "invented"}),
            ),
            Err(CloudError::InvalidModelOutput)
        );
    }

    #[test]
    #[cfg(feature = "deterministic-help")]
    fn deterministic_proof_accepts_only_the_explicit_no_trust_marker() {
        let mut receipt = ModelAccessReceipt {
            schema_version: "sapphirus.model-access-receipt.v1".to_owned(),
            receipt_id: id("receipt_01J00000000000000000000000"),
            request_id: id("request_01J00000000000000000000000"),
            request_hash: sha256_bytes(b"request"),
            result_hash: sha256_bytes(b"result"),
            manifest_hash: sha256_bytes(b"manifest"),
            binding_hash: sha256_bytes(b"binding"),
            consumption_hash: sha256_bytes(b"consumption"),
            consent_disclosure_hash: sha256_bytes(b"disclosure"),
            provider_profile_hash: sha256_bytes(b"provider"),
            model_profile_hash: sha256_bytes(b"model"),
            deployment_hash: sha256_bytes(b"deployment"),
            retention_mode: RetentionMode::TransientNoStore,
            region: "localdev".to_owned(),
            input_bytes: 1,
            output_bytes: 1,
            started_at: UnixMillis(1_000),
            completed_at: UnixMillis(1_000),
            status: desktop_cloud::ModelReceiptStatus::Succeeded,
            proof: "deterministic-fake-no-trust".to_owned(),
        };
        assert_eq!(
            DeterministicReceiptProofVerifier.verify_proof(&receipt),
            Ok(())
        );
        receipt.proof = "forged".to_owned();
        assert_eq!(
            DeterministicReceiptProofVerifier.verify_proof(&receipt),
            Err(CloudError::ReceiptInvalid)
        );
    }
}
