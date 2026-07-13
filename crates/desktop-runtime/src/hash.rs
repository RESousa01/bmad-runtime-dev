use core::cmp::Ordering;
use core::fmt;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

const DIGEST_BYTES: usize = 32;
const DIGEST_HEX_CHARS: usize = DIGEST_BYTES * 2;
const DIGEST_PREFIX: &str = "sha256:";

/// A validated, lowercase, SHA-256 digest with the contract-required prefix.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Sha256Digest([u8; DIGEST_BYTES]);

impl Sha256Digest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; DIGEST_BYTES]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; DIGEST_BYTES] {
        &self.0
    }

    #[must_use]
    pub fn hex_value(&self) -> String {
        hex::encode(self.0)
    }

    pub fn parse(value: &str) -> Result<Self, CanonicalHashError> {
        let encoded = value
            .strip_prefix(DIGEST_PREFIX)
            .ok_or(CanonicalHashError::InvalidDigest)?;
        if encoded.len() != DIGEST_HEX_CHARS
            || encoded
                .bytes()
                .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
        {
            return Err(CanonicalHashError::InvalidDigest);
        }

        let decoded = hex::decode(encoded).map_err(|_| CanonicalHashError::InvalidDigest)?;
        let bytes: [u8; DIGEST_BYTES] = decoded
            .try_into()
            .map_err(|_| CanonicalHashError::InvalidDigest)?;
        Ok(Self(bytes))
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{DIGEST_PREFIX}{}", hex::encode(self.0))
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl Serialize for Sha256Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Error)]
pub enum CanonicalHashError {
    #[error("the SHA-256 digest is not in canonical lowercase form")]
    InvalidDigest,
    #[error("the hash purpose must be lowercase ASCII with digits or hyphens")]
    InvalidPurpose,
    #[error("schema major must be at least one")]
    InvalidSchemaMajor,
    #[error("the canonical value must be a JSON object")]
    ExpectedObject,
    #[error("the excluded self-hash field is missing")]
    MissingExcludedField,
    #[error("canonical JSON serialization failed")]
    Serialization(#[from] serde_json::Error),
}

/// Hash arbitrary bytes without applying a contract-purpose preimage.
#[must_use]
pub fn sha256_bytes(value: &[u8]) -> Sha256Digest {
    let digest: [u8; DIGEST_BYTES] = Sha256::digest(value).into();
    Sha256Digest::from_bytes(digest)
}

/// Serialize a value into deterministic canonical JSON.
///
/// Object keys use RFC 8785's UTF-16 code-unit ordering. Contract values in
/// this crate contain only finite integers, so `serde_json`'s number rendering
/// is deterministic for the supported domain.
pub fn canonical_json_bytes<T>(value: &T) -> Result<Vec<u8>, CanonicalHashError>
where
    T: Serialize + ?Sized,
{
    let value = serde_json::to_value(value)?;
    let mut output = Vec::new();
    write_canonical(&value, &mut output)?;
    Ok(output)
}

/// Compute the purpose-separated contract hash defined by note 99.
pub fn canonical_hash<T>(
    purpose: &str,
    schema_major: u32,
    value: &T,
) -> Result<Sha256Digest, CanonicalHashError>
where
    T: Serialize + ?Sized,
{
    validate_hash_domain(purpose, schema_major)?;
    let mut preimage = format!("sapphirus:{purpose}:{schema_major}\n").into_bytes();
    preimage.extend(canonical_json_bytes(value)?);
    Ok(sha256_bytes(&preimage))
}

/// Compute a purpose-separated hash after removing one top-level self-hash.
pub fn canonical_hash_without_field<T>(
    purpose: &str,
    schema_major: u32,
    value: &T,
    excluded_field: &str,
) -> Result<Sha256Digest, CanonicalHashError>
where
    T: Serialize + ?Sized,
{
    validate_hash_domain(purpose, schema_major)?;
    let mut value = serde_json::to_value(value)?;
    let removed = value
        .as_object_mut()
        .ok_or(CanonicalHashError::ExpectedObject)?
        .remove(excluded_field);
    if removed.is_none() {
        return Err(CanonicalHashError::MissingExcludedField);
    }
    canonical_hash(purpose, schema_major, &value)
}

fn validate_hash_domain(purpose: &str, schema_major: u32) -> Result<(), CanonicalHashError> {
    if schema_major == 0 {
        return Err(CanonicalHashError::InvalidSchemaMajor);
    }
    if purpose.is_empty()
        || purpose
            .bytes()
            .any(|byte| !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && byte != b'-')
    {
        return Err(CanonicalHashError::InvalidPurpose);
    }
    Ok(())
}

fn write_canonical(value: &Value, output: &mut Vec<u8>) -> Result<(), serde_json::Error> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(flag) => {
            output.extend_from_slice(if *flag { b"true" } else { b"false" });
        }
        Value::Number(number) => output.extend_from_slice(number.to_string().as_bytes()),
        Value::String(text) => output.extend(serde_json::to_vec(text)?),
        Value::Array(items) => {
            output.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_canonical(item, output)?;
            }
            output.push(b']');
        }
        Value::Object(map) => {
            output.push(b'{');
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|(left, _), (right, _)| compare_utf16(left, right));
            for (index, (key, item)) in entries.into_iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                output.extend(serde_json::to_vec(key)?);
                output.push(b':');
                write_canonical(item, output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

fn compare_utf16(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{canonical_hash, canonical_json_bytes, sha256_bytes, Sha256Digest};

    #[test]
    fn rejects_noncanonical_digest_text() {
        let uppercase = format!("sha256:{}", "A".repeat(64));
        assert!(Sha256Digest::parse(&uppercase).is_err());
        assert!(Sha256Digest::parse(&"0".repeat(64)).is_err());
    }

    #[test]
    fn canonical_json_sorts_nested_object_keys() -> Result<(), Box<dyn std::error::Error>> {
        let value = json!({"z": 1, "a": {"b": true, "a": null}});
        let actual = String::from_utf8(canonical_json_bytes(&value)?)?;
        assert_eq!(actual, r#"{"a":{"a":null,"b":true},"z":1}"#);
        Ok(())
    }

    #[test]
    fn purpose_separator_changes_the_digest() -> Result<(), Box<dyn std::error::Error>> {
        let value = json!({"id": "candidate_1"});
        let candidate = canonical_hash("candidate-action", 1, &value)?;
        let approval = canonical_hash("approval-decision", 1, &value)?;
        assert_ne!(candidate, approval);
        Ok(())
    }

    #[test]
    fn byte_hash_has_expected_vector() {
        let digest = sha256_bytes(b"abc");
        assert_eq!(
            digest.to_string(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
