use std::collections::{BTreeMap, BTreeSet};

use desktop_runtime::{
    canonical_hash, sha256_bytes, ContractId, RelativeWorkspacePath, Sha256Digest, UnixMillis,
};
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
/// removed. Kept next to the code that emits them so the two stay visible to
/// each other.
///
/// **This is a second copy of the kind names**, and the covering test cannot
/// detect the drift that matters. `redaction_markers_cover_every_kind_the_scanner_emits`
/// scans a hand-written corpus and compares the markers that corpus *happened to
/// produce* against this list. A new `record(&mut findings, "slack_token", …)`
/// in `scan` that nobody adds a corpus entry for emits nothing during the test,
/// so both directions are vacuously satisfied and the guard silently stops
/// covering the new kind — meaning a model echoing a redacted view could write
/// `[REDACTED:slack_token]` over a real token, which is exactly what
/// `reintroduced_redaction_marker` exists to prevent.
///
/// Adding a redaction kind therefore means adding it here **and** adding a
/// triggering entry to that corpus. The durable fix is to derive both from one
/// list of kinds that `scan` itself iterates, which needs the four redactors to
/// share a signature.
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

/// The version two local egress manifest schema identifier.
///
/// It is the `schemaVersion` member of the hashed draft below, and it is
/// deliberately different from the version one manifest's so that no version one
/// draft can ever hash to a version two manifest even if the two were otherwise
/// byte-identical.
pub const LOCAL_EGRESS_MANIFEST_V2_SCHEMA: &str = "sapphirus.context-egress-manifest.v2";

/// The mode one model request runs in. Mirrors
/// `model-profile.schema.json#/$defs/ModelRequestMode` without importing the
/// generated contracts into this adapter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRequestMode {
    StructuredResponse,
    StreamingChat,
}

/// One context part, reduced to exactly what both languages can see.
///
/// The bytes themselves are bound by `content_hash` rather than carried, so a
/// server that holds the request can recompute this without the desktop having
/// to restate the content, and a single changed byte changes the manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalEgressManifestV2Item {
    pub client_item_id: ContractId,
    pub relative_label: RelativeWorkspacePath,
    pub semantic_role: String,
    pub language: String,
    pub content_hash: Sha256Digest,
    pub byte_count: u64,
    pub classification: ContextClassification,
}

/// One declared tool, reduced to the parts that actually leave the machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalEgressManifestV2Tool {
    pub tool_name: String,
    pub description: String,
    pub parameters_schema_id: ContractId,
    pub parameters_schema_hash: Sha256Digest,
}

/// Everything the version two local egress manifest hash is computed over.
///
/// Every member is present in `model-access-request-v2.schema.json`, which is
/// what makes this preimage recomputable by the support plane rather than only
/// comparable to a number the desktop asserted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalEgressManifestV2Input {
    pub request_id: ContractId,
    pub registration_id: ContractId,
    pub purpose: String,
    pub model_role: String,
    pub request_mode: ModelRequestMode,
    pub model_profile_id: String,
    pub requested_variant_id: Option<String>,
    pub catalog_version: String,
    pub model_profile_hash: Sha256Digest,
    pub model_capability_hash: Sha256Digest,
    pub context_snapshot_id: ContractId,
    pub canonical_output_schema_id: Option<ContractId>,
    pub canonical_output_schema_hash: Option<Sha256Digest>,
    pub retention_mode: RetentionMode,
    pub budget_class: String,
    pub items: Vec<LocalEgressManifestV2Item>,
    pub tools: Vec<LocalEgressManifestV2Tool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalEgressManifestV2ItemDraft<'a> {
    byte_count: u64,
    classification: ContextClassification,
    client_item_id: &'a ContractId,
    content_hash: Sha256Digest,
    language: &'a str,
    relative_label: &'a RelativeWorkspacePath,
    semantic_role: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalEgressManifestV2ToolDraft<'a> {
    description: &'a str,
    parameters_schema_hash: Sha256Digest,
    parameters_schema_id: &'a ContractId,
    tool_name: &'a str,
}

/// The hashed document. Every nullable member is written as JSON `null` rather
/// than omitted, matching the version two consent envelope, so a request that
/// names no variant and one that names the literal string `"null"` are
/// different preimages.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalEgressManifestV2Draft<'a> {
    schema_version: &'static str,
    request_id: &'a ContractId,
    registration_id: &'a ContractId,
    purpose: &'a str,
    model_role: &'a str,
    request_mode: ModelRequestMode,
    model_profile_id: &'a str,
    requested_variant_id: Option<&'a str>,
    catalog_version: &'a str,
    model_profile_hash: Sha256Digest,
    model_capability_hash: Sha256Digest,
    context_snapshot_id: &'a ContractId,
    canonical_output_schema_id: Option<&'a ContractId>,
    canonical_output_schema_hash: Option<Sha256Digest>,
    retention_mode: RetentionMode,
    budget_class: &'a str,
    items: Vec<LocalEgressManifestV2ItemDraft<'a>>,
    tools: Vec<LocalEgressManifestV2ToolDraft<'a>>,
    total_outbound_bytes: u64,
}

