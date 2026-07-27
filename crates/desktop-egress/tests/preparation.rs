#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use desktop_egress::{
    reintroduced_redaction_marker, ContextCandidate, ContextClassification, ContextPreparer,
    EgressError, EgressLimits, PatternSecretScanner, PrepareContextInput, RetentionMode,
    SecretScanner, REDACTION_MARKERS,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, UnixMillis};

/// The four PEM markers the scanner knows, split so this file never carries a
/// whole one as a literal.
const BEGIN_PLAIN: &str = concat!("-----BEGIN ", "PRIVATE KEY-----");
const END_PLAIN: &str = concat!("-----END ", "PRIVATE KEY-----");
const BEGIN_RSA: &str = concat!("-----BEGIN ", "RSA PRIVATE KEY-----");
const END_RSA: &str = concat!("-----END ", "RSA PRIVATE KEY-----");
const BEGIN_OPENSSH: &str = concat!("-----BEGIN ", "OPENSSH PRIVATE KEY-----");
const END_OPENSSH: &str = concat!("-----END ", "OPENSSH PRIVATE KEY-----");

/// Three wrapped base64 lines: the shape of a real key body, and far past the
/// forty-character floor the scanner requires before it believes a marker.
const PEM_BODY: &str = "MIIEowIBAAKCAQEAxJm5hQkTGpKmYbUvWnR4cXpLdEZoTmJHc1F3ZXJ0eXVpb3Ax\n\
Mjc4OTBhc2RmZ2hqa2x6eGN2Ym5tUVdFUlRZVUlPUEFTREZHSEpLTFpYQ1ZCTk0r\n\
L3d4WXpBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZWjAxMjM0NTY3ODkrLz09";

fn candidate(label: &str, content: &str) -> ContextCandidate {
    ContextCandidate {
        client_item_id: ContractId::new(format!("item_{}", label.replace(['/', '.'], "_")))
            .expect("fixture item id"),
        relative_label: RelativeWorkspacePath::new(label).expect("fixture label"),
        semantic_role: "source".to_owned(),
        language: Some("text".to_owned()),
        classification: ContextClassification::Internal,
        content: content.to_owned(),
    }
}

fn fixture_input(candidates: Vec<ContextCandidate>) -> PrepareContextInput {
    PrepareContextInput {
        tenant_ref: ContractId::new("tenant_ref").expect("tenant ref"),
        project_ref: ContractId::new("project_ref").expect("project ref"),
        run_ref: ContractId::new("run_ref").expect("run ref"),
        purpose: "planning".to_owned(),
        model_role: "planner".to_owned(),
        canonical_output_schema_id: ContractId::new("planning_output_v1").expect("schema id"),
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
        candidates,
        exclusions: Vec::new(),
    }
}

#[test]
fn preparation_preserves_safe_bytes_and_binds_original_and_outbound_hashes() {
    let content = "safe context\n";
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("notes.txt", content)]))
        .expect("safe manifest");

    let item = &manifest.draft.items[0];
    assert_eq!(item.outbound_content, content);
    assert_eq!(item.original_content_hash, sha256_bytes(content.as_bytes()));
    assert_eq!(item.outbound_content_hash, sha256_bytes(content.as_bytes()));
    assert!(manifest.draft.secret_findings.is_empty());
}

#[test]
fn preparation_rejects_dotenv_before_scanning() {
    let input = fixture_input(vec![candidate(".env", concat!("API_KEY=", "secret"))]);

    let error = ContextPreparer::new(PatternSecretScanner)
        .prepare(input)
        .expect_err("dotenv is denied");

    assert_eq!(error, EgressError::DeniedContextLabel);
}

#[test]
fn preparation_rejects_nested_secret_bearing_filenames_case_insensitively() {
    let input = fixture_input(vec![candidate("config/CREDENTIALS", "secret")]);

    let error = ContextPreparer::new(PatternSecretScanner)
        .prepare(input)
        .expect_err("credential file is denied");

    assert_eq!(error, EgressError::DeniedContextLabel);
}

