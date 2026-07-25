//! Minimal host-owned `OpenAI` Responses transport for the Forge prototype.
//!
//! The route and environment conventions are adapted from `OpenCode`'s `OpenAI`
//! and Azure provider facades. A runtime credential enters through the narrow
//! sign-in IPC command, is immediately moved into zeroizing native memory, and
//! is never returned or included in serialized projections. This mode is
//! explicitly development only: it creates a local no-trust receipt marker
//! rather than a production support-plane signature.

use std::env;
use std::fmt;
use std::time::Duration;

use desktop_runtime::{deserialize_strict_json, sha256_bytes, ContractId, UnixMillis};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde_json::{json, Value};
use time::OffsetDateTime;
use url::Url;
use zeroize::Zeroizing;

use crate::model::MAX_OUTPUT_BYTES;
use crate::{
    AuthorizedModelRequest, CloudError, DispatchedModelRequest, ModelAccessReceipt,
    ModelReceiptStatus, RawModelOutput,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const OPERATION_TIMEOUT: Duration = Duration::from_mins(2);
const MAX_PROVIDER_RESPONSE_BYTES: u64 = MAX_OUTPUT_BYTES as u64;
const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1/";
const AZURE_API_VERSION: &str = "v1";
const DIRECT_RECEIPT_SCHEMA: &str = "sapphirus.model-access-receipt.v1";
const HELP_MODEL_INSTRUCTIONS: &str = "You are Forge's BMAD help router. Use only the exact reviewed context. Never invent package, capability, or evidence identifiers. Return exactly one JSON object and no other text. For a recommendation use {\"proposalKind\":\"recommended_capability\",\"capabilityKey\":{\"packageVersionId\":\"...\",\"moduleCode\":\"...\",\"skillName\":\"...\",\"normalizedAction\":null},\"evidenceTokenIds\":[\"...\"],\"rationaleSummary\":\"...\"}. When exact catalog evidence is absent use {\"proposalKind\":\"no_recommendation\",\"reasonCode\":\"catalog_evidence_absent\"}; the only other allowed reason codes are completion_evidence_ambiguous and dependency_unavailable.";

/// Explicit marker accepted only by Forge's development receipt verifier.
pub const DIRECT_PROVIDER_DEVELOPMENT_PROOF: &str = "forge-direct-provider-development-no-trust";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectProviderKind {
    OpenAi,
    AzureOpenAi,
}

impl DirectProviderKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::AzureOpenAi => "azure",
        }
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::AzureOpenAi => "Azure OpenAI",
        }
    }
}

#[derive(Clone)]
pub struct DirectProviderConfig {
    kind: DirectProviderKind,
    base_url: Url,
    model: String,
    region: String,
    credential: Zeroizing<String>,
}

impl fmt::Debug for DirectProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DirectProviderConfig")
            .field("kind", &self.kind)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("region", &self.region)
            .field("credential", &"[REDACTED]")
            .finish()
    }
}

impl DirectProviderConfig {
    /// Resolves a renderer-selected provider/model pair against credentials
    /// owned by the native process environment. The selection is non-secret
    /// and may cross IPC; this legacy startup path does not accept secrets.
    ///
    /// A missing credential leaves the selected provider unavailable instead
    /// of turning the selection command into a secret-entry API.
    ///
    /// # Errors
    ///
    /// Returns a stable configuration error when the model identifier,
    /// credential, or provider endpoint metadata is malformed or unsafe.
    pub fn from_selection(
        kind: DirectProviderKind,
        model: &str,
    ) -> Result<Option<Self>, CloudError> {
        if !valid_identifier(model, 128) {
            return Err(CloudError::InvalidSupportOrigin);
        }
        match kind {
            DirectProviderKind::OpenAi => {
                let Some(credential) = optional_environment("OPENAI_API_KEY")? else {
                    return Ok(None);
                };
                let base_url = optional_environment("OPENAI_BASE_URL")?
                    .unwrap_or_else(|| OPENAI_DEFAULT_BASE_URL.to_owned());
                Self::new_openai(&base_url, model, credential).map(Some)
            }
            DirectProviderKind::AzureOpenAi => {
                let Some(credential) = optional_environment("AZURE_OPENAI_API_KEY")? else {
                    return Ok(None);
                };
                let region = optional_environment("AZURE_OPENAI_REGION")?
                    .unwrap_or_else(|| "azure".to_owned());
                let base_url = if let Some(value) = optional_environment("AZURE_OPENAI_BASE_URL")? {
                    value
                } else {
                    let resource = optional_environment("AZURE_OPENAI_RESOURCE_NAME")?
                        .or(optional_environment("AZURE_RESOURCE_NAME")?)
                        .ok_or(CloudError::InvalidSupportOrigin)?;
                    if !valid_resource_name(&resource) {
                        return Err(CloudError::InvalidSupportOrigin);
                    }
                    format!("https://{resource}.openai.azure.com/openai/v1/")
                };
                Self::new_azure(&base_url, model, &region, credential).map(Some)
            }
        }
    }