/// Computes the canonical version two local egress manifest hash.
///
/// # The gap this closes
///
/// Version one of the desktop request carried a manifest hash the support plane
/// could only compare against the copy inside the signed decision, because the
/// version one manifest is hashed over local authority — tenant, project and run
/// references, exclusions, secret findings, token estimates — that never crosses
/// the wire. Agreement between two numbers the same client produced is not a
/// server-side check.
///
/// This preimage is built only from members of
/// `model-access-request-v2.schema.json`, so the support plane can recompute it
/// from the request it received and refuse a decision that was signed over
/// different context, a different profile, a different catalog epoch, or a
/// different tool set. The item bytes are represented by `contentHash` and
/// `byteCount`, so the exact outbound bytes stay bound without the manifest
/// restating them.
///
/// The preimage is `sapphirus:context-egress-manifest:v2\n` followed by the
/// RFC 8785 canonical JSON of the draft above.
///
/// # Errors
///
/// Returns [`EgressError::InvalidManifest`] when a member is empty, the item
/// list is empty or holds a duplicate identifier, or the retention mode is not
/// the canonical transient mode; and [`EgressError::ContextBudgetExceeded`] when
/// the outbound byte total overflows.
pub fn local_egress_manifest_hash_v2(
    input: &LocalEgressManifestV2Input,
) -> Result<Sha256Digest, EgressError> {
    validate_local_egress_manifest_v2(input)?;
    let mut total_outbound_bytes = 0_u64;
    for item in &input.items {
        total_outbound_bytes = total_outbound_bytes
            .checked_add(item.byte_count)
            .ok_or(EgressError::ContextBudgetExceeded)?;
    }
    let draft = LocalEgressManifestV2Draft {
        schema_version: LOCAL_EGRESS_MANIFEST_V2_SCHEMA,
        request_id: &input.request_id,
        registration_id: &input.registration_id,
        purpose: &input.purpose,
        model_role: &input.model_role,
        request_mode: input.request_mode,
        model_profile_id: &input.model_profile_id,
        requested_variant_id: input.requested_variant_id.as_deref(),
        catalog_version: &input.catalog_version,
        model_profile_hash: input.model_profile_hash,
        model_capability_hash: input.model_capability_hash,
        context_snapshot_id: &input.context_snapshot_id,
        canonical_output_schema_id: input.canonical_output_schema_id.as_ref(),
        canonical_output_schema_hash: input.canonical_output_schema_hash,
        retention_mode: input.retention_mode,
        budget_class: &input.budget_class,
        items: input
            .items
            .iter()
            .map(|item| LocalEgressManifestV2ItemDraft {
                byte_count: item.byte_count,
                classification: item.classification,
                client_item_id: &item.client_item_id,
                content_hash: item.content_hash,
                language: &item.language,
                relative_label: &item.relative_label,
                semantic_role: &item.semantic_role,
            })
            .collect(),
        tools: input
            .tools
            .iter()
            .map(|tool| LocalEgressManifestV2ToolDraft {
                description: &tool.description,
                parameters_schema_hash: tool.parameters_schema_hash,
                parameters_schema_id: &tool.parameters_schema_id,
                tool_name: &tool.tool_name,
            })
            .collect(),
        total_outbound_bytes,
    };
    canonical_hash("context-egress-manifest", 2, &draft).map_err(|_| EgressError::CanonicalHash)
}

fn validate_local_egress_manifest_v2(
    input: &LocalEgressManifestV2Input,
) -> Result<(), EgressError> {
    if input.retention_mode != RetentionMode::TransientNoStore
        || input.items.is_empty()
        || input.purpose.is_empty()
        || input.model_role.is_empty()
        || input.model_profile_id.is_empty()
        || input.catalog_version.is_empty()
        || input.budget_class.is_empty()
        || input
            .requested_variant_id
            .as_ref()
            .is_some_and(String::is_empty)
        // A schema identifier and its hash are one fact. Carrying one without
        // the other would let a structured request be signed with an unbound
        // output schema.
        || input.canonical_output_schema_id.is_some()
            != input.canonical_output_schema_hash.is_some()
    {
        return Err(EgressError::InvalidManifest);
    }
    let mut seen = BTreeSet::new();
    for item in &input.items {
        if item.language.is_empty()
            || item.semantic_role.is_empty()
            || !seen.insert(item.client_item_id.as_str())
        {
            return Err(EgressError::InvalidManifest);
        }
    }
    let mut seen_tools = BTreeSet::new();
    for tool in &input.tools {
        if tool.tool_name.is_empty() || !seen_tools.insert(tool.tool_name.as_str()) {
            return Err(EgressError::InvalidManifest);
        }
    }
    Ok(())
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
///
/// The rule is deliberately narrow: **prose is lowercase words joined by
/// hyphens.** `learn-classifier-pipeline` and `hynix-memory-modules` are that;
/// an issued key never is.
///
/// An earlier version of this only asked whether the body held a digit or an
/// uppercase letter, justified by `validate_base_url` pinning the provider host
/// to `api.openai.com`. That argument was a category error and it opened a real
/// leak. `validate_base_url` constrains *the host this app calls*. This scanner
/// runs over *arbitrary workspace files the user attaches to a turn*, which can
/// hold `sk-` secrets from anything — a `LiteLLM` or `vLLM` proxy master key, an
/// `OpenRouter` key, a colleague's key pasted into a document. Under that gate
/// `sk-devteamsharedkey` was all lowercase, so it left the machine verbatim with
/// no finding on the consent surface. It had been redacted before.
///
/// Requiring a hyphen closes it: a hyphenless lowercase body is treated as a
/// key again, which is the direction to be wrong in. `sk-proj-`-style keys carry
/// digits and uppercase anyway and never reach the hyphen test.
fn looks_randomly_generated(body: &str) -> bool {
    let hyphenated_lowercase_words = body.contains('-')
        && body
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-');
    !hyphenated_lowercase_words
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