#[test]
fn preparation_denies_credential_stores_token_caches_and_authority_state() {
    for label in [
        ".git-credentials",
        ".netrc",
        ".pypirc",
        ".docker/config.json",
        ".azure/msal_token_cache.json",
        ".sapphirus/authority.db",
        ".kube/config",
        ".gnupg/private-keys-v1.d/key.key",
        "tls/server.key",
        "certs/client.pem",
        "keys/id_ecdsa",
        "keys/id_dsa",
        "keys/id_ed25519_sk",
        "certs/client.p12",
        "certs/client.pfx",
        "config/token-cache.bin",
    ] {
        let error = ContextPreparer::new(PatternSecretScanner)
            .prepare(fixture_input(vec![candidate(
                label,
                "innocent-looking bytes",
            )]))
            .expect_err("authority-bearing label is denied before scanning");
        assert_eq!(error, EgressError::DeniedContextLabel, "accepted {label}");
    }
}

#[test]
fn preparation_redacts_private_key_material_and_records_a_finding() {
    // The fixture this test used to carry was `prefix <marker> value`: a bare
    // marker with no key under it. It proved only that the scanner reacted to
    // the marker text, which is public, and it is the exact shape an ops
    // runbook produces. A real block is what has to be redacted.
    let source = format!("Attached key:\n\n{BEGIN_PLAIN}\n{PEM_BODY}\n{END_PLAIN}\n\nRotate it.\n");
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("notes.txt", &source)]))
        .expect("redacted manifest");

    let item = &manifest.draft.items[0];
    assert_eq!(
        item.outbound_content, "Attached key:\n\n[REDACTED:private_key]\n\nRotate it.\n",
        "the block, and only the block, must be replaced"
    );
    assert_eq!(item.redactions[0].kind, "private_key");
    assert_eq!(item.redactions[0].occurrence_count, 1);
    assert_eq!(manifest.draft.secret_findings[0].kind, "private_key");
    assert_eq!(manifest.draft.secret_findings[0].occurrence_count, 1);
}

#[test]
fn preparation_bounds_an_unterminated_private_key_block_to_its_body() {
    // No END marker: the file was truncated, or the paste was partial. The
    // predecessor redacted from the marker to the end of the input, so the
    // sentence after the key — and every other byte of the document — was
    // deleted before the model saw it.
    let source = format!(
        "Incident 4412:\n\n{BEGIN_RSA}\n{PEM_BODY}\nRotate the key and close the ticket.\n"
    );
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("incident.md", &source)]))
        .expect("redacted manifest");

    let item = &manifest.draft.items[0];
    assert_eq!(
        item.outbound_content,
        "Incident 4412:\n\n[REDACTED:private_key]\nRotate the key and close the ticket.\n",
        "an unterminated block must cost the body, not the document"
    );
    for line in PEM_BODY.lines() {
        assert!(
            !item.outbound_content.contains(line),
            "key body line survived: {line}"
        );
    }
    assert_eq!(item.redactions[0].occurrence_count, 1);
}

#[test]
fn preparation_redacts_a_json_embedded_private_key_whose_newlines_are_escapes() {
    // A service-account JSON holds the whole key on one physical line with
    // two-character `\n` escapes. Any rule that demanded the marker start a
    // line would skip this entirely, which is why the scanner gates on body
    // shape instead.
    let escaped = PEM_BODY.replace('\n', "\\n");
    let source = format!("{{\"private_key\":\"{BEGIN_PLAIN}\\n{escaped}\\n{END_PLAIN}\\n\"}}\n");
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("service.json", &source)]))
        .expect("redacted manifest");

    let item = &manifest.draft.items[0];
    assert_eq!(
        item.outbound_content, "{\"private_key\":\"[REDACTED:private_key]\\n\"}\n",
        "the escaped block must be replaced without eating the JSON around it"
    );
    assert_eq!(item.redactions[0].occurrence_count, 1);
}