    /// Builds a direct-provider configuration from a short-lived secret
    /// supplied to the native host at runtime. The secret is immediately moved
    /// into zeroizing storage and is never exposed by the configuration.
    ///
    /// # Errors
    ///
    /// Returns a closed configuration error for unsafe provider metadata or
    /// credentials.
    pub fn from_runtime_credential(
        kind: DirectProviderKind,
        model: &str,
        credential: String,
        endpoint: Option<&str>,
        region: Option<&str>,
    ) -> Result<Self, CloudError> {
        match kind {
            DirectProviderKind::OpenAi => {
                if endpoint.is_some() || region.is_some() {
                    return Err(CloudError::InvalidSupportOrigin);
                }
                Self::new_openai(OPENAI_DEFAULT_BASE_URL, model, credential)
            }
            DirectProviderKind::AzureOpenAi => {
                let endpoint = endpoint.ok_or(CloudError::InvalidSupportOrigin)?;
                let region = region.unwrap_or("azure");
                Self::new_azure(endpoint, model, region, credential)
            }
        }
    }

    /// Rebinds the same in-memory connection to another renderer-selected
    /// model without asking the renderer to resend its secret.
    ///
    /// # Errors
    ///
    /// Returns a closed configuration error for an invalid model identifier.
    pub fn with_model(&self, model: &str) -> Result<Self, CloudError> {
        Self::new(
            self.kind,
            self.base_url.clone(),
            model,
            &self.region,
            self.credential.to_string(),
        )
    }

    fn new_openai(base_url: &str, model: &str, credential: String) -> Result<Self, CloudError> {
        let base_url = validate_base_url(base_url, DirectProviderKind::OpenAi)?;
        Self::new(
            DirectProviderKind::OpenAi,
            base_url,
            model,
            "global",
            credential,
        )
    }

    fn new_azure(
        base_url: &str,
        model: &str,
        region: &str,
        credential: String,
    ) -> Result<Self, CloudError> {
        let base_url = validate_base_url(base_url, DirectProviderKind::AzureOpenAi)?;
        Self::new(
            DirectProviderKind::AzureOpenAi,
            base_url,
            model,
            region,
            credential,
        )
    }

    fn new(
        kind: DirectProviderKind,
        base_url: Url,
        model: &str,
        region: &str,
        credential: String,
    ) -> Result<Self, CloudError> {
        if !valid_identifier(model, 128)
            || !valid_identifier(region, 64)
            || credential.len() < 8
            || credential.len() > 4_096
            || credential.chars().any(char::is_control)
        {
            return Err(CloudError::InvalidSupportOrigin);
        }
        Ok(Self {
            kind,
            base_url,
            model: model.to_owned(),
            region: region.to_owned(),
            credential: Zeroizing::new(credential),
        })
    }

