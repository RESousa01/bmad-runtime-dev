//! Production support-plane composition (D2-E Task 9): fail-closed
//! configuration, ES256 proof verification for signed policies, leases, and
//! canonical receipts, and the bootstrap → registration → policy → lease
//! sequencing client with a bounded last-known-valid signed cache.
//!
//! Trust material here is public only (tenant/client identifiers, pinned
//! public keys, issuer/audience strings). Tokens, private keys, proofs, and
//! raw model output never appear in this module's types, so no projection of
//! them can cross IPC.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use desktop_runtime::{canonical_hash, ContractId, Sha256Digest, UnixMillis};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::installation_identity::p256_spki_from_point;
use crate::transport::SupportApiOrigin;
use crate::CloudError;

/// One pinned ES256 verification key: the exact versioned key id and the
/// DER `SubjectPublicKeyInfo` of its public half.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedProofKey {
    pub key_id: String,
    pub public_key_spki: Vec<u8>,
}

impl PinnedProofKey {
    fn is_valid(&self) -> bool {
        (1..=256).contains(&self.key_id.len()) && self.public_key_spki.len() == 91
    }
}

/// The rotation policy for one proof surface: exactly one active key plus an
/// explicit verification-only overlap. Unknown, disabled, or retired keys
/// are outside policy and always rejected.
#[derive(Clone, Debug)]
pub struct ProofKeyRing {
    active: PinnedProofKey,
    verification_only: Vec<PinnedProofKey>,
}

impl ProofKeyRing {
    /// # Errors
    ///
    /// Fails when any pinned key is structurally invalid.
    pub fn new(
        active: PinnedProofKey,
        verification_only: Vec<PinnedProofKey>,
    ) -> Result<Self, CloudError> {
        if !active.is_valid() || verification_only.iter().any(|key| !key.is_valid()) {
            return Err(CloudError::InvalidSupportOrigin);
        }
        Ok(Self {
            active,
            verification_only,
        })
    }

    fn resolve(&self, key_id: &str) -> Option<&PinnedProofKey> {
        if self.active.key_id == key_id {
            return Some(&self.active);
        }
        self.verification_only
            .iter()
            .find(|key| key.key_id == key_id)
    }

    /// Verifies a raw `r||s` base64url ES256 signature over the exact
    /// 32-byte canonical digest with the ring member named by `key_id`.
    /// Vault-held proof keys sign the digest directly (no re-hashing),
    /// matching `KeyVaultHashSigner` on the service side.
    ///
    /// # Errors
    ///
    /// Fails closed on unknown key ids, bad encodings, and bad signatures.
    pub fn verify(
        &self,
        key_id: &str,
        digest: &Sha256Digest,
        signature_base64url: &str,
    ) -> Result<(), CloudError> {
        let Some(key) = self.resolve(key_id) else {
            return Err(CloudError::ReceiptInvalid);
        };
        let Some(signature) = decode_base64url(signature_base64url) else {
            return Err(CloudError::ReceiptInvalid);
        };
        if signature.len() != 64 {
            return Err(CloudError::ReceiptInvalid);
        }
        verify_p256_digest_signature(&key.public_key_spki, digest.as_bytes(), &signature)
    }
}

/// Everything production mode requires before it may start. Construction
/// fails closed when any element is missing or malformed; there is no
/// partial or degraded production configuration.
#[derive(Clone, Debug)]
pub struct ProductionSupportConfig {
    pub tenant_id: String,
    pub api_client_id: String,
    pub scope: String,
    pub origin: SupportApiOrigin,
    pub region: String,
    pub policy_trust: ProofKeyRing,
    pub receipt_trust: ProofKeyRing,
    pub receipt_issuer: String,
    pub receipt_audience: String,
}

impl ProductionSupportConfig {
    /// # Errors
    ///
    /// Returns [`CloudError::InvalidSupportOrigin`] when any field is
    /// absent or malformed. Production cannot start without the exact
    /// tenant, API client, scope, origin, and trust configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant_id: &str,
        api_client_id: &str,
        scope: &str,
        origin: &str,
        region: &str,
        policy_trust: ProofKeyRing,
        receipt_trust: ProofKeyRing,
        receipt_issuer: &str,
        receipt_audience: &str,
    ) -> Result<Self, CloudError> {
        if !is_guid(tenant_id)
            || !is_guid(api_client_id)
            || !scope.starts_with("api://")
            || scope.len() > 256
            || region.is_empty()
            || region.len() > 64
            || receipt_issuer.len() < 8
            || !receipt_issuer.starts_with("https://")
            || receipt_audience.is_empty()
            || receipt_audience.len() > 256
        {
            return Err(CloudError::InvalidSupportOrigin);
        }
        Ok(Self {
            tenant_id: tenant_id.to_owned(),
            api_client_id: api_client_id.to_owned(),
            scope: scope.to_owned(),
            origin: SupportApiOrigin::new(origin)?,
            region: region.to_owned(),
            policy_trust,
            receipt_trust,
            receipt_issuer: receipt_issuer.to_owned(),
            receipt_audience: receipt_audience.to_owned(),
        })
    }
}

