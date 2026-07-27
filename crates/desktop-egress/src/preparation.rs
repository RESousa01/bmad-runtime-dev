use std::collections::BTreeMap;

use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis};
use serde::{Deserialize, Serialize};

use crate::manifest::PreparedContextItemDraft;
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

/// Every marker [`PatternSecretScanner`] can write into outbound bytes.
///
/// These are the only strings in a context item that exist because a secret was
/// removed. Kept here, next to the code that emits them, so that a caller
/// checking for them cannot drift when a new redaction kind is added;
/// `redaction_markers_cover_every_kind_the_scanner_emits` fails if it does.
pub const REDACTION_MARKERS: [&str; 4] = [
    "[REDACTED:private_key]",
    "[REDACTED:github_token]",
    "[REDACTED:openai_token]",
    "[REDACTED:credential]",
];

/// Reports a redaction marker that `proposed` introduces and `preimage` does
/// not already carry.
///
/// # What this is for
///
/// Redaction rewrites the bytes the model is shown. A model that echoes the
/// body it was shown — which is exactly what a whole-file `replace` is — writes
/// `[REDACTED:credential]` over the user's real credential. The governed
/// change-set path re-reads the target and compares its hash against the
/// model's declared preimage, so it catches a *stale* view of the file; it
/// cannot catch a *faithful* echo of a redacted view, because the file has not
/// moved. The D3 review diff is then the only thing between that write and the
/// user's file, which makes the guarantee "the user noticed".
///
/// # Why counts and not presence
///
/// A file may legitimately contain the literal text — a redaction test corpus,
/// a support ticket quoting an egress preview, this crate's own tests. Refusing
/// every body that mentions a marker would make those files permanently
/// un-editable through the model, which is a false refusal of exactly the kind
/// this change is trying to stop producing. Comparing per-marker occurrence
/// counts refuses only the net-new marker: the write that adds one.
///
/// Returns `None` when the proposed body is safe to write.
#[must_use]
pub fn reintroduced_redaction_marker(preimage: &str, proposed: &str) -> Option<&'static str> {
    REDACTION_MARKERS
        .into_iter()
        .find(|marker| proposed.matches(marker).count() > preimage.matches(marker).count())
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
            items.push(PreparedContextItem::seal(PreparedContextItemDraft {
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
            })?);
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
    let normalized = label.as_str().to_ascii_lowercase();
    let filename = normalized.rsplit('/').next().unwrap_or_default().to_owned();
    let components: Vec<&str> = normalized.split('/').collect();
    let authority_directory = components.iter().any(|component| {
        matches!(
            *component,
            ".azure" | ".aws" | ".ssh" | ".sapphirus" | ".codex" | ".kube" | ".gnupg"
        )
    });
    let docker_credentials =
        normalized == ".docker/config.json" || normalized.ends_with("/.docker/config.json");
    let cache_or_authority_store = filename.contains("token_cache")
        || filename.contains("token-cache")
        || filename.starts_with("msal")
        || matches!(
            filename.as_str(),
            "authority.db" | "identity.db" | "accounts.json" | "tokens.json"
        );
    let private_key_extension = matches!(
        filename.rsplit_once('.').map(|(_, extension)| extension),
        Some("key" | "pem" | "p12" | "pfx")
    );
    filename == ".env"
        || filename.starts_with(".env.")
        || authority_directory
        || docker_credentials
        || cache_or_authority_store
        || private_key_extension
        || matches!(
            filename.as_str(),
            ".git-credentials"
                | ".netrc"
                | ".npmrc"
                | ".pypirc"
                | "id_rsa"
                | "id_dsa"
                | "id_ecdsa"
                | "id_ed25519"
                | "id_ecdsa_sk"
                | "id_ed25519_sk"
                | "credentials"
        )
        || filename.starts_with("credentials.")
}

fn record(findings: &mut BTreeMap<&'static str, u32>, kind: &'static str, count: u32) {
    if count > 0 {
        findings.insert(kind, count);
    }
}

const PRIVATE_KEY_MARKERS: [(&str, &str); 4] = [
    (
        concat!("-----BEGIN ", "PRIVATE KEY-----"),
        concat!("-----END ", "PRIVATE KEY-----"),
    ),
    (
        concat!("-----BEGIN ", "RSA PRIVATE KEY-----"),
        concat!("-----END ", "RSA PRIVATE KEY-----"),
    ),
    (
        concat!("-----BEGIN ", "EC PRIVATE KEY-----"),
        concat!("-----END ", "EC PRIVATE KEY-----"),
    ),
    (
        concat!("-----BEGIN ", "OPENSSH PRIVATE KEY-----"),
        concat!("-----END ", "OPENSSH PRIVATE KEY-----"),
    ),
];

/// The smallest base64 payload that can encode a private key.
///
/// The floor in the wild is an Ed25519 PKCS#8 key: 48 DER bytes, 64 base64
/// characters on one line. A SEC1 P-256 key is 164. Nothing real is shorter,
/// so 40 sits below every key and above the accidental collisions that matter
/// — a bare hex digest is 32, a UUID is 36 but carries hyphens, and a prose
/// line long enough to reach 40 has to run 40 characters without a space or a
/// punctuation mark, which the line-shape check below already forbids.
const MINIMUM_PEM_BODY_BASE64: usize = 40;