    #[must_use]
    pub const fn kind(&self) -> DirectProviderKind {
        self.kind
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub fn region(&self) -> &str {
        &self.region
    }

    #[must_use]
    pub fn destination_label(&self) -> String {
        format!("{} · {}", self.kind.display_name(), self.model)
    }

    fn responses_url(&self) -> Result<Url, CloudError> {
        let mut url = self
            .base_url
            .join("responses")
            .map_err(|_| CloudError::InvalidSupportOrigin)?;
        if self.kind == DirectProviderKind::AzureOpenAi {
            url.query_pairs_mut()
                .append_pair("api-version", AZURE_API_VERSION);
        }
        Ok(url)
    }
}

pub struct DirectProviderTransport {
    config: DirectProviderConfig,
    client: reqwest::Client,
}

impl fmt::Debug for DirectProviderTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DirectProviderTransport")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl DirectProviderTransport {
    /// Builds the bounded, no-redirect HTTPS client used for direct prototype
    /// calls.
    ///
    /// # Errors
    ///
    /// Returns [`CloudError::TransportFailed`] when the client cannot be built.
    pub fn new(config: DirectProviderConfig) -> Result<Self, CloudError> {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(OPERATION_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .user_agent("forge-desktop/0.1")
            .build()
            .map_err(|_| CloudError::TransportFailed)?;
        Ok(Self { config, client })
    }

    /// Sends one already-authorized request through `OpenAI`'s Responses shape.
    /// Azure uses the same payload and a resource-scoped base URL, matching the
    /// route used by `OpenCode`'s Azure provider facade.
    ///
    /// # Errors
    ///
    /// Returns only stable model/transport errors. Provider bodies and API keys
    /// are never returned or logged.
    pub async fn send(
        &self,
        request: AuthorizedModelRequest,
        started_at: UnixMillis,
    ) -> Result<(DispatchedModelRequest, RawModelOutput), CloudError> {
        request.verify()?;
        let body = responses_request(&request, self.config.model())?;
        let body = serde_json::to_vec(&body).map_err(|_| CloudError::TransportFailed)?;
        if body.len() > MAX_OUTPUT_BYTES {
            return Err(CloudError::TransportFailed);
        }
        let url = self.config.responses_url()?;
        let builder = self
            .client
            .post(url)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json")
            .body(body);
        let builder = match self.config.kind {
            DirectProviderKind::OpenAi => builder.bearer_auth(self.config.credential.as_str()),
            DirectProviderKind::AzureOpenAi => {
                builder.header("api-key", self.config.credential.as_str())
            }
        };
        let response = builder
            .send()
            .await
            .map_err(|_| CloudError::TransportFailed)?;
        let response_json = read_bounded_json_response(response).await?;
        if response_json.get("status").and_then(Value::as_str) != Some("completed") {
            return Err(CloudError::InvalidModelOutput);
        }
        let payload_json = extract_output_text(&response_json)?;
        if payload_json.len() > MAX_OUTPUT_BYTES {
            return Err(CloudError::InvalidModelOutput);
        }
        parse_untrusted_json(payload_json.as_bytes())?;
        let payload_hash = sha256_bytes(payload_json.as_bytes());
        let completed_at = now_millis()?.max(started_at);
        let receipt_seed = format!("{}:{}", request.request_id(), payload_hash);
        let receipt_hash = sha256_bytes(receipt_seed.as_bytes());
        let receipt_id = ContractId::new(format!(
            "receipt_direct_{}",
            &hex::encode(receipt_hash.as_bytes())[..32]
        ))
        .map_err(|_| CloudError::ReceiptInvalid)?;
        let output_bytes =
            u64::try_from(payload_json.len()).map_err(|_| CloudError::InvalidModelOutput)?;
        let output = RawModelOutput {
            request_id: request.request_id().clone(),
            output_schema_id: request.canonical_output_schema_id().clone(),
            payload_json,
            payload_hash,
            receipt: ModelAccessReceipt {
                schema_version: DIRECT_RECEIPT_SCHEMA.to_owned(),
                receipt_id,
                request_id: request.request_id().clone(),
                request_hash: request.request_hash(),
                result_hash: payload_hash,
                manifest_hash: request.manifest_hash(),
                binding_hash: request.binding_hash(),
                consumption_hash: request.consumption_hash(),
                consent_disclosure_hash: request.consent_disclosure_hash(),
                provider_profile_hash: request.provider_profile_hash(),
                model_profile_hash: request.model_profile_hash(),
                deployment_hash: request.deployment_hash(),
                retention_mode: request.retention_mode(),
                region: request.region().to_owned(),
                input_bytes: request.total_outbound_bytes(),
                output_bytes,
                started_at,
                completed_at,
                status: ModelReceiptStatus::Succeeded,
                proof: DIRECT_PROVIDER_DEVELOPMENT_PROOF.to_owned(),
            },
        };
        Ok((DispatchedModelRequest::new(request), output))
    }
}

async fn read_bounded_json_response(mut response: reqwest::Response) -> Result<Value, CloudError> {
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > MAX_PROVIDER_RESPONSE_BYTES)
        || !response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_ascii_lowercase().starts_with("application/json"))
    {
        return Err(CloudError::TransportFailed);
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or_default(),
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| CloudError::TransportFailed)?
    {
        let next_length = bytes
            .len()
            .checked_add(chunk.len())
            .ok_or(CloudError::TransportFailed)?;
        if next_length > MAX_OUTPUT_BYTES {
            return Err(CloudError::TransportFailed);
        }
        bytes.extend_from_slice(&chunk);
    }
    parse_untrusted_json(&bytes)
}