#[test]
fn preparation_redacts_an_indented_block_and_one_carrying_pem_headers() {
    // Left: a YAML literal block, indented two spaces. Right: an encrypted key,
    // whose RFC 1421 headers and blank separator sit between the marker and the
    // body. Both are real shapes and both must still be caught.
    let indented = PEM_BODY
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!(
        "key: |\n  {BEGIN_OPENSSH}\n{indented}\n  {END_OPENSSH}\ndone: true\n\n\
{BEGIN_RSA}\nProc-Type: 4,ENCRYPTED\nDEK-Info: AES-128-CBC,1F2A3B4C5D6E7F8091A2B3C4D5E6F708\n\n\
{PEM_BODY}\n{END_RSA}\n"
    );
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("deploy.yaml", &source)]))
        .expect("redacted manifest");

    let item = &manifest.draft.items[0];
    assert_eq!(
        item.outbound_content,
        "key: |\n  [REDACTED:private_key]\ndone: true\n\n[REDACTED:private_key]\n",
        "indented and header-bearing blocks must both be replaced"
    );
    assert_eq!(item.redactions[0].occurrence_count, 2);
}

#[test]
fn preparation_leaves_a_runbook_that_only_talks_about_key_markers_byte_identical() {
    // Three ways an operator writes about a marker without pasting a key:
    // inline in a sentence, alone on its own line as a heading, and as a
    // begin/end pair quoted in one breath. None of them is key material and
    // none of them may cost the reader a byte — nor raise a finding, because a
    // reviewer who learns that the secret banner lies stops reading it.
    let source = format!(
        "Runbook 7: if the export log shows {BEGIN_RSA} the operator must stop the export.\n\n\
Expected header:\n\n{BEGIN_PLAIN}\n\nNever paste the body into a ticket.\n\n\
The block runs from {BEGIN_OPENSSH} to {END_OPENSSH} and is base64 in between.\n"
    );
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("runbook.md", &source)]))
        .expect("runbook manifest");

    let item = &manifest.draft.items[0];
    assert_eq!(item.outbound_content, source, "the runbook was rewritten");
    assert_eq!(item.outbound_content_hash, item.original_content_hash);
    assert!(item.redactions.is_empty(), "{:?}", item.redactions);
    assert!(
        manifest.draft.secret_findings.is_empty(),
        "{:?}",
        manifest.draft.secret_findings
    );
}

#[test]
fn preparation_passes_a_near_miss_prose_corpus_through_byte_identical() {
    // Every line here carries a token that a boundary-free or extent-free
    // matcher fires on. Byte equality is the assertion, not `contains`: a
    // scanner that deletes the tail of a document still satisfies `contains`
    // for the head of it.
    let corpus = format!(
        "The risk-assessment and the task-list both feed the disk-encryption review.\n\
Assign the ghost-writer to the desk-checking notes and the brisk-turnaround plan.\n\
Label MAKIAVELLIAN-7 and AKIABURST-9 are tracked in the ask-response-log.\n\
Scoring uses high-ghp_relevance-weighting, not the risk-register alone.\n\
Runbook: grep the archive for {BEGIN_RSA} before you approve the export.\n\
Fields named password= and secret= are empty in this template.\n\
The whisk-timer, brusk-tone, and mollusk-shell entries are unrelated.\n\
Pipeline uses sk-learn-classifier-pipeline and sk-hynix-memory-modules.\n"
    );
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("prd.md", &corpus)]))
        .expect("prose manifest");

    let item = &manifest.draft.items[0];
    assert_eq!(
        item.outbound_content, corpus,
        "ordinary prose was rewritten"
    );
    assert_eq!(item.outbound_content_hash, item.original_content_hash);
    assert_eq!(
        item.outbound_byte_count, item.original_byte_count,
        "a byte was added or removed"
    );
    assert!(item.redactions.is_empty(), "{:?}", item.redactions);
    assert!(
        manifest.draft.secret_findings.is_empty(),
        "{:?}",
        manifest.draft.secret_findings
    );
}