/// How many RFC 1421 header lines (`Proc-Type:`, `DEK-Info:`) and blank
/// separators may sit between an encrypted key's BEGIN marker and its body.
/// OpenSSL writes two headers and one blank line; the rest is slack.
const MAXIMUM_PEM_PROLOGUE_LINES: usize = 6;

/// The bytes a PEM body line is built from, and nothing else. Notably absent:
/// space, `.`, `,`, and `-`. That absence is what stops a body scan from
/// walking out of a key and into the sentence after it.
const fn is_pem_body_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')
}

/// True for an RFC 1421 encapsulated header such as `DEK-Info: AES-128-CBC,…`.
fn is_pem_header_line(line: &str) -> bool {
    line.split_once(':').is_some_and(|(name, _)| {
        !name.is_empty()
            && name.len() <= 32
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

/// Extends `body_end` over an END marker sitting immediately after the body,
/// tolerating one line break and any indentation. Returns `body_end` unchanged
/// when the block is unterminated, which is the truncated-key case.
fn extend_over_end_marker(rest: &str, body_end: usize, end_marker: &str) -> usize {
    let bytes = rest.as_bytes();
    let mut probe = body_end;
    if bytes.get(probe) == Some(&b'\r') {
        probe = probe.saturating_add(1);
    }
    if bytes.get(probe) == Some(&b'\n') {
        probe = probe.saturating_add(1);
    }
    while matches!(bytes.get(probe), Some(b' ' | b'\t')) {
        probe = probe.saturating_add(1);
    }
    if rest
        .get(probe..)
        .is_some_and(|tail| tail.starts_with(end_marker))
    {
        probe.saturating_add(end_marker.len())
    } else {
        body_end
    }
}

/// Measures the key block that follows a BEGIN marker, or reports that there
/// is no key block and the marker is being talked *about*.
///
/// `rest` is everything after the marker. The answer is a byte length measured
/// from the start of `rest`; `None` means "prose, leave every byte alone".
///
/// # Why extent is decided by body shape rather than by the END marker
///
/// The predecessor searched forward for the matching END marker and, when it
/// found none, redacted to the end of the input. An ops runbook that merely
/// quotes a PEM BEGIN line in a sentence therefore lost every
/// byte after that point, silently, and the model was then handed a truncated
/// document to reason about — plus a fabricated `private_key` finding on the
/// consent surface, which is the more expensive half: a reviewer taught to
/// wave through secret warnings waves through the real one too.
///
/// A key block is recognisable without its terminator. It is a run of lines
/// containing only base64 payload characters, long enough to hold a key. That
/// test is what gates redaction here, and the extent is the run itself, so an
/// unterminated block costs the body and not the document.
fn plausible_key_block(rest: &str, end_marker: &str) -> Option<usize> {
    // Shape one: the body shares the marker's line. This is the JSON-embedded
    // key — a one-line string field holding the BEGIN marker, the body and the
    // END marker, with the newlines written as the two characters backslash-n
    // rather than as real line breaks. It is how a Google service-account file
    // leaks, and any "marker must start a line" rule would miss it entirely.
    let inline_run = rest
        .bytes()
        .take_while(|byte| is_pem_body_byte(*byte) || *byte == b'\\')
        .count();
    let inline_payload = rest
        .bytes()
        .take(inline_run)
        .filter(|byte| is_pem_body_byte(*byte))
        .count();
    if inline_payload >= MINIMUM_PEM_BODY_BASE64 {
        return Some(extend_over_end_marker(rest, inline_run, end_marker));
    }

    // Shape two: the marker owns its line. Anything else left on that line is
    // a sentence continuing around the marker, not a key.
    let first_break = rest.find('\n')?;
    if !rest.get(..first_break)?.trim().is_empty() {
        return None;
    }

    let mut cursor = first_break.saturating_add(1);
    let mut payload = 0_usize;
    let mut prologue_lines = 0_usize;
    let mut body_end = cursor;
    while cursor < rest.len() {
        let tail = rest.get(cursor..)?;
        let line_break = tail.find('\n').map_or(rest.len(), |offset| cursor + offset);
        let raw = rest.get(cursor..line_break)?;
        let line = raw.trim();
        let next = line_break.saturating_add(usize::from(line_break < rest.len()));
        if !line.is_empty() && line.bytes().all(is_pem_body_byte) {
            payload = payload.saturating_add(line.len());
            body_end = cursor.saturating_add(raw.trim_end().len());
            cursor = next;
            continue;
        }
        // Encrypted PEM puts `Proc-Type:`/`DEK-Info:` and a blank line between
        // the marker and the body. Crossing those costs nothing: acceptance is
        // still decided by the payload below, so a prose line that happens to
        // look like a header only widens the search, never the verdict.
        if payload == 0
            && prologue_lines < MAXIMUM_PEM_PROLOGUE_LINES
            && (line.is_empty() || is_pem_header_line(line))
        {
            prologue_lines = prologue_lines.saturating_add(1);
            cursor = next;
            continue;
        }
        break;
    }

    (payload >= MINIMUM_PEM_BODY_BASE64).then(|| extend_over_end_marker(rest, body_end, end_marker))
}

fn redact_private_key_blocks(value: &mut String) -> u32 {
    const REPLACEMENT: &str = "[REDACTED:private_key]";
    let mut count = 0_u32;
    let mut cursor = 0_usize;
    while let Some((start, begin_length, end_marker)) = value.get(cursor..).and_then(|tail| {
        PRIVATE_KEY_MARKERS
            .iter()
            .filter_map(|(begin, end)| {
                tail.find(begin)
                    .map(|offset| (cursor + offset, begin.len(), *end))
            })
            .min_by_key(|(index, _, _)| *index)
    }) {
        let after_marker = start.saturating_add(begin_length);
        let Some(extent) = value
            .get(after_marker..)
            .and_then(|rest| plausible_key_block(rest, end_marker))
        else {
            // A marker with no key under it is prose. Advancing past it leaves
            // every byte of the sentence intact and records no finding.
            cursor = after_marker;
            continue;
        };
        value.replace_range(start..after_marker.saturating_add(extent), REPLACEMENT);
        cursor = start.saturating_add(REPLACEMENT.len());
        count = count.saturating_add(1);
    }
    count
}

/// Bytes a credential can be built from, and therefore the bytes that cannot
/// separate one from the word in front of it.
const fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

/// True when `start` begins a word rather than continuing one.
///
/// Without this, `find(prefix)` matches mid-word: `risk-assessment` contains
/// `sk-` at offset two, the scan then eats `assessment`, and a thirteen-byte
/// span clears the minimum length. Ordinary prose came back as
/// `ri[REDACTED:openai_token]` — the subject of the document deleted before it
/// reached the model, plus a fabricated `openai_token` finding on the consent
/// surface, which teaches reviewers to wave through the one signal the
/// disclosure asks them to weigh.
///
/// The check costs nothing in real detection: a credential in a file is
/// preceded by start-of-input, whitespace, `=`, `:`, a quote, or a path
/// separator, and every one of those is a boundary. A multi-byte character in
/// front leaves a continuation byte here, which is not a token byte, so it
/// reads as a boundary too.
fn starts_at_token_boundary(value: &str, start: usize) -> bool {
    start
        .checked_sub(1)
        .is_none_or(|previous| !is_token_byte(value.as_bytes()[previous]))
}

/// Whether a token body looks machine-generated rather than like English words.
///
/// The word-boundary check above stops `risk-assessment`, but it cannot stop a
/// hyphenated word that genuinely *starts* at a boundary: `sk-learn` is
/// scikit-learn, and `sk-learn-classifier-pipeline` was being rewritten to
/// `[REDACTED:openai_token]` in any Python workspace — corrupting the document
/// and raising a fabricated finding, which is the same cry-wolf cost that made
/// the `risk-assessment` bug worth fixing.
///
/// Issued tokens are random base62. Over a body of the minimum twelve bytes the
/// chance of containing neither a digit nor an uppercase letter is `(26/62)^12`,
/// about one in five million; over a real 40-byte key it is beyond negligible.
/// Lowercase hyphenated English, by contrast, never has either.
///
/// The obvious objection — that this misses a short hand-chosen key like
/// `sk-mysecretkey` — does not apply to this product. `validate_base_url`
/// (`desktop-cloud/src/direct_provider.rs:678`) pins the provider host to
/// exactly `api.openai.com`, so a local proxy issuing its own keys is rejected
/// at startup by design. Every `sk-` value that can reach here is issued by
/// that host.
fn looks_randomly_generated(body: &str) -> bool {
    body.bytes()
        .any(|byte| byte.is_ascii_digit() || byte.is_ascii_uppercase())
}

fn redact_prefixed_tokens(
    value: &mut String,
    prefix: &str,
    kind: &'static str,
    minimum_bytes: usize,
) -> u32 {
    debug_assert!(
        prefix.bytes().all(is_token_byte),
        "a prefix built from non-token bytes would break the skip below"
    );
    let replacement = format!("[REDACTED:{kind}]");
    let mut count = 0_u32;
    let mut cursor = 0_usize;
    while let Some(relative_start) = value[cursor..].find(prefix) {
        let start = cursor + relative_start;
        let mut end = start + prefix.len();
        for byte in value.as_bytes()[end..].iter().copied() {
            if is_token_byte(byte) {
                end += 1;
            } else {
                break;
            }
        }
        if !starts_at_token_boundary(value, start)
            || end - start < minimum_bytes
            || !looks_randomly_generated(&value[start + prefix.len()..end])
        {
            // Everything in `start..end` is a token byte, so any further
            // occurrence inside this span is mid-word too and would fail the
            // same boundary check. Skipping the span keeps the walk linear
            // without hiding a candidate.
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