fn parse_untrusted_json(bytes: &[u8]) -> Result<Value, CloudError> {
    deserialize_strict_json(bytes).map_err(|_| CloudError::InvalidModelOutput)
}

fn responses_request(request: &AuthorizedModelRequest, model: &str) -> Result<Value, CloudError> {
    let reviewed_context = request
        .items()
        .iter()
        .map(|item| {
            format!(
                "--- {} ({}; reviewedContentHash={}) ---\n{}",
                item.relative_label, item.semantic_role, item.content_hash, item.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let instructions = model_instructions(request.canonical_output_schema_id().as_str());
    if instructions.is_empty() {
        return Err(CloudError::InvalidModelOutput);
    }
    Ok(json!({
        "model": model,
        "store": false,
        "instructions": instructions,
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": format!("Purpose: {}\nModel role: {}\n\nReviewed context:\n{}", request.purpose(), request.model_role(), reviewed_context)
            }]
        }],
        "text": {
            "format": {
                "type": "json_object"
            }
        }
    }))
}

fn model_instructions(schema_id: &str) -> &'static str {
    match schema_id {
        "sapphirus.bmad-method-help-proposal.v1" => HELP_MODEL_INSTRUCTIONS,
        "sapphirus.chat-turn.v1" => "You are a helpful assistant in a desktop workspace application. The reviewed context contains the conversation so far, oldest first, and may also contain an agent persona to adopt and workspace files the user attached. Answer the newest user message directly and conversationally. Return exactly one JSON object and no other text. The root object must contain exactly schemaVersion=sapphirus.chat-turn.v1 and text. text must be your complete reply as a single plain-text string; Markdown is allowed inside it. Never invent the contents of a file that was not supplied. You cannot read, write, or execute anything: if the user asks for an effect, say what you would do and let them run it.",
        "sapphirus.bmad-document-artifact.v1" => "Use only the reviewed context. Return exactly one JSON object and no other text. The root object must contain exactly resultKind=document_artifact and documentArtifact. documentArtifact must contain exactly schemaVersion=sapphirus.bmad-document-artifact.v1, title, sections, evidenceRefs, and openQuestions, with optional mermaidText. sections must be a non-empty array of objects that each contain exactly heading and body. evidenceRefs and openQuestions must be arrays. Do not invent reference identifiers.",
        "sapphirus.bmad-governed-change-set.v1" => "Use only the reviewed context. Return exactly one JSON object and no other text. The root object must contain exactly resultKind=governed_change_set and governedChangeSet. governedChangeSet must contain exactly schemaVersion=sapphirus.bmad-governed-change-set.v1, summary, and changes. changes must be a non-empty array. Every change must match exactly one closed variant: operation=create must contain exactly operation, path, and complete UTF-8 content; operation=replace must contain exactly operation, path, complete UTF-8 content, and preimageSha256; operation=delete must contain exactly operation, path, and preimageSha256. Every path must be workspace-relative. For replace and delete, preimageSha256 must copy the reviewedContentHash shown for the matching path in the reviewed context; never invent a hash. Never emit modify, shell commands, absolute paths, or extra keys.",
        "sapphirus.bmad-inactive-builder-draft.v1" => "Use only the reviewed context. Return exactly one JSON object and no other text. The root object must contain exactly resultKind=inactive_builder_draft and inactiveBuilderDraft. inactiveBuilderDraft must contain exactly schemaVersion=sapphirus.bmad-inactive-builder-draft.v1, draftKind, title, revisionNote, and files. draftKind must be agent or workflow. files must be a non-empty array of objects that each contain exactly path and content, and every path must be workspace-relative. This is inert authoring output: it cannot install, register, or execute anything.",
        _ => "",
    }
}

