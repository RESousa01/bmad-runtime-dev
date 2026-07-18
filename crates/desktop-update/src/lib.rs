#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    Development,
    Beta,
    Stable,
    EnterpriseManaged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum UpdateState {
    Disabled,
    Idle,
    Checking,
    Current,
    Available { version: Version },
    Downloaded { version: Version },
    Blocked { reason: UpdateBlockReason },
    Failed { safe_message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateBlockReason {
    ManagedInstallation,
    ActiveEffectJournal,
    StoreRecoveryRequired,
    StoreIncompatible,
    InvalidMetadata,
    InvalidSignature,
    WrongChannel,
    WrongArchitecture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseMetadata {
    pub product: String,
    pub channel: ReleaseChannel,
    pub architecture: String,
    pub version: Version,
    pub minimum_store_schema: u32,
    pub maximum_store_schema: u32,
    pub artifact_sha256: String,
    pub tauri_signature: String,
    pub metadata_signature: String,
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("release metadata is not applicable to this installation")]
    Inapplicable,
    #[error("the installation is not currently safe to update")]
    UnsafeState,
    #[error("the update signature has not been verified")]
    SignatureRequired,
}

#[derive(Debug, Clone)]
pub struct UpdatePolicy {
    product: String,
    channel: ReleaseChannel,
    architecture: String,
    store_schema: u32,
}

impl UpdatePolicy {
    #[must_use]
    pub fn new(
        product: impl Into<String>,
        channel: ReleaseChannel,
        architecture: impl Into<String>,
        store_schema: u32,
    ) -> Self {
        Self {
            product: product.into(),
            channel,
            architecture: architecture.into(),
            store_schema,
        }
    }

    /// Evaluates whether a release can be offered to this installation.
    ///
    /// # Errors
    ///
    /// Returns [`UpdateError::Inapplicable`] when the release product, channel,
    /// or architecture does not match this installation.
    pub fn evaluate(
        &self,
        release: &ReleaseMetadata,
        signatures_verified: bool,
        has_active_journal: bool,
        recovery_required: bool,
    ) -> Result<UpdateState, UpdateError> {
        if self.channel == ReleaseChannel::EnterpriseManaged {
            return Ok(UpdateState::Blocked {
                reason: UpdateBlockReason::ManagedInstallation,
            });
        }
        if release.product != self.product
            || release.channel != self.channel
            || release.architecture != self.architecture
        {
            return Err(UpdateError::Inapplicable);
        }
        if !signatures_verified {
            return Ok(UpdateState::Blocked {
                reason: UpdateBlockReason::InvalidSignature,
            });
        }
        if has_active_journal {
            return Ok(UpdateState::Blocked {
                reason: UpdateBlockReason::ActiveEffectJournal,
            });
        }
        if recovery_required {
            return Ok(UpdateState::Blocked {
                reason: UpdateBlockReason::StoreRecoveryRequired,
            });
        }
        if self.store_schema < release.minimum_store_schema
            || self.store_schema > release.maximum_store_schema
        {
            return Ok(UpdateState::Blocked {
                reason: UpdateBlockReason::StoreIncompatible,
            });
        }
        Ok(UpdateState::Available {
            version: release.version.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> Result<ReleaseMetadata, semver::Error> {
        Ok(ReleaseMetadata {
            product: "sapphirus-desktop".to_owned(),
            channel: ReleaseChannel::Beta,
            architecture: "x64".to_owned(),
            version: Version::parse("0.2.0-beta.1")?,
            minimum_store_schema: 1,
            maximum_store_schema: 1,
            artifact_sha256: "sha256:test".to_owned(),
            tauri_signature: "test".to_owned(),
            metadata_signature: "test".to_owned(),
        })
    }

    #[test]
    fn blocks_update_while_effect_journal_is_active() -> Result<(), Box<dyn std::error::Error>> {
        let policy = UpdatePolicy::new("sapphirus-desktop", ReleaseChannel::Beta, "x64", 1);
        assert_eq!(
            policy.evaluate(&release()?, true, true, false)?,
            UpdateState::Blocked {
                reason: UpdateBlockReason::ActiveEffectJournal
            }
        );
        Ok(())
    }

    #[test]
    fn active_journal_blocks_an_otherwise_eligible_update() -> Result<(), Box<dyn std::error::Error>>
    {
        let policy = UpdatePolicy::new("sapphirus-desktop", ReleaseChannel::Beta, "x64", 1);
        assert_eq!(
            policy.evaluate(&release()?, true, true, false)?,
            UpdateState::Blocked {
                reason: UpdateBlockReason::ActiveEffectJournal
            }
        );
        Ok(())
    }

    #[test]
    fn enterprise_mode_never_invokes_in_app_install() -> Result<(), Box<dyn std::error::Error>> {
        let policy = UpdatePolicy::new(
            "sapphirus-desktop",
            ReleaseChannel::EnterpriseManaged,
            "x64",
            1,
        );
        assert_eq!(
            policy.evaluate(&release()?, true, false, false)?,
            UpdateState::Blocked {
                reason: UpdateBlockReason::ManagedInstallation
            }
        );
        Ok(())
    }
}
