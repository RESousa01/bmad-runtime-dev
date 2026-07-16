use desktop_egress::RetentionMode;
use desktop_runtime::{
    canonical_hash, BmadKernelError, BmadKernelErrorCode, BmadTrustedHelpModelProfile,
    BmadTrustedHelpModelProfileData, ContractId, Sha256Digest,
};
use serde::Serialize;

const OUTPUT_SCHEMA_ID: &str = "sapphirus.bmad-method-help-proposal.v1";
const REGION: &str = "localdev";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HelpModelMode {
    Offline,
    #[cfg(feature = "deterministic-help")]
    DeterministicDevelopment,
}

#[derive(Clone)]
pub(crate) struct HelpModelConfiguration {
    pub mode: HelpModelMode,
    pub destination_label: &'static str,
    pub region: &'static str,
    pub retention_mode: RetentionMode,
    pub canonical_output_schema_id: ContractId,
    pub trusted_profile: BmadTrustedHelpModelProfile,
    pub provider_profile_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub policy_hash: Sha256Digest,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NamedProfile<'a> {
    schema_version: &'static str,
    profile_id: &'a str,
    mode: &'a str,
}

pub(crate) fn current_help_model_configuration() -> Result<HelpModelConfiguration, BmadKernelError>
{
    #[cfg(feature = "deterministic-help")]
    let (mode, mode_name, destination_label, provider_id, model_id, deployment_id) = (
        HelpModelMode::DeterministicDevelopment,
        "deterministic_development",
        "Deterministic local model — development only",
        "deterministic-local",
        "bmad-help-fixture-v1",
        "localdev",
    );
    #[cfg(not(feature = "deterministic-help"))]
    let (mode, mode_name, destination_label, provider_id, model_id, deployment_id) = (
        HelpModelMode::Offline,
        "offline",
        "Model support unavailable",
        "offline",
        "unavailable",
        "unavailable",
    );

    let provider_profile_hash = named_hash(
        "bmad-help-provider-profile",
        "sapphirus.bmad-help-provider-profile.v1",
        provider_id,
        mode_name,
    )?;
    let model_profile_hash = named_hash(
        "bmad-help-model-profile",
        "sapphirus.bmad-help-model-profile.v1",
        model_id,
        mode_name,
    )?;
    let model_capability_hash = named_hash(
        "bmad-help-model-capability-profile",
        "sapphirus.bmad-help-model-capability-profile.v1",
        OUTPUT_SCHEMA_ID,
        mode_name,
    )?;
    let context_window_profile_hash = named_hash(
        "bmad-help-context-window-profile",
        "sapphirus.bmad-help-context-window-profile.v1",
        "4-items-65536-bytes-16000-tokens",
        mode_name,
    )?;
    let egress_profile_hash = named_hash(
        "bmad-help-egress-profile",
        "sapphirus.bmad-help-egress-profile.v1",
        "localdev-transient-no-store",
        mode_name,
    )?;
    let request_schema_hash = named_hash(
        "bmad-help-model-request-schema",
        "sapphirus.bmad-help-model-request-schema.v1",
        "sapphirus.bmad-help-model-request.v1",
        mode_name,
    )?;
    let deployment_hash = named_hash(
        "bmad-help-model-deployment",
        "sapphirus.bmad-help-model-deployment.v1",
        deployment_id,
        mode_name,
    )?;
    let policy_hash = named_hash(
        "bmad-help-model-policy",
        "sapphirus.bmad-help-model-policy.v1",
        "review-consume-once-verified-receipt",
        mode_name,
    )?;
    let trusted_profile =
        BmadTrustedHelpModelProfile::from_host_assertion(BmadTrustedHelpModelProfileData {
            provider_id: provider_id.to_owned(),
            model_id: model_id.to_owned(),
            deployment_id: deployment_id.to_owned(),
            model_profile_hash,
            model_capability_hash,
            context_window_profile_hash,
            egress_profile_hash,
            request_schema_hash,
        })?;

    Ok(HelpModelConfiguration {
        mode,
        destination_label,
        region: REGION,
        retention_mode: RetentionMode::TransientNoStore,
        canonical_output_schema_id: ContractId::new(OUTPUT_SCHEMA_ID)
            .map_err(|_| configuration_error())?,
        trusted_profile,
        provider_profile_hash,
        deployment_hash,
        policy_hash,
    })
}

fn named_hash(
    domain: &str,
    schema_version: &'static str,
    profile_id: &str,
    mode: &str,
) -> Result<Sha256Digest, BmadKernelError> {
    canonical_hash(
        domain,
        1,
        &NamedProfile {
            schema_version,
            profile_id,
            mode,
        },
    )
    .map_err(|_| configuration_error())
}

fn configuration_error() -> BmadKernelError {
    BmadKernelErrorCode::SealedHelpBindingMismatch.into()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{current_help_model_configuration, HelpModelMode};

    #[test]
    fn build_mode_is_explicit_and_never_implies_identity() {
        let configuration =
            current_help_model_configuration().expect("host-owned Help model configuration");

        #[cfg(feature = "deterministic-help")]
        {
            assert_eq!(configuration.mode, HelpModelMode::DeterministicDevelopment);
            assert_eq!(configuration.region, "localdev");
            assert_eq!(
                configuration.destination_label,
                "Deterministic local model — development only"
            );
        }
        #[cfg(not(feature = "deterministic-help"))]
        {
            assert_eq!(configuration.mode, HelpModelMode::Offline);
            assert_eq!(configuration.destination_label, "Model support unavailable");
        }
    }
}