fn extract_output_text(response: &Value) -> Result<String, CloudError> {
    if let Some(text) = response.get("output_text").and_then(Value::as_str) {
        return Ok(text.to_owned());
    }
    response
        .get("output")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                item.get("content")
                    .and_then(Value::as_array)
                    .and_then(|content| {
                        content.iter().find_map(|part| {
                            (part.get("type").and_then(Value::as_str) == Some("output_text"))
                                .then(|| part.get("text").and_then(Value::as_str))
                                .flatten()
                        })
                    })
            })
        })
        .map(str::to_owned)
        .ok_or(CloudError::InvalidModelOutput)
}

fn validate_base_url(value: &str, kind: DirectProviderKind) -> Result<Url, CloudError> {
    let mut url = Url::parse(value).map_err(|_| CloudError::InvalidSupportOrigin)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(CloudError::InvalidSupportOrigin);
    }
    let host = url.host_str().ok_or(CloudError::InvalidSupportOrigin)?;
    match kind {
        DirectProviderKind::OpenAi if host != "api.openai.com" => {
            return Err(CloudError::InvalidSupportOrigin);
        }
        DirectProviderKind::AzureOpenAi
            if !host.ends_with(".openai.azure.com")
                && !host.ends_with(".cognitiveservices.azure.com") =>
        {
            return Err(CloudError::InvalidSupportOrigin);
        }
        _ => {}
    }
    match kind {
        DirectProviderKind::OpenAi if !matches!(url.path(), "/v1" | "/v1/") => {
            return Err(CloudError::InvalidSupportOrigin);
        }
        DirectProviderKind::AzureOpenAi
            if !matches!(url.path(), "/" | "/openai/v1" | "/openai/v1/") =>
        {
            return Err(CloudError::InvalidSupportOrigin);
        }
        _ => {}
    }
    url.set_path(match kind {
        DirectProviderKind::OpenAi => "/v1/",
        DirectProviderKind::AzureOpenAi => "/openai/v1/",
    });
    Ok(url)
}

fn optional_environment(name: &str) -> Result<Option<String>, CloudError> {
    match env::var(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(CloudError::InvalidSupportOrigin),
    }
}

fn valid_resource_name(value: &str) -> bool {
    (1..=63).contains(&value.len())
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

fn valid_identifier(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':' | '/')
        })
}

