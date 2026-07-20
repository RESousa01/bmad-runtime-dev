//! Installation identity: a non-exportable P-256 key held by a Windows
//! platform key provider, used to sign canonical consent envelopes (D2-E
//! Task 4).
//!
//! # Signature specification (shared with the C# verifier)
//!
//! - The signed payload is the ASCII byte string `sha256:<64 lowercase hex>`
//!   of the domain-separated canonical consent-envelope hash
//!   (`sapphirus:model-context-consent:v1` preimage, RFC 8785 canonical
//!   JSON of the envelope draft without `consentEnvelopeHash`/`proof`).
//! - The algorithm is ES256: ECDSA over NIST P-256 with SHA-256.
//! - The signature encoding is the raw 64-byte `r || s` concatenation,
//!   base64url without padding (JOSE ES256 encoding).
//! - The proof key id is the SHA-256 hash of the DER
//!   `SubjectPublicKeyInfo`, rendered `sha256:<hex>` — identical to the
//!   registration's installation public-key hash.
//!
//! Only the opaque persisted key name and the public `SubjectPublicKeyInfo`
//! ever leave the provider; private material is non-exportable by
//! construction and no key or signature bytes appear in `Debug` output.

use desktop_runtime::{sha256_bytes, Sha256Digest};

/// DER `SubjectPublicKeyInfo` prefix for an uncompressed NIST P-256 point:
/// `SEQUENCE { SEQUENCE { id-ecPublicKey, prime256v1 }, BIT STRING { 0x04 … } }`.
const P256_SPKI_PREFIX: [u8; 27] = [
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a,
    0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00, 0x04,
];

/// Encodes bytes as base64url without padding.
#[must_use]
pub fn base64url_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        output.push(ALPHABET[usize::from(b0 >> 2)] as char);
        output.push(ALPHABET[usize::from(((b0 & 0x03) << 4) | (b1 >> 4))] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[usize::from(((b1 & 0x0f) << 2) | (b2 >> 6))] as char);
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[usize::from(b2 & 0x3f)] as char);
        }
    }
    output
}

/// Wraps an uncompressed P-256 point (x, y) into DER `SubjectPublicKeyInfo`.
#[must_use]
pub fn p256_spki_from_point(x: &[u8; 32], y: &[u8; 32]) -> Vec<u8> {
    let mut spki = Vec::with_capacity(P256_SPKI_PREFIX.len() + 64);
    spki.extend_from_slice(&P256_SPKI_PREFIX);
    spki.extend_from_slice(x);
    spki.extend_from_slice(y);
    spki
}

/// The stable key id for an installation public key: the hash of its SPKI.
#[must_use]
pub fn installation_key_id(spki: &[u8]) -> String {
    sha256_bytes(spki).to_string()
}

/// The exact bytes signed for a consent envelope hash.
#[must_use]
pub fn consent_signature_payload(envelope_hash: &Sha256Digest) -> Vec<u8> {
    envelope_hash.to_string().into_bytes()
}

#[cfg(windows)]
pub use windows_platform::WindowsInstallationIdentity;

#[cfg(windows)]
#[allow(
    unsafe_code,
    clippy::borrow_as_ptr,
    reason = "NCrypt FFI requires out-pointer arguments"
)]
mod windows_platform {
    use desktop_runtime::Sha256Digest;
    use sha2::{Digest, Sha256};
    use windows::core::PCWSTR;
    use windows::Win32::Security::Cryptography::{
        NCryptCreatePersistedKey, NCryptFinalizeKey, NCryptFreeObject, NCryptOpenKey,
        NCryptOpenStorageProvider, NCryptSignHash, BCRYPT_ECDSA_P256_ALGORITHM, CERT_KEY_SPEC,
        MS_KEY_STORAGE_PROVIDER, MS_PLATFORM_CRYPTO_PROVIDER, NCRYPT_FLAGS, NCRYPT_HANDLE,
        NCRYPT_KEY_HANDLE, NCRYPT_PROV_HANDLE, NCRYPT_SILENT_FLAG,
    };

    use super::{
        base64url_no_pad, consent_signature_payload, installation_key_id, p256_spki_from_point,
    };
    use crate::support_contract::InstallationConsentSigner;
    use crate::CloudError;

    const NTE_BAD_KEYSET: i32 = 0x8009_0016_u32.cast_signed();
    /// `BCRYPT_ECDSA_PUBLIC_P256_MAGIC` ("ECS1").
    const ECDSA_PUBLIC_P256_MAGIC: u32 = 0x3153_4345;

    /// A non-exportable installation P-256 key persisted in a Windows key
    /// storage provider (TPM-backed platform provider when available,
    /// software KSP otherwise). Only the opaque key name is stored by the
    /// caller; private material never leaves the provider.
    pub struct WindowsInstallationIdentity {
        provider: NCRYPT_PROV_HANDLE,
        key: NCRYPT_KEY_HANDLE,
        spki: Vec<u8>,
        key_id: String,
    }

