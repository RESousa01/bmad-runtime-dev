use std::collections::HashSet;

use desktop_runtime::{Sha256Digest, UnixMillis};
use semver::Version;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::{CloudError, EntitlementLease};

const LEASE_SCHEMA: &str = "desktop-entitlement-lease.v1";
const DELIVERY_MODEL: &str = "windows_local";

pub trait EntitlementProofVerifier: Send + Sync {
    /// Verifies the lease signature, key trust, and audience policy.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::EntitlementUnavailable`] for untrusted proof.
    fn verify(&self, lease: &EntitlementLease) -> Result<(), CloudError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedEntitlement {
    registration_id: String,
    required_feature: String,
    policy_hash: Sha256Digest,
    expires_at: UnixMillis,
    offline_grace_ends_at: UnixMillis,
}

impl VerifiedEntitlement {
    /// Trusted internal bridge for documents already verified by the
    /// production signed-lease path.
    pub(crate) const fn from_verified_parts(
        registration_id: String,
        required_feature: String,
        policy_hash: Sha256Digest,
        expires_at: UnixMillis,
        offline_grace_ends_at: UnixMillis,
    ) -> Self {
        Self {
            registration_id,
            required_feature,
            policy_hash,
            expires_at,
            offline_grace_ends_at,
        }
    }

    #[must_use]
    pub fn registration_id(&self) -> &str {
        &self.registration_id
    }

    #[must_use]
    pub fn required_feature(&self) -> &str {
        &self.required_feature
    }

    #[must_use]
    pub const fn expires_at(&self) -> UnixMillis {
        self.expires_at
    }

    #[must_use]
    pub const fn offline_grace_ends_at(&self) -> UnixMillis {
        self.offline_grace_ends_at
    }

    pub(crate) fn authorize_model_request(
        &self,
        policy_hash: Sha256Digest,
        now: UnixMillis,
    ) -> Result<(), CloudError> {
        if self.required_feature != "model_access"
            || self.policy_hash != policy_hash
            || now >= self.expires_at
        {
            return Err(CloudError::EntitlementUnavailable);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct EntitlementVerifier<V> {
    proof_verifier: V,
    expected_registration_id: String,
    expected_subject_hash: Sha256Digest,
    expected_policy_hash: Sha256Digest,
    required_feature: String,
    current_client_version: Version,
}

impl<V> EntitlementVerifier<V>
where
    V: EntitlementProofVerifier,
{
    /// Creates an audience-bound lease verifier from trusted local state.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::EntitlementUnavailable`] for malformed trusted
    /// configuration.
    pub fn new(
        proof_verifier: V,
        expected_registration_id: impl Into<String>,
        expected_subject_hash: Sha256Digest,
        expected_policy_hash: Sha256Digest,
        required_feature: impl Into<String>,
        current_client_version: Version,
    ) -> Result<Self, CloudError> {
        let expected_registration_id = expected_registration_id.into();
        let required_feature = required_feature.into();
        if !valid_registration_id(&expected_registration_id) || !valid_feature(&required_feature) {
            return Err(CloudError::EntitlementUnavailable);
        }
        Ok(Self {
            proof_verifier,
            expected_registration_id,
            expected_subject_hash,
            expected_policy_hash,
            required_feature,
            current_client_version,
        })
    }

    /// Verifies one untrusted signed lease for online feature use.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::FeatureDisabled`] when the required feature is
    /// absent and [`CloudError::EntitlementUnavailable`] for all other lease,
    /// binding, version, time-window, or proof failures.
    pub fn verify(
        &self,
        lease: &EntitlementLease,
        now: UnixMillis,
    ) -> Result<VerifiedEntitlement, CloudError> {
        validate_lease_shape(lease)?;
        self.proof_verifier.verify(lease)?;
        let subject_hash = Sha256Digest::parse(&lease.subject_hash)
            .map_err(|_| CloudError::EntitlementUnavailable)?;
        let policy_hash = Sha256Digest::parse(&lease.tenant_policy_hash)
            .map_err(|_| CloudError::EntitlementUnavailable)?;
        if lease.registration_id != self.expected_registration_id
            || subject_hash != self.expected_subject_hash
            || policy_hash != self.expected_policy_hash
        {
            return Err(CloudError::EntitlementUnavailable);
        }
        if !lease
            .features
            .iter()
            .any(|feature| feature == &self.required_feature)
        {
            return Err(CloudError::FeatureDisabled);
        }
        let minimum_version = Version::parse(&lease.minimum_client_version)
            .map_err(|_| CloudError::EntitlementUnavailable)?;
        if self.current_client_version < minimum_version {
            return Err(CloudError::EntitlementUnavailable);
        }
        let issued_at = parse_instant(&lease.issued_at)?;
        let not_before = parse_instant(&lease.not_before)?;
        let expires_at = parse_instant(&lease.expires_at)?;
        let offline_grace_ends_at = parse_instant(&lease.offline_grace_ends_at)?;
        if not_before > issued_at
            || issued_at >= expires_at
            || expires_at > offline_grace_ends_at
            || now < not_before
            || now >= expires_at
        {
            return Err(CloudError::EntitlementUnavailable);
        }
        Ok(VerifiedEntitlement {
            registration_id: lease.registration_id.clone(),
            required_feature: self.required_feature.clone(),
            policy_hash,
            expires_at,
            offline_grace_ends_at,
        })
    }
}

fn validate_lease_shape(lease: &EntitlementLease) -> Result<(), CloudError> {
    if lease.schema_version != LEASE_SCHEMA
        || lease.delivery_model != DELIVERY_MODEL
        || !valid_registration_id(&lease.registration_id)
        || !(3..=128).contains(&lease.lease_id.len())
        || lease.key_id.is_empty()
        || lease.key_id.len() > 128
        || lease.signature.is_empty()
        || lease.signature.len() > 16 * 1024
        || lease.features.is_empty()
        || lease.features.len() > 64
    {
        return Err(CloudError::EntitlementUnavailable);
    }
    let mut features = HashSet::new();
    if lease
        .features
        .iter()
        .any(|feature| !valid_feature(feature) || !features.insert(feature))
    {
        return Err(CloudError::EntitlementUnavailable);
    }
    Ok(())
}

fn parse_instant(value: &str) -> Result<UnixMillis, CloudError> {
    let nanos = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| CloudError::EntitlementUnavailable)?
        .unix_timestamp_nanos();
    let millis = nanos
        .checked_div(1_000_000)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(CloudError::EntitlementUnavailable)?;
    Ok(UnixMillis(millis))
}

fn valid_registration_id(value: &str) -> bool {
    value.len() == 31
        && value.starts_with("dreg_")
        && value[5..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_feature(value: &str) -> bool {
    (3..=64).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}
