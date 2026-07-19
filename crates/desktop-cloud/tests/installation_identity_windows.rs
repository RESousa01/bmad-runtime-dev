#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::borrow_as_ptr,
    reason = "test fixtures fail loudly at construction"
)]
//! Windows platform-key integration tests for the installation identity.
#![cfg(windows)]
#![allow(unsafe_code)]

use desktop_cloud::InstallationConsentSigner as _;
use desktop_cloud::WindowsInstallationIdentity;
use desktop_runtime::sha256_bytes;
use sha2::{Digest, Sha256};
use windows::Win32::Security::Cryptography::{
    BCryptCloseAlgorithmProvider, BCryptDestroyKey, BCryptImportKeyPair,
    BCryptOpenAlgorithmProvider, BCryptVerifySignature, BCRYPT_ALG_HANDLE,
    BCRYPT_ECDSA_P256_ALGORITHM, BCRYPT_ECCPUBLIC_BLOB, BCRYPT_KEY_HANDLE,
    BCRYPT_FLAGS, BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS,
};

fn base64url_decode(value: &str) -> Vec<u8> {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let index = |character: u8| -> u32 {
        u32::try_from(
            ALPHABET
                .iter()
                .position(|&entry| entry == character)
                .expect("valid base64url character"),
        )
        .expect("index fits")
    };
    let bytes = value.as_bytes();
    let mut output = Vec::new();
    for chunk in bytes.chunks(4) {
        let mut accumulator: u32 = 0;
        for &character in chunk {
            accumulator = (accumulator << 6) | index(character);
        }
        let bits = chunk.len() * 6;
        accumulator <<= 24 - bits;
        let produced = match chunk.len() {
            2 => 1,
            3 => 2,
            4 => 3,
            _ => panic!("invalid base64url chunk"),
        };
        for byte_index in 0..produced {
            output.push(((accumulator >> (16 - 8 * byte_index)) & 0xff) as u8);
        }
    }
    output
}

fn verify_p256_raw_signature(spki: &[u8], payload: &[u8], signature: &[u8]) -> bool {
    assert_eq!(spki.len(), 91, "P-256 SPKI length");
    assert_eq!(signature.len(), 64, "raw r||s length");
    let digest: [u8; 32] = Sha256::digest(payload).into();
    // Rebuild the BCRYPT_ECCKEY_BLOB from the SPKI point.
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
        .expect("open ECDSA provider");
        let mut key = BCRYPT_KEY_HANDLE::default();
        BCryptImportKeyPair(
            algorithm,
            None,
            BCRYPT_ECCPUBLIC_BLOB,
            &mut key,
            &blob,
            0,
        )
        .ok()
        .expect("import public key");
        let verified = BCryptVerifySignature(key, None, &digest, signature, BCRYPT_FLAGS(0))
            .ok()
            .is_ok();
        let _ = BCryptDestroyKey(key);
        let _ = BCryptCloseAlgorithmProvider(algorithm, 0);
        verified
    }
}

#[test]
fn windows_installation_key_signs_the_exact_consent_payload() {
    let key_name = format!("sapphirus-test-install-{}", std::process::id());
    let identity =
        WindowsInstallationIdentity::open_or_create(&key_name).expect("open or create key");

    let spki = identity.public_key_spki().to_vec();
    let key_id = identity.key_id().to_string();
    assert_eq!(key_id, sha256_bytes(&spki).to_string());
    assert!(key_id.starts_with("sha256:"));

    let envelope_hash = sha256_bytes(b"consent-envelope-fixture");
    let signature = identity.sign(&envelope_hash).expect("sign envelope hash");
    let signature_bytes = base64url_decode(&signature);
    let payload = desktop_cloud::consent_signature_payload(&envelope_hash);
    assert!(
        verify_p256_raw_signature(&spki, &payload, &signature_bytes),
        "signature must verify over the ASCII envelope-hash payload",
    );
    assert!(
        !verify_p256_raw_signature(&spki, b"sha256:forged", &signature_bytes),
        "signature must not verify over another payload",
    );

    // Reopening resolves the same persisted key.
    drop(identity);
    let reopened =
        WindowsInstallationIdentity::open_or_create(&key_name).expect("reopen key");
    assert_eq!(reopened.public_key_spki(), spki.as_slice());
    let debug_output = format!("{reopened:?}");
    assert!(debug_output.contains("key_id"));
    assert!(
        !debug_output.contains(&reopened.public_key_base64url()),
        "debug output must redact key material",
    );
    reopened.delete().expect("delete test key");
}