fn is_guid(value: &str) -> bool {
    value.len() == 36
        && value.chars().enumerate().all(|(index, character)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                character == '-'
            } else {
                character.is_ascii_hexdigit()
            }
        })
}

/// A signed desktop policy exactly as served; unknown fields fail closed.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedDesktopPolicyDoc {
    pub schema_version: String,
    pub policy_id: String,
    pub policy_version: u64,
    pub policy_hash: String,
    pub system_browser_fallback_allowed: bool,
    pub maximum_context_bytes: u32,
    pub maximum_context_items: u32,
    pub allowed_regions: Vec<String>,
    pub key_id: String,
    pub signature: String,
}

/// A signed entitlement lease exactly as served; unknown fields fail closed.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedEntitlementLeaseDoc {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyDraft<'a> {
    schema_version: &'a str,
    policy_id: &'a str,
    policy_version: u64,
    system_browser_fallback_allowed: bool,
    maximum_context_bytes: u32,
    maximum_context_items: u32,
    allowed_regions: &'a [String],
    retention_mode: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LeaseDraft<'a> {
    schema_version: &'a str,
    lease_id: &'a str,
    registration_id: &'a str,
    subject_hash: &'a str,
    delivery_model: &'a str,
    issued_at: &'a str,
    not_before: &'a str,
    expires_at: &'a str,
    offline_grace_ends_at: &'a str,
    features: &'a [String],
    tenant_policy_hash: &'a str,
    minimum_client_version: &'a str,
}

/// A policy whose canonical hash and ES256 proof verified against the
/// pinned policy trust ring, with downgrade protection.
#[derive(Clone, Debug)]
pub struct VerifiedSignedPolicy {
    pub document: SignedDesktopPolicyDoc,
    pub canonical_hash: Sha256Digest,
}

impl VerifiedSignedPolicy {
    /// # Errors
    ///
    /// Fails closed on schema, hash, signature, key, region, limit, or
    /// version-downgrade violations.
    pub fn verify(
        document: SignedDesktopPolicyDoc,
        config: &ProductionSupportConfig,
        previous_version: Option<u64>,
    ) -> Result<Self, CloudError> {
        if document.schema_version != "desktop-policy.v1"
            || document.policy_version == 0
            || document.maximum_context_bytes == 0
            || document.maximum_context_bytes > 512 * 1024
            || document.maximum_context_items == 0
            || document.maximum_context_items > 64
            || document.allowed_regions.is_empty()
            || !document
                .allowed_regions
                .iter()
                .any(|allowed| allowed == &config.region)
        {
            return Err(CloudError::EntitlementUnavailable);
        }
        if let Some(previous) = previous_version {
            if document.policy_version < previous {
                return Err(CloudError::EntitlementUnavailable);
            }
        }
        let draft = PolicyDraft {
            schema_version: &document.schema_version,
            policy_id: &document.policy_id,
            policy_version: document.policy_version,
            system_browser_fallback_allowed: document.system_browser_fallback_allowed,
            maximum_context_bytes: document.maximum_context_bytes,
            maximum_context_items: document.maximum_context_items,
            allowed_regions: &document.allowed_regions,
            retention_mode: "transient_no_store",
        };
        let digest = canonical_hash("desktop-policy", 1, &draft)
            .map_err(|_| CloudError::EntitlementUnavailable)?;
        if digest.to_string() != document.policy_hash {
            return Err(CloudError::EntitlementUnavailable);
        }
        config
            .policy_trust
            .verify(&document.key_id, &digest, &document.signature)?;
        Ok(Self {
            document,
            canonical_hash: digest,
        })
    }
}

/// A lease whose canonical hash and proof verified, bound to one
/// registration and the verified policy hash, inside its validity window.
#[derive(Clone, Debug)]
pub struct VerifiedLease {
    pub document: SignedEntitlementLeaseDoc,
    pub canonical_hash: Sha256Digest,
}