    // NCrypt handles are process-local kernel object references that the
    // provider documents as usable across threads.
    unsafe impl Send for WindowsInstallationIdentity {}
    unsafe impl Sync for WindowsInstallationIdentity {}

    impl std::fmt::Debug for WindowsInstallationIdentity {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("WindowsInstallationIdentity")
                .field("key_id", &self.key_id)
                .finish_non_exhaustive()
        }
    }

    impl Drop for WindowsInstallationIdentity {
        fn drop(&mut self) {
            unsafe {
                let _ = NCryptFreeObject(NCRYPT_HANDLE(self.key.0));
                let _ = NCryptFreeObject(NCRYPT_HANDLE(self.provider.0));
            }
        }
    }

    impl WindowsInstallationIdentity {
        /// Opens the named installation key, creating and finalizing a new
        /// non-exportable P-256 key when none exists yet.
        ///
        /// # Errors
        ///
        /// Returns [`CloudError::InstallationKeyUnavailable`] when no key
        /// storage provider can open or create the key.
        pub fn open_or_create(key_name: &str) -> Result<Self, CloudError> {
            let name_wide: Vec<u16> = key_name.encode_utf16().chain([0]).collect();
            let mut last_error = CloudError::InstallationKeyUnavailable;
            for provider_name in [MS_PLATFORM_CRYPTO_PROVIDER, MS_KEY_STORAGE_PROVIDER] {
                match Self::open_or_create_with_provider(provider_name, &name_wide) {
                    Ok(identity) => return Ok(identity),
                    Err(error) => last_error = error,
                }
            }
            Err(last_error)
        }

        fn open_or_create_with_provider(
            provider_name: PCWSTR,
            key_name: &[u16],
        ) -> Result<Self, CloudError> {
            unsafe {
                let mut provider = NCRYPT_PROV_HANDLE::default();
                NCryptOpenStorageProvider(&mut provider, provider_name, 0)
                    .map_err(|_| CloudError::InstallationKeyUnavailable)?;
                let mut key = NCRYPT_KEY_HANDLE::default();
                let open = NCryptOpenKey(
                    provider,
                    &mut key,
                    PCWSTR(key_name.as_ptr()),
                    CERT_KEY_SPEC::default(),
                    NCRYPT_SILENT_FLAG,
                );
                if let Err(error) = open {
                    if error.code().0 != NTE_BAD_KEYSET {
                        let _ = NCryptFreeObject(NCRYPT_HANDLE(provider.0));
                        return Err(CloudError::InstallationKeyUnavailable);
                    }
                    let created = NCryptCreatePersistedKey(
                        provider,
                        &mut key,
                        BCRYPT_ECDSA_P256_ALGORITHM,
                        PCWSTR(key_name.as_ptr()),
                        CERT_KEY_SPEC::default(),
                        NCRYPT_FLAGS(0),
                    )
                    .and_then(|()| NCryptFinalizeKey(key, NCRYPT_SILENT_FLAG));
                    if created.is_err() {
                        let _ = NCryptFreeObject(NCRYPT_HANDLE(provider.0));
                        return Err(CloudError::InstallationKeyUnavailable);
                    }
                }
                match Self::export_spki(key) {
                    Ok(spki) => {
                        let key_id = installation_key_id(&spki);
                        Ok(Self {
                            provider,
                            key,
                            spki,
                            key_id,
                        })
                    }
                    Err(error) => {
                        let _ = NCryptFreeObject(NCRYPT_HANDLE(key.0));
                        let _ = NCryptFreeObject(NCRYPT_HANDLE(provider.0));
                        Err(error)
                    }
                }
            }
        }

        /// The DER `SubjectPublicKeyInfo` of the installation public key.
        #[must_use]
        pub fn public_key_spki(&self) -> &[u8] {
            &self.spki
        }

        /// The public key encoded base64url for registration transport.
        #[must_use]
        pub fn public_key_base64url(&self) -> String {
            base64url_no_pad(&self.spki)
        }

        /// Signs a raw 32-byte digest directly (no additional hashing), the
        /// way a vault-held proof key signs canonical digests. Test-support
        /// analog for verifying the production proof path end to end.
        #[doc(hidden)]
        pub fn sign_digest(&self, digest: &[u8; 32]) -> Result<String, CloudError> {
            unsafe {
                let mut required: u32 = 0;
                NCryptSignHash(self.key, None, digest, None, &mut required, NCRYPT_FLAGS(0))
                    .map_err(|_| CloudError::InstallationKeyUnavailable)?;
                let mut signature = vec![0u8; required as usize];
                NCryptSignHash(
                    self.key,
                    None,
                    digest,
                    Some(&mut signature),
                    &mut required,
                    NCRYPT_FLAGS(0),
                )
                .map_err(|_| CloudError::InstallationKeyUnavailable)?;
                signature.truncate(required as usize);
                if signature.len() != 64 {
                    return Err(CloudError::InstallationKeyUnavailable);
                }
                Ok(base64url_no_pad(&signature))
            }
        }

        /// Deletes the persisted key. Test-support only.
        #[doc(hidden)]
        pub fn delete(self) -> Result<(), CloudError> {
            use windows::Win32::Security::Cryptography::NCryptDeleteKey;
            unsafe {
                NCryptDeleteKey(self.key, 0).map_err(|_| CloudError::InstallationKeyUnavailable)?;
            }
            // NCryptDeleteKey frees the key handle; forget self so Drop does
            // not free it again, then release the provider handle.
            let provider = self.provider;
            std::mem::forget(self);
            unsafe {
                let _ = NCryptFreeObject(NCRYPT_HANDLE(provider.0));
            }
            Ok(())
        }

        fn export_spki(key: NCRYPT_KEY_HANDLE) -> Result<Vec<u8>, CloudError> {
            use windows::Win32::Security::Cryptography::NCryptExportKey;
            unsafe {
                let blob_type = windows::core::w!("ECCPUBLICBLOB");
                let mut required: u32 = 0;
                NCryptExportKey(
                    key,
                    None,
                    blob_type,
                    None,
                    None,
                    &mut required,
                    NCRYPT_FLAGS(0),
                )
                .map_err(|_| CloudError::InstallationKeyUnavailable)?;
                let mut blob = vec![0u8; required as usize];
                NCryptExportKey(
                    key,
                    None,
                    blob_type,
                    None,
                    Some(&mut blob),
                    &mut required,
                    NCRYPT_FLAGS(0),
                )
                .map_err(|_| CloudError::InstallationKeyUnavailable)?;
                blob.truncate(required as usize);
                // BCRYPT_ECCKEY_BLOB: magic, cbKey, X, Y.
                if blob.len() < 8 + 64 {
                    return Err(CloudError::InstallationKeyUnavailable);
                }
                let magic = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]);
                let key_bytes = u32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]);
                if magic != ECDSA_PUBLIC_P256_MAGIC || key_bytes != 32 {
                    return Err(CloudError::InstallationKeyUnavailable);
                }
                let mut x = [0u8; 32];
                let mut y = [0u8; 32];
                x.copy_from_slice(&blob[8..40]);
                y.copy_from_slice(&blob[40..72]);
                Ok(p256_spki_from_point(&x, &y))
            }
        }
    }

    impl InstallationConsentSigner for WindowsInstallationIdentity {
        fn key_id(&self) -> &str {
            &self.key_id
        }

        fn sign(&self, signed_payload_hash: &Sha256Digest) -> Result<String, CloudError> {
            let payload = consent_signature_payload(signed_payload_hash);
            let digest: [u8; 32] = Sha256::digest(&payload).into();
            unsafe {
                let mut required: u32 = 0;
                NCryptSignHash(
                    self.key,
                    None,
                    &digest,
                    None,
                    &mut required,
                    NCRYPT_FLAGS(0),
                )
                .map_err(|_| CloudError::InstallationKeyUnavailable)?;
                let mut signature = vec![0u8; required as usize];
                NCryptSignHash(
                    self.key,
                    None,
                    &digest,
                    Some(&mut signature),
                    &mut required,
                    NCRYPT_FLAGS(0),
                )
                .map_err(|_| CloudError::InstallationKeyUnavailable)?;
                signature.truncate(required as usize);
                if signature.len() != 64 {
                    return Err(CloudError::InstallationKeyUnavailable);
                }
                Ok(base64url_no_pad(&signature))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_matches_known_vectors() {
        assert_eq!(base64url_no_pad(b""), "");
        assert_eq!(base64url_no_pad(b"f"), "Zg");
        assert_eq!(base64url_no_pad(b"fo"), "Zm8");
        assert_eq!(base64url_no_pad(b"foo"), "Zm9v");
        assert_eq!(base64url_no_pad(&[0xfb, 0xef, 0xbe]), "----");
        assert_eq!(base64url_no_pad(&[0xff, 0xff, 0xff]), "____");
    }

    #[test]
    fn spki_wrapping_is_der_stable() {
        let spki = p256_spki_from_point(&[0x11; 32], &[0x22; 32]);
        assert_eq!(spki.len(), 91);
        assert_eq!(spki[0], 0x30);
        assert_eq!(spki[26], 0x04);
        assert!(installation_key_id(&spki).starts_with("sha256:"));
    }
}