#[test]
fn preparation_passes_a_true_positive_corpus_and_keeps_every_secret_out() {
    // The other direction. Fixing false positives is only worth anything if
    // the real shapes still go. Each entry is a distinct detection path.
    let openai = concat!("sk-", "proj-abcdefghijklmnopqrstuvwxyz0123456789");
    let github = concat!("ghp_", "abcdefghijklmnopqrstuvwxyz0123456789");
    let assigned = "hunter2-correct-horse-battery";
    let escaped = PEM_BODY.replace('\n', "\\n");
    let corpus = format!(
        "{BEGIN_PLAIN}\n{PEM_BODY}\n{END_PLAIN}\n\
{BEGIN_RSA}\n{PEM_BODY}\n\
{{\"private_key\":\"{BEGIN_OPENSSH}\\n{escaped}\\n{END_OPENSSH}\"}}\n\
export OPENAI_API_KEY={openai}\n\
\"{github}\"\n\
DB_PASSWORD={assigned}\n"
    );
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("leaked.txt", &corpus)]))
        .expect("true positive manifest");

    let item = &manifest.draft.items[0];
    for secret in [openai, github, assigned] {
        assert!(
            !item.outbound_content.contains(secret),
            "secret survived: {secret}"
        );
    }
    for line in PEM_BODY.lines() {
        assert!(
            !item.outbound_content.contains(line),
            "key body line survived: {line}"
        );
    }
    let serialized = serde_json::to_string(&manifest.review_projection()).expect("projection json");
    for secret in [openai, github, assigned] {
        assert!(
            !serialized.contains(secret),
            "secret reached review: {secret}"
        );
    }
    let mut kinds: Vec<&str> = item
        .redactions
        .iter()
        .map(|redaction| redaction.kind.as_str())
        .collect();
    kinds.sort_unstable();
    assert_eq!(
        kinds,
        ["credential", "github_token", "openai_token", "private_key"],
        "a detection path went silent"
    );
    let private_key = item
        .redactions
        .iter()
        .find(|redaction| redaction.kind == "private_key")
        .expect("private key finding");
    assert_eq!(
        private_key.occurrence_count, 3,
        "terminated, unterminated, and escaped blocks are three findings"
    );
}

#[test]
fn redaction_markers_cover_every_kind_the_scanner_emits() {
    // `reintroduced_redaction_marker` is only as good as this list. If a new
    // redaction kind is added and the constant is not, the write-back guard
    // silently stops covering it — so derive the truth from the scanner.
    let escaped = PEM_BODY.replace('\n', "\\n");
    let corpus = format!(
        "{BEGIN_PLAIN}\n{PEM_BODY}\n{END_PLAIN}\n\
{{\"k\":\"{BEGIN_OPENSSH}\\n{escaped}\\n{END_OPENSSH}\"}}\n\
{}\n\"{}\"\nAPI_KEY=value\naccess_token=value\npassword=value\nsecret=value\n",
        concat!("sk-", "proj-abcdefghijklmnopqrstuvwxyz0123456789"),
        concat!("ghp_", "abcdefghijklmnopqrstuvwxyz0123456789"),
    );
    let scanned = PatternSecretScanner.scan(&corpus).outbound_content;

    let mut emitted = Vec::new();
    let mut rest = scanned.as_str();
    while let Some(open) = rest.find("[REDACTED:") {
        let tail = &rest[open..];
        let close = tail.find(']').expect("an unterminated marker is a bug");
        emitted.push(tail[..=close].to_owned());
        rest = &tail[close + 1..];
    }
    assert!(!emitted.is_empty(), "the corpus redacted nothing");
    for marker in &emitted {
        assert!(
            REDACTION_MARKERS.contains(&marker.as_str()),
            "scanner emits {marker}, which REDACTION_MARKERS does not list"
        );
    }
    for marker in REDACTION_MARKERS {
        assert!(
            emitted.iter().any(|seen| seen == marker),
            "REDACTION_MARKERS lists {marker}, which no corpus entry produces"
        );
    }
}

