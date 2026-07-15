use std::collections::BTreeMap;

use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis};
use serde::{Deserialize, Serialize};

use crate::{
    ContextClassification, ContextEgressManifest, ContextEgressManifestDraft, ContextExclusion,
    EgressError, EgressLimits, PreparedContextItem, RedactionRecord, RetentionMode, SecretFinding,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextCandidate {
    pub client_item_id: ContractId,
    pub relative_label: RelativeWorkspacePath,
    pub semantic_role: String,
    pub language: Option<String>,
    pub classification: ContextClassification,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareContextInput {
    pub tenant_ref: ContractId,
    pub project_ref: ContractId,
    pub run_ref: ContractId,
    pub purpose: String,
    pub model_role: String,
    pub canonical_output_schema_id: ContractId,
    pub canonical_output_schema_hash: Sha256Digest,
    pub provider_profile_hash: Sha256Digest,
    pub model_profile_hash: Sha256Digest,
    pub deployment_hash: Sha256Digest,
    pub policy_hash: Sha256Digest,
    pub region: String,
    pub retention_mode: RetentionMode,
    pub created_at: UnixMillis,
    pub expires_at: UnixMillis,
    pub limits: EgressLimits,
    pub candidates: Vec<ContextCandidate>,
    pub exclusions: Vec<ContextExclusion>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretScanFinding {
    pub kind: String,
    pub occurrence_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretScanResult {
    pub outbound_content: String,
    pub findings: Vec<SecretScanFinding>,
}

pub trait SecretScanner: Send + Sync {
    fn scan(&self, content: &str) -> SecretScanResult;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PatternSecretScanner;

impl SecretScanner for PatternSecretScanner {
    fn scan(&self, content: &str) -> SecretScanResult {
        let mut outbound = content.to_owned();
        let mut findings = BTreeMap::<&'static str, u32>::new();

        record(
            &mut findings,
            "private_key",
            redact_private_key_blocks(&mut outbound),
        );
        record(
            &mut findings,
            "github_token",
            redact_prefixed_tokens(&mut outbound, "ghp_", "github_token", 12),
        );
        record(
            &mut findings,
            "openai_token",
            redact_prefixed_tokens(&mut outbound, "sk-", "openai_token", 12),
        );
        record(
            &mut findings,
            "credential",
            redact_assignments(&mut outbound),
        );

        SecretScanResult {
            outbound_content: outbound,
            findings: findings
                .into_iter()
                .map(|(kind, occurrence_count)| SecretScanFinding {
                    kind: kind.to_owned(),
                    occurrence_count,
                })
                .collect(),
        }
    }
}

pub struct ContextPreparer<S> {
    scanner: S,
}

impl<S> ContextPreparer<S>
where
    S: SecretScanner,
{
    #[must_use]
    pub const fn new(scanner: S) -> Self {
        Self { scanner }
    }

    /// Denies secret-bearing labels, redacts candidate bytes, and seals the
    /// exact outbound context manifest.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError`] when a label is denied, a size calculation
    /// overflows, or the prepared manifest fails semantic validation.
    pub fn prepare(
        &self,
        input: PrepareContextInput,
    ) -> Result<ContextEgressManifest, EgressError> {
        if input
            .candidates
            .iter()
            .any(|candidate| is_denied_label(&candidate.relative_label))
        {
            return Err(EgressError::DeniedContextLabel);
        }

        let mut items = Vec::with_capacity(input.candidates.len());
        let mut secret_findings = Vec::new();
        let mut total_outbound_bytes = 0_u64;
        let mut total_token_estimate = 0_u64;

        for candidate in input.candidates {
            let scan = self.scanner.scan(&candidate.content);
            let outbound_byte_count = u64::try_from(scan.outbound_content.len())
                .map_err(|_| EgressError::ContextBudgetExceeded)?;
            let original_byte_count = u64::try_from(candidate.content.len())
                .map_err(|_| EgressError::ContextBudgetExceeded)?;
            let token_estimate = outbound_byte_count
                .checked_add(3)
                .ok_or(EgressError::ContextBudgetExceeded)?
                / 4;
            total_outbound_bytes = total_outbound_bytes
                .checked_add(outbound_byte_count)
                .ok_or(EgressError::ContextBudgetExceeded)?;
            total_token_estimate = total_token_estimate
                .checked_add(token_estimate)
                .ok_or(EgressError::ContextBudgetExceeded)?;

            let redactions = scan
                .findings
                .iter()
                .map(|finding| RedactionRecord {
                    kind: finding.kind.clone(),
                    occurrence_count: finding.occurrence_count,
                })
                .collect();
            secret_findings.extend(scan.findings.into_iter().map(|finding| SecretFinding {
                client_item_id: candidate.client_item_id.clone(),
                kind: finding.kind,
                occurrence_count: finding.occurrence_count,
            }));
            items.push(PreparedContextItem {
                client_item_id: candidate.client_item_id,
                relative_label: candidate.relative_label,
                semantic_role: candidate.semantic_role,
                language: candidate.language,
                original_content_hash: sha256_bytes(candidate.content.as_bytes()),
                outbound_content_hash: sha256_bytes(scan.outbound_content.as_bytes()),
                original_byte_count,
                outbound_byte_count,
                token_estimate,
                classification: candidate.classification,
                redactions,
                outbound_content: scan.outbound_content,
            });
        }

        ContextEgressManifestDraft {
            schema_version: "sapphirus.context-egress-manifest.v1".to_owned(),
            tenant_ref: input.tenant_ref,
            project_ref: input.project_ref,
            run_ref: input.run_ref,
            purpose: input.purpose,
            model_role: input.model_role,
            canonical_output_schema_id: input.canonical_output_schema_id,
            canonical_output_schema_hash: input.canonical_output_schema_hash,
            provider_profile_hash: input.provider_profile_hash,
            model_profile_hash: input.model_profile_hash,
            deployment_hash: input.deployment_hash,
            policy_hash: input.policy_hash,
            region: input.region,
            retention_mode: input.retention_mode,
            created_at: input.created_at,
            expires_at: input.expires_at,
            limits: input.limits,
            items,
            exclusions: input.exclusions,
            secret_findings,
            total_outbound_bytes,
            total_token_estimate,
        }
        .seal()
    }
}

fn is_denied_label(label: &RelativeWorkspacePath) -> bool {
    let filename = label
        .as_str()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    filename == ".env"
        || filename.starts_with(".env.")
        || matches!(
            filename.as_str(),
            ".npmrc" | "id_rsa" | "id_ed25519" | "credentials"
        )
        || filename.starts_with("credentials.")
}

fn record(findings: &mut BTreeMap<&'static str, u32>, kind: &'static str, count: u32) {
    if count > 0 {
        findings.insert(kind, count);
    }
}

fn redact_private_key_blocks(value: &mut String) -> u32 {
    const MARKERS: [(&str, &str); 4] = [
        ("-----BEGIN PRIVATE KEY-----", "-----END PRIVATE KEY-----"),
        (
            "-----BEGIN RSA PRIVATE KEY-----",
            "-----END RSA PRIVATE KEY-----",
        ),
        (
            "-----BEGIN EC PRIVATE KEY-----",
            "-----END EC PRIVATE KEY-----",
        ),
        (
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "-----END OPENSSH PRIVATE KEY-----",
        ),
    ];
    let mut count = 0_u32;
    while let Some((start, end_marker)) = MARKERS
        .iter()
        .filter_map(|(begin, end)| value.find(begin).map(|index| (index, *end)))
        .min_by_key(|(index, _)| *index)
    {
        let search_start = start.saturating_add(1);
        let end = value[search_start..]
            .find(end_marker)
            .map_or(value.len(), |relative| {
                search_start + relative + end_marker.len()
            });
        value.replace_range(start..end, "[REDACTED:private_key]");
        count = count.saturating_add(1);
    }
    count
}

fn redact_prefixed_tokens(
    value: &mut String,
    prefix: &str,
    kind: &'static str,
    minimum_bytes: usize,
) -> u32 {
    let replacement = format!("[REDACTED:{kind}]");
    let mut count = 0_u32;
    let mut cursor = 0_usize;
    while let Some(relative_start) = value[cursor..].find(prefix) {
        let start = cursor + relative_start;
        let mut end = start + prefix.len();
        for byte in value.as_bytes()[end..].iter().copied() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-') {
                end += 1;
            } else {
                break;
            }
        }
        if end - start < minimum_bytes {
            cursor = end;
            continue;
        }
        value.replace_range(start..end, &replacement);
        cursor = start + replacement.len();
        count = count.saturating_add(1);
    }
    count
}

fn redact_assignments(value: &mut String) -> u32 {
    const PREFIXES: [&str; 8] = [
        "API_KEY=",
        "ACCESS_TOKEN=",
        "PASSWORD=",
        "SECRET=",
        "api_key=",
        "access_token=",
        "password=",
        "secret=",
    ];
    let replacement = "[REDACTED:credential]";
    let mut count = 0_u32;
    for prefix in PREFIXES {
        let mut cursor = 0_usize;
        while let Some(relative_start) = value[cursor..].find(prefix) {
            let start = cursor + relative_start + prefix.len();
            let mut end = start;
            for character in value[start..].chars() {
                if character.is_whitespace() || matches!(character, '\'' | '"') {
                    break;
                }
                end += character.len_utf8();
            }
            if end == start {
                cursor = start;
                continue;
            }
            value.replace_range(start..end, replacement);
            cursor = start + replacement.len();
            count = count.saturating_add(1);
        }
    }
    count
}