fn now_millis() -> Result<UnixMillis, CloudError> {
    let millis = OffsetDateTime::now_utc()
        .unix_timestamp_nanos()
        .checked_div(1_000_000)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(CloudError::TransportFailed)?;
    Ok(UnixMillis(millis))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "fixture construction failures should stop these unit tests at their source"
    )]

    use std::collections::BTreeSet;

    use desktop_egress::{
        ApproveDecisionInput, ConsentService, ConsumeDecisionInput, ContextCandidate,
        ContextClassification, ContextPreparer, EgressLimits, MemoryDecisionLedger,
        ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput, RetentionMode,
    };
    use desktop_runtime::{
        sha256_bytes, BmadCandidateChange, BmadCapabilityOutput, BmadGovernedChangeSet, ContractId,
        RelativeWorkspacePath, UnixMillis,
    };
    use serde_json::json;

    use super::{
        extract_output_text, model_instructions, parse_untrusted_json, responses_request,
        validate_base_url, AuthorizedModelRequest, CloudError, DirectProviderConfig,
        DirectProviderKind,
    };

    fn id(value: &str) -> ContractId {
        ContractId::new(value).expect("valid fixture identifier")
    }

    fn authorized_change_fixture(content: &str) -> AuthorizedModelRequest {
        let manifest = ContextPreparer::new(PatternSecretScanner)
            .prepare(PrepareContextInput {
                tenant_ref: id("tenant_ref"),
                project_ref: id("project_ref"),
                run_ref: id("run_ref"),
                purpose: "planning".to_owned(),
                model_role: "planner".to_owned(),
                canonical_output_schema_id: id("sapphirus.bmad-governed-change-set.v1"),
                canonical_output_schema_hash: sha256_bytes(b"schema"),
                provider_profile_hash: sha256_bytes(b"provider-profile"),
                model_profile_hash: sha256_bytes(b"model-profile"),
                deployment_hash: sha256_bytes(b"deployment"),
                policy_hash: sha256_bytes(b"policy"),
                region: "westeurope".to_owned(),
                retention_mode: RetentionMode::TransientNoStore,
                created_at: UnixMillis(1_000),
                expires_at: UnixMillis(61_000),
                limits: EgressLimits {
                    maximum_context_items: 8,
                    maximum_context_bytes: 64 * 1024,
                    maximum_token_estimate: 16_000,
                },
                candidates: vec![ContextCandidate {
                    client_item_id: id("item_story"),
                    relative_label: RelativeWorkspacePath::new("story.md").expect("fixture path"),
                    semantic_role: "source".to_owned(),
                    language: Some("markdown".to_owned()),
                    classification: ContextClassification::Internal,
                    content: content.to_owned(),
                }],
                exclusions: Vec::new(),
            })
            .expect("manifest");
        let binding = ModelInvocationBindingDraft {
            schema_version: "sapphirus.model-invocation-binding.v1".to_owned(),
            request_id: id("request_001"),
            tenant_ref: manifest.draft.tenant_ref.clone(),
            project_ref: manifest.draft.project_ref.clone(),
            run_ref: manifest.draft.run_ref.clone(),
            installation_id: id("installation_001"),
            session_authority_hash: sha256_bytes(b"session-authority"),
            manifest_hash: manifest.manifest_hash,
            purpose: manifest.draft.purpose.clone(),
            model_role: manifest.draft.model_role.clone(),
            canonical_output_schema_id: manifest.draft.canonical_output_schema_id.clone(),
            canonical_output_schema_hash: manifest.draft.canonical_output_schema_hash,
            provider_profile_hash: manifest.draft.provider_profile_hash,
            model_profile_hash: manifest.draft.model_profile_hash,
            deployment_hash: manifest.draft.deployment_hash,
            policy_hash: manifest.draft.policy_hash,
            region: manifest.draft.region.clone(),
            retention_mode: manifest.draft.retention_mode,
            consent_disclosure_hash: sha256_bytes(b"consent-disclosure-v1"),
        }
        .seal()
        .expect("binding");
        let ledger = MemoryDecisionLedger::default();
        let service = ConsentService::new(&ledger);
        let decision = service
            .approve(ApproveDecisionInput {
                manifest: &manifest,
                binding: &binding,
                decision_id: id("decision_001"),
                issued_at: UnixMillis(1_500),
                expires_at: UnixMillis(31_500),
            })
            .expect("decision");
        let consumption = service
            .consume(ConsumeDecisionInput {
                decision: &decision,
                binding: &binding,
                invocation_id: id("invocation_001"),
                consumed_at: UnixMillis(2_000),
            })
            .expect("consumption");
        AuthorizedModelRequest::new(&manifest, &binding, consumption).expect("authorized request")
    }

    #[test]
    fn runtime_credentials_build_redacted_reconfigurable_provider_settings(
    ) -> Result<(), CloudError> {
        let runtime_credential = ["sk", "test", "runtime", "only"].join("-");
        let openai = DirectProviderConfig::from_runtime_credential(
            DirectProviderKind::OpenAi,
            "gpt-5-mini",
            runtime_credential.clone(),
            None,
            None,
        )?;
        assert_eq!(openai.kind(), DirectProviderKind::OpenAi);
        assert_eq!(openai.model(), "gpt-5-mini");
        assert!(!format!("{openai:?}").contains(&runtime_credential));

        let changed = openai.with_model("gpt-5.1-mini")?;
        assert_eq!(changed.model(), "gpt-5.1-mini");
        assert!(!format!("{changed:?}").contains(&runtime_credential));

        let azure = DirectProviderConfig::from_runtime_credential(
            DirectProviderKind::AzureOpenAi,
            "forge-gpt-5",
            ["azure", "test", "runtime", "only"].join("-"),
            Some("https://forge-models.openai.azure.com"),
            Some("westeurope"),
        )?;
        assert_eq!(azure.kind(), DirectProviderKind::AzureOpenAi);
        assert_eq!(azure.region(), "westeurope");
        Ok(())
    }

    #[test]
    fn provider_urls_are_closed_to_the_expected_openai_hosts() -> Result<(), CloudError> {
        let openai = validate_base_url("https://api.openai.com/v1", DirectProviderKind::OpenAi)?;
        assert_eq!(openai.as_str(), "https://api.openai.com/v1/");

        let azure = validate_base_url(
            "https://forge-models.openai.azure.com",
            DirectProviderKind::AzureOpenAi,
        )?;
        assert_eq!(
            azure.as_str(),
            "https://forge-models.openai.azure.com/openai/v1/"
        );

        assert!(validate_base_url("https://example.com/v1", DirectProviderKind::OpenAi).is_err());
        Ok(())
    }

    #[test]
    fn responses_output_parser_accepts_only_output_text() -> Result<(), CloudError> {
        let response = json!({
            "output": [{
                "type": "message",
                "content": [{
                    "type": "output_text",
                    "text": "{\"proposalKind\":\"no_recommendation\",\"reasonCode\":\"catalog_evidence_absent\"}"
                }]
            }]
        });
        assert!(extract_output_text(&response)?.contains("no_recommendation"));
        assert!(extract_output_text(&json!({ "output": [] })).is_err());
        Ok(())
    }

    #[test]
    fn direct_provider_rejects_duplicate_decoded_keys() {
        assert!(parse_untrusted_json(br#"{"status":"completed","status":"completed"}"#).is_err());
    }

    #[test]
    fn document_capability_instructions_match_the_canonical_closed_shape() {
        let document = model_instructions("sapphirus.bmad-document-artifact.v1");
        for required in [
            "resultKind=document_artifact",
            "documentArtifact",
            "schemaVersion=sapphirus.bmad-document-artifact.v1",
            "title",
            "sections",
            "heading",
            "body",
            "evidenceRefs",
            "openQuestions",
            "mermaidText",
            "contain exactly",
        ] {
            assert!(
                document.contains(required),
                "document instructions omitted canonical contract fragment: {required}"
            );
        }
        assert_eq!(
            document.matches("contain exactly").count(),
            3,
            "document instructions must close the root, artifact, and section shapes"
        );
        for incompatible in ["artifactKind", "summary", "evidence", "sourceRefs"] {
            assert!(
                !document
                    .split(|character: char| !character.is_ascii_alphanumeric())
                    .any(|token| token == incompatible),
                "document instructions retained incompatible field: {incompatible}"
            );
        }
    }

    #[test]
    fn governed_change_instructions_keep_the_canonical_closed_shape() {
        let changes = model_instructions("sapphirus.bmad-governed-change-set.v1");
        for required in [
            "resultKind=governed_change_set",
            "governedChangeSet",
            "schemaVersion=sapphirus.bmad-governed-change-set.v1",
            "summary",
            "changes",
            "operation=create",
            "operation=replace",
            "operation=delete",
            "path",
            "content",
            "preimageSha256",
            "contain exactly",
        ] {
            assert!(
                changes.contains(required),
                "governed-change instructions omitted canonical contract fragment: {required}"
            );
        }
        for legacy_create_only in [
            "Every change must use operation=create",
            "Never emit modify, delete",
        ] {
            assert!(
                !changes.contains(legacy_create_only),
                "governed-change instructions retained create-only restriction: {legacy_create_only}"
            );
        }
    }

    #[test]
    fn governed_change_variant_fixtures_match_the_instruction_contract() {
        let preimage = sha256_bytes(b"# Existing\n");
        let output = BmadCapabilityOutput::GovernedChangeSet(
            BmadGovernedChangeSet::new(
                "Implement the reviewed story.".to_owned(),
                vec![
                    BmadCandidateChange::Create {
                        path: RelativeWorkspacePath::new("src/new.rs").expect("create path"),
                        content: "fn created() {}\n".to_owned(),
                    },
                    BmadCandidateChange::Replace {
                        path: RelativeWorkspacePath::new("src/existing.rs").expect("replace path"),
                        content: "fn replaced() {}\n".to_owned(),
                        preimage_sha256: preimage,
                    },
                    BmadCandidateChange::Delete {
                        path: RelativeWorkspacePath::new("src/obsolete.rs").expect("delete path"),
                        preimage_sha256: preimage,
                    },
                ],
            )
            .expect("canonical governed change set"),
        );
        let value = serde_json::to_value(output).expect("canonical result JSON");
        let changes = value["governedChangeSet"]["changes"]
            .as_array()
            .expect("canonical changes");
        let instructions = model_instructions("sapphirus.bmad-governed-change-set.v1");

        for (change, expected_keys) in changes.iter().zip([
            ["operation", "path", "content"].as_slice(),
            ["operation", "path", "content", "preimageSha256"].as_slice(),
            ["operation", "path", "preimageSha256"].as_slice(),
        ]) {
            let object = change.as_object().expect("canonical change object");
            assert_eq!(
                object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                expected_keys.iter().copied().collect::<BTreeSet<_>>()
            );
            let operation = object["operation"].as_str().expect("operation");
            assert!(instructions.contains(&format!("operation={operation}")));
            for key in expected_keys {
                assert!(
                    instructions.contains(key),
                    "instructions omitted {key} for {operation}"
                );
            }
        }
    }

    #[test]
    fn reviewed_context_header_carries_the_exact_outbound_content_hash() {
        let content = "# Existing\n";
        let request = authorized_change_fixture(content);
        let body = responses_request(&request, "gpt-5-mini").expect("Responses request");
        let input = body["input"][0]["content"][0]["text"]
            .as_str()
            .expect("input text");

        assert!(input.contains("story.md"));
        assert!(input.contains("source"));
        assert!(input.contains(&format!(
            "reviewedContentHash={}",
            sha256_bytes(content.as_bytes())
        )));
    }

    #[test]
    fn builder_capability_instructions_match_the_canonical_closed_shape() {
        let builder = model_instructions("sapphirus.bmad-inactive-builder-draft.v1");
        for required in [
            "resultKind=inactive_builder_draft",
            "inactiveBuilderDraft",
            "schemaVersion=sapphirus.bmad-inactive-builder-draft.v1",
            "draftKind",
            "title",
            "revisionNote",
            "files",
            "path",
            "content",
            "contain exactly",
            "cannot install, register, or execute",
        ] {
            assert!(
                builder.contains(required),
                "Builder instructions omitted canonical contract fragment: {required}"
            );
        }
        assert_eq!(
            builder.matches("contain exactly").count(),
            3,
            "Builder instructions must close the root, draft, and file shapes"
        );
        for incompatible in ["name", "summary"] {
            assert!(
                !builder
                    .split(|character: char| !character.is_ascii_alphanumeric())
                    .any(|token| token == incompatible),
                "Builder instructions retained incompatible field: {incompatible}"
            );
        }
    }
}