#[test]
fn a_body_that_adds_a_redaction_marker_is_refused_and_one_that_inherits_it_is_not() {
    let preimage = "token: ghp_realvalue\nnote: keep\n";
    let echoed = "token: [REDACTED:github_token]\nnote: keep\nnote2: added\n";
    assert_eq!(
        reintroduced_redaction_marker(preimage, echoed),
        Some("[REDACTED:github_token]"),
        "an echoed redaction marker must be refusable"
    );

    // The file already documents the marker. Editing it elsewhere is ordinary
    // work and must not be refused, or the guard becomes the next false alarm.
    let documented = "The scanner writes [REDACTED:credential] in place of the value.\n";
    let edited = "The scanner writes [REDACTED:credential] in place of the secret value.\n";
    assert_eq!(reintroduced_redaction_marker(documented, edited), None);

    // Adding a second copy to a file that already had one is still net-new.
    let doubled = "[REDACTED:credential] and [REDACTED:credential]\n";
    assert_eq!(
        reintroduced_redaction_marker(documented, doubled),
        Some("[REDACTED:credential]")
    );

    // A body that touches no marker at all is always fine.
    assert_eq!(reintroduced_redaction_marker("a\n", "b\n"), None);
}

#[test]
fn preparation_redacts_prefixed_tokens_without_retaining_the_secret() {
    let github_token = concat!("ghp_", "abcdefghijklmnopqrstuvwxyz0123456789");
    let source = format!("token={github_token}");
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("notes.txt", &source)]))
        .expect("redacted manifest");

    let serialized = serde_json::to_string(&manifest.review_projection()).expect("projection json");
    assert!(!serialized.contains(github_token));
    assert!(serialized.contains("github_token"));
}

#[test]
fn preparation_leaves_ordinary_prose_that_merely_contains_a_prefix_untouched() {
    // Every one of these carries `sk-` or `ghp_` inside a longer word. A
    // scanner that matches without a left word boundary deletes the subject
    // of the sentence and reports a false secret finding on the consent
    // surface, which is worse than the redaction itself.
    let source = "risk-assessment, task-management, disk-encryption, risk-register, \
desk-checking, and the ask-response-log for high-ghp_relevance-scoring.";
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("prd.md", source)]))
        .expect("prose manifest");

    let item = &manifest.draft.items[0];
    assert_eq!(
        item.outbound_content, source,
        "ordinary prose was rewritten"
    );
    assert_eq!(item.outbound_content_hash, item.original_content_hash);
    assert!(item.redactions.is_empty(), "{:?}", item.redactions);
    assert!(
        manifest.draft.secret_findings.is_empty(),
        "{:?}",
        manifest.draft.secret_findings
    );
}

#[test]
fn preparation_still_redacts_real_prefixed_tokens_at_every_word_boundary() {
    let openai = concat!("sk-", "proj-abcdefghijklmnopqrstuvwxyz0123456789");
    let github = concat!("ghp_", "abcdefghijklmnopqrstuvwxyz0123456789");
    // Start of input, after an assignment, after a quote, and after a slash:
    // the four ways a real credential actually shows up in a file.
    let source =
        format!("{openai} OPENAI_KEY={openai}\n\"{github}\" https://x:{github}@example.invalid\n");
    let manifest = ContextPreparer::new(PatternSecretScanner)
        .prepare(fixture_input(vec![candidate("notes.txt", &source)]))
        .expect("redacted manifest");

    let item = &manifest.draft.items[0];
    assert!(!item.outbound_content.contains(openai));
    assert!(!item.outbound_content.contains(github));
    let serialized = serde_json::to_string(&manifest.review_projection()).expect("projection json");
    assert!(!serialized.contains(openai));
    assert!(!serialized.contains(github));
    let openai_finding = item
        .redactions
        .iter()
        .find(|redaction| redaction.kind == "openai_token")
        .expect("openai finding");
    assert_eq!(openai_finding.occurrence_count, 2);
    let github_finding = item
        .redactions
        .iter()
        .find(|redaction| redaction.kind == "github_token")
        .expect("github finding");
    assert_eq!(github_finding.occurrence_count, 2);
}

#[test]
fn preparation_enforces_the_outbound_byte_budget_after_redaction() {
    let mut input = fixture_input(vec![candidate("notes.txt", "12345")]);
    input.limits.maximum_context_bytes = 4;

    let result = ContextPreparer::new(PatternSecretScanner).prepare(input);

    assert_eq!(result, Err(EgressError::ContextBudgetExceeded));
}