impl VerifiedLease {
    /// # Errors
    ///
    /// Fails closed on schema, binding, window, hash, signature, or key
    /// violations.
    pub fn verify(
        document: SignedEntitlementLeaseDoc,
        config: &ProductionSupportConfig,
        policy: &VerifiedSignedPolicy,
        registration_id: &ContractId,
        now: UnixMillis,
    ) -> Result<Self, CloudError> {
        if document.schema_version != "desktop-entitlement-lease.v1"
            || document.delivery_model != "windows_local"
            || document.registration_id != registration_id.as_str()
            || document.tenant_policy_hash != policy.document.policy_hash
        {
            return Err(CloudError::EntitlementUnavailable);
        }
        // The wire encoding of instants may differ from the canonical
        // rendering the service hashed; every instant is parsed and
        // re-rendered canonically before recomputing the digest.
        let issued_at =
            canonical_instant(&document.issued_at).ok_or(CloudError::EntitlementUnavailable)?;
        let (not_before_millis, not_before) = canonical_instant_with_millis(&document.not_before)
            .ok_or(CloudError::EntitlementUnavailable)?;
        let (expires_millis, expires_at) = canonical_instant_with_millis(&document.expires_at)
            .ok_or(CloudError::EntitlementUnavailable)?;
        let offline_grace_ends_at = canonical_instant(&document.offline_grace_ends_at)
            .ok_or(CloudError::EntitlementUnavailable)?;
        if now.0 < not_before_millis.0 || now.0 >= expires_millis.0 {
            return Err(CloudError::EntitlementUnavailable);
        }
        let draft = LeaseDraft {
            schema_version: &document.schema_version,
            lease_id: &document.lease_id,
            registration_id: &document.registration_id,
            subject_hash: &document.subject_hash,
            delivery_model: &document.delivery_model,
            issued_at: &issued_at,
            not_before: &not_before,
            expires_at: &expires_at,
            offline_grace_ends_at: &offline_grace_ends_at,
            features: &document.features,
            tenant_policy_hash: &document.tenant_policy_hash,
            minimum_client_version: &document.minimum_client_version,
        };
        let digest = canonical_hash("entitlement-lease", 1, &draft)
            .map_err(|_| CloudError::EntitlementUnavailable)?;
        config
            .policy_trust
            .verify(&document.key_id, &digest, &document.signature)?;
        Ok(Self {
            document,
            canonical_hash: digest,
        })
    }
}

/// The canonical receipt proof header served by the support plane.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalReceiptProof {
    pub proof_type: String,
    pub algorithm: String,
    pub issuer: String,
    pub audience: String,
    pub key_id: String,
    pub signed_payload_hash: String,
    pub signature: String,
}

/// Verifies one canonical receipt proof against the receipt trust ring and
/// the exact expected canonical receipt hash, with replay protection over
/// receipt ids handled by [`ProductionSupportClient`].
///
/// # Errors
///
/// Fails closed on issuer, audience, algorithm, hash-binding, key, or
/// signature violations.
pub fn verify_canonical_receipt_proof(
    proof: &CanonicalReceiptProof,
    expected_receipt_hash: &str,
    config: &ProductionSupportConfig,
) -> Result<(), CloudError> {
    if proof.proof_type != "support_plane_signature"
        || proof.algorithm != "ES256"
        || proof.issuer != config.receipt_issuer
        || proof.audience != config.receipt_audience
        || proof.signed_payload_hash != expected_receipt_hash
    {
        return Err(CloudError::ReceiptInvalid);
    }
    let digest = digest_from_rendered(expected_receipt_hash).ok_or(CloudError::ReceiptInvalid)?;
    config
        .receipt_trust
        .verify(&proof.key_id, &digest, &proof.signature)
}

fn digest_from_rendered(value: &str) -> Option<Sha256Digest> {
    let hex = value.strip_prefix("sha256:")?;
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        bytes[index] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(Sha256Digest::from_bytes(bytes))
}

/// Host-owned encrypted storage for last-known-valid signed documents.
/// Implementations must live inside the existing local identity boundary.
pub trait SignedStateStore: Send + Sync {
    fn load(&self, name: &str) -> Option<String>;
    fn save(&self, name: &str, value: &str);
    fn clear(&self);
}

