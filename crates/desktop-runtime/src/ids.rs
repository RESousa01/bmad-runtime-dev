use core::cmp::Ordering;
use core::fmt;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

const MAX_ID_BYTES: usize = 128;
const MAX_RELATIVE_PATH_BYTES: usize = 1_024;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IdentifierError {
    #[error("contract identifier is invalid")]
    InvalidContractId,
    #[error("relative workspace path is invalid")]
    InvalidRelativePath,
}

/// An opaque, validated contract identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContractId(String);

impl ContractId {
    /// Creates an opaque identifier containing 3 to 128 safe ASCII bytes.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierError::InvalidContractId`] when the value has an
    /// invalid length or contains a character outside the contract alphabet.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        let valid_length = (3..=MAX_ID_BYTES).contains(&value.len());
        let valid_chars = value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
        if !valid_length || !valid_chars {
            return Err(IdentifierError::InvalidContractId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContractId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for ContractId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ContractId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// A canonical forward-slash path which cannot escape a selected workspace.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelativeWorkspacePath(String);

impl RelativeWorkspacePath {
    /// Creates a validated, workspace-relative path using forward slashes.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierError::InvalidRelativePath`] when the value is
    /// rooted, escapes the workspace, uses a Windows alias or reserved form,
    /// or violates the path length and character constraints.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_RELATIVE_PATH_BYTES
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\\')
            || value.contains(':')
            || value.chars().any(char::is_control)
        {
            return Err(IdentifierError::InvalidRelativePath);
        }

        for component in value.split('/') {
            if !is_safe_component(component) {
                return Err(IdentifierError::InvalidRelativePath);
            }
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn case_folded(&self) -> String {
        self.0.to_lowercase()
    }

    /// Cross-language canonical ordering used for authority-owned path arrays.
    #[must_use]
    pub fn canonical_cmp(&self, other: &Self) -> Ordering {
        self.0.encode_utf16().cmp(other.0.encode_utf16())
    }
}

impl fmt::Display for RelativeWorkspacePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for RelativeWorkspacePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RelativeWorkspacePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// UTC Unix time in milliseconds. Ordering is used only for expiry checks.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct UnixMillis(pub u64);

/// Serialize an internal millisecond clock value as the contract's canonical
/// RFC 3339 UTC instant with exactly three fractional digits.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde serialize_with callbacks receive field values by reference"
)]
pub(crate) fn serialize_utc_instant<S>(value: &UnixMillis, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    const MILLIS_PER_SECOND: u64 = 1_000;
    const SECONDS_PER_DAY: u64 = 86_400;
    const MAX_CONTRACT_MILLIS: u64 = 253_402_300_799_999;

    if value.0 > MAX_CONTRACT_MILLIS {
        return Err(serde::ser::Error::custom(
            "UTC instant exceeds the contract's four-digit year range",
        ));
    }

    let total_seconds = value.0 / MILLIS_PER_SECOND;
    let millisecond = value.0 % MILLIS_PER_SECOND;
    let days_since_epoch = total_seconds / SECONDS_PER_DAY;
    let seconds_of_day = total_seconds % SECONDS_PER_DAY;
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_date_from_epoch_days(days_since_epoch);
    serializer.serialize_str(&format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millisecond:03}Z"
    ))
}

fn civil_date_from_epoch_days(days_since_epoch: u64) -> (u64, u64, u64) {
    // Howard Hinnant's proleptic-Gregorian civil-from-days algorithm. The
    // serializer bounds the input to 0000..=9999 before reaching this helper.
    let shifted_days = days_since_epoch + 719_468;
    let era = shifted_days / 146_097;
    let day_of_era = shifted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    if month <= 2 {
        year += 1;
    }
    (year, month, day)
}

fn is_safe_component(component: &str) -> bool {
    if component.is_empty()
        || matches!(component, "." | "..")
        || component.ends_with('.')
        || component.ends_with(' ')
        || component
            .chars()
            .any(|character| matches!(character, '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return false;
    }

    let stem = component
        .split_once('.')
        .map_or(component, |(prefix, _)| prefix)
        .to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) && !is_numbered_device(&stem, "COM")
        && !is_numbered_device(&stem, "LPT")
}

fn is_numbered_device(stem: &str, prefix: &str) -> bool {
    stem.strip_prefix(prefix).is_some_and(|suffix| {
        matches!(
            suffix,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    })
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::{serialize_utc_instant, RelativeWorkspacePath, UnixMillis};

    #[derive(Serialize)]
    struct InstantFixture {
        #[serde(serialize_with = "serialize_utc_instant")]
        value: UnixMillis,
    }

    #[test]
    fn serializes_contract_utc_instants_canonically() -> Result<(), Box<dyn std::error::Error>> {
        let epoch = serde_json::to_value(InstantFixture {
            value: UnixMillis(0),
        })?;
        let leap_day = serde_json::to_value(InstantFixture {
            value: UnixMillis(951_827_696_789),
        })?;
        assert_eq!(epoch["value"], "1970-01-01T00:00:00.000Z");
        assert_eq!(leap_day["value"], "2000-02-29T12:34:56.789Z");
        Ok(())
    }

    #[test]
    fn accepts_a_canonical_relative_path() {
        let path = RelativeWorkspacePath::new("src/components/App.tsx");
        assert!(path.is_ok());
    }

    #[test]
    fn rejects_windows_escape_and_alias_forms() {
        for candidate in [
            "../secret.txt",
            "/rooted.txt",
            "C:/absolute.txt",
            "src\\file.txt",
            "file.txt:stream",
            "NUL.txt",
            "dir/COM1",
            "dir/LPT².txt",
            "CONOUT$",
            "trailing. ",
            "double//separator",
        ] {
            assert!(
                RelativeWorkspacePath::new(candidate).is_err(),
                "accepted {candidate}"
            );
        }
    }
}