/// Sequencing and session authority for the production support plane.
/// Sign-out bumps the session epoch: every projection minted before the
/// bump is rejected afterward, without touching local work.
pub struct ProductionSupportClient {
    config: ProductionSupportConfig,
    state_store: Box<dyn SignedStateStore>,
    session_epoch: AtomicU64,
    seen_receipt_ids: Mutex<HashSet<String>>,
}

impl std::fmt::Debug for ProductionSupportClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductionSupportClient")
            .field("region", &self.config.region)
            .field("session_epoch", &self.session_epoch.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

/// A bounded, IPC-safe session projection: status words and an epoch only.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSessionProjection {
    pub schema_version: &'static str,
    pub status: &'static str,
    pub session_epoch: u64,
    pub region: String,
}

impl ProductionSupportClient {
    #[must_use]
    pub fn new(config: ProductionSupportConfig, state_store: Box<dyn SignedStateStore>) -> Self {
        Self {
            config,
            state_store,
            session_epoch: AtomicU64::new(1),
            seen_receipt_ids: Mutex::new(HashSet::new()),
        }
    }

    #[must_use]
    pub fn config(&self) -> &ProductionSupportConfig {
        &self.config
    }

    #[must_use]
    pub fn session_epoch(&self) -> u64 {
        self.session_epoch.load(Ordering::SeqCst)
    }

    /// Invalidates every outstanding session-scoped authority. Local work,
    /// caches of *verified* public documents, and history are untouched.
    pub fn sign_out(&self) {
        self.session_epoch.fetch_add(1, Ordering::SeqCst);
    }

    /// # Errors
    ///
    /// Returns [`CloudError::SessionInvalidated`] when `minted_epoch` is
    /// older than the current session epoch.
    pub fn require_session(&self, minted_epoch: u64) -> Result<(), CloudError> {
        if minted_epoch == self.session_epoch() {
            Ok(())
        } else {
            Err(CloudError::SessionInvalidated)
        }
    }

    #[must_use]
    pub fn session_projection(&self) -> ProductionSessionProjection {
        ProductionSessionProjection {
            schema_version: "sapphirus.production-session.v1",
            status: "configured",
            session_epoch: self.session_epoch(),
            region: self.config.region.clone(),
        }
    }

    /// Verifies and caches a freshly served policy, enforcing downgrade
    /// protection against the last known valid policy.
    ///
    /// # Errors
    ///
    /// Propagates every verification failure; the cache is only written
    /// after full verification.
    pub fn accept_policy(
        &self,
        document: SignedDesktopPolicyDoc,
    ) -> Result<VerifiedSignedPolicy, CloudError> {
        let previous_version = self
            .last_known_policy()
            .map(|policy| policy.document.policy_version);
        let verified = VerifiedSignedPolicy::verify(document, &self.config, previous_version)?;
        if let Ok(serialized) = serde_json::to_string(&verified.document) {
            self.state_store
                .save("policy.last-known-valid", &serialized);
        }
        Ok(verified)
    }

    /// The last cached policy, re-verified from the signed bytes; a
    /// tampered cache entry is discarded, never trusted.
    #[must_use]
    pub fn last_known_policy(&self) -> Option<VerifiedSignedPolicy> {
        let serialized = self.state_store.load("policy.last-known-valid")?;
        let document: SignedDesktopPolicyDoc = serde_json::from_str(&serialized).ok()?;
        VerifiedSignedPolicy::verify(document, &self.config, None).ok()
    }

    /// Verifies a served lease against the verified policy and window.
    ///
    /// # Errors
    ///
    /// Propagates every verification failure.
    pub fn accept_lease(
        &self,
        document: SignedEntitlementLeaseDoc,
        policy: &VerifiedSignedPolicy,
        registration_id: &ContractId,
        now: UnixMillis,
    ) -> Result<VerifiedLease, CloudError> {
        let verified = VerifiedLease::verify(document, &self.config, policy, registration_id, now)?;
        if let Ok(serialized) = serde_json::to_string(&verified.document) {
            self.state_store.save("lease.last-known-valid", &serialized);
        }
        Ok(verified)
    }

    /// Verifies a canonical receipt proof and enforces receipt-id replay
    /// protection for this process lifetime.
    ///
    /// # Errors
    ///
    /// Fails closed on proof violations and on any replayed receipt id.
    pub fn accept_receipt_proof(
        &self,
        receipt_id: &str,
        proof: &CanonicalReceiptProof,
        expected_receipt_hash: &str,
    ) -> Result<(), CloudError> {
        verify_canonical_receipt_proof(proof, expected_receipt_hash, &self.config)?;
        let mut seen = self.seen_receipt_ids.lock();
        if !seen.insert(receipt_id.to_owned()) {
            return Err(CloudError::ReceiptInvalid);
        }
        Ok(())
    }
}

fn canonical_instant(value: &str) -> Option<String> {
    canonical_instant_with_millis(value).map(|(_, rendered)| rendered)
}

fn canonical_instant_with_millis(value: &str) -> Option<(UnixMillis, String)> {
    let format = time::format_description::well_known::Rfc3339;
    let parsed = time::OffsetDateTime::parse(value, &format)
        .ok()?
        .to_offset(time::UtcOffset::UTC);
    let millis = u64::try_from(parsed.unix_timestamp_nanos() / 1_000_000).ok()?;
    let rendered = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        parsed.year(),
        u8::from(parsed.month()),
        parsed.day(),
        parsed.hour(),
        parsed.minute(),
        parsed.second(),
        parsed.millisecond(),
    );
    Some((UnixMillis(millis), rendered))
}

fn decode_base64url(value: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    if value.is_empty() || value.len() % 4 == 1 {
        return None;
    }
    let mut output = Vec::with_capacity(value.len() * 3 / 4);
    let mut accumulator: u32 = 0;
    let mut bits = 0u32;
    for byte in value.bytes() {
        let index = ALPHABET.iter().position(|&entry| entry == byte)?;
        accumulator = (accumulator << 6) | u32::try_from(index).ok()?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(u8::try_from((accumulator >> bits) & 0xff).ok()?);
        }
    }
    Some(output)
}

#[cfg(windows)]
#[allow(unsafe_code, clippy::borrow_as_ptr)]
fn verify_p256_digest_signature(
    spki: &[u8],
    digest: &[u8],
    signature: &[u8],
) -> Result<(), CloudError> {
    use windows::Win32::Security::Cryptography::{
        BCryptCloseAlgorithmProvider, BCryptDestroyKey, BCryptImportKeyPair,
        BCryptOpenAlgorithmProvider, BCryptVerifySignature, BCRYPT_ALG_HANDLE,
        BCRYPT_ECCPUBLIC_BLOB, BCRYPT_ECDSA_P256_ALGORITHM, BCRYPT_FLAGS, BCRYPT_KEY_HANDLE,
        BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS,
    };

    if spki.len() != 91 || spki[26] != 0x04 || digest.len() != 32 {
        return Err(CloudError::ReceiptInvalid);
    }
    let mut blob = Vec::with_capacity(8 + 64);
    blob.extend_from_slice(&0x3153_4345_u32.to_le_bytes());
    blob.extend_from_slice(&32_u32.to_le_bytes());
    blob.extend_from_slice(&spki[27..91]);
    unsafe {
        let mut algorithm = BCRYPT_ALG_HANDLE::default();
        BCryptOpenAlgorithmProvider(
            &mut algorithm,
            BCRYPT_ECDSA_P256_ALGORITHM,
            None,
            BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0),
        )
        .ok()
        .map_err(|_| CloudError::ReceiptInvalid)?;
        let mut key = BCRYPT_KEY_HANDLE::default();
        let imported =
            BCryptImportKeyPair(algorithm, None, BCRYPT_ECCPUBLIC_BLOB, &mut key, &blob, 0).ok();
        if imported.is_err() {
            let _ = BCryptCloseAlgorithmProvider(algorithm, 0);
            return Err(CloudError::ReceiptInvalid);
        }
        let verified = BCryptVerifySignature(key, None, digest, signature, BCRYPT_FLAGS(0))
            .ok()
            .is_ok();
        let _ = BCryptDestroyKey(key);
        let _ = BCryptCloseAlgorithmProvider(algorithm, 0);
        if verified {
            Ok(())
        } else {
            Err(CloudError::ReceiptInvalid)
        }
    }
}

#[cfg(not(windows))]
fn verify_p256_digest_signature(
    _spki: &[u8],
    _digest: &[u8],
    _signature: &[u8],
) -> Result<(), CloudError> {
    // Production desktop composition is Windows-only in this delivery.
    Err(CloudError::ReceiptInvalid)
}

/// Builds a pinned proof key from an uncompressed P-256 point.
#[must_use]
pub fn pinned_key_from_point(key_id: &str, x: &[u8; 32], y: &[u8; 32]) -> PinnedProofKey {
    PinnedProofKey {
        key_id: key_id.to_owned(),
        public_key_spki: p256_spki_from_point(x, y),
    }
}
