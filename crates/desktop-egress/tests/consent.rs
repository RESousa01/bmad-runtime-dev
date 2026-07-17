#![allow(
    clippy::expect_used,
    reason = "invalid static fixtures should fail the test at construction"
)]

use desktop_egress::{
    ApproveDecisionInput, CancelDecisionInput, ConsentService, ConsumeDecisionInput,
    ContextCandidate, ContextClassification, ContextEgressManifest, ContextPreparer,
    DecisionEvidenceInput, EgressError, EgressLimits, MemoryDecisionLedger,
    ModelInvocationBindingDraft, PatternSecretScanner, PrepareContextInput, RetentionMode,
};
use desktop_runtime::{sha256_bytes, ContractId, RelativeWorkspacePath, UnixMillis};
use std::sync::{Arc, Barrier};

fn id(value: &str) -> ContractId {
    ContractId::new(value).expect("valid fixture identifier")
}

fn fixture_manifest() -> ContextEgressManifest {
    ContextPreparer::new(PatternSecretScanner)
        .prepare(PrepareContextInput {
            tenant_ref: id("tenant_ref"),
            project_ref: id("project_ref"),
            run_ref: id("run_ref"),
            purpose: "planning".to_owned(),
            model_role: "planner".to_owned(),
            canonical_output_schema_id: id("planning_output_v1"),
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
                client_item_id: id("item_notes"),
                relative_label: RelativeWorkspacePath::new("notes.txt").expect("fixture path"),
                semantic_role: "source".to_owned(),
                language: Some("text".to_owned()),
                classification: ContextClassification::Internal,
                content: "safe context".to_owned(),
            }],
            exclusions: Vec::new(),
        })
        .expect("fixture manifest")
}

fn fixture_binding(manifest: &ContextEgressManifest) -> ModelInvocationBindingDraft {
    ModelInvocationBindingDraft {
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
}

fn fixture_approval<'a>(
    manifest: &'a ContextEgressManifest,
    binding: &'a desktop_egress::ModelInvocationBinding,
) -> ApproveDecisionInput<'a> {
    ApproveDecisionInput {
        manifest,
        binding,
        decision_id: id("decision_001"),
        issued_at: UnixMillis(1_500),
        expires_at: UnixMillis(31_500),
    }
}

fn fixture_consumption<'a>(
    decision: &'a desktop_egress::PendingContextDecision,
    binding: &'a desktop_egress::ModelInvocationBinding,
) -> ConsumeDecisionInput<'a> {
    ConsumeDecisionInput {
        decision,
        binding,
        invocation_id: id("invocation_001"),
        consumed_at: UnixMillis(2_000),
    }
}

#[test]
fn one_decision_authorizes_one_exact_invocation() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");

    let consumed = service
        .consume(fixture_consumption(&decision, &binding))
        .expect("consumption");

    assert_eq!(consumed.decision_id(), decision.decision_id());
    assert_eq!(consumed.binding_hash(), binding.binding_hash);
    assert_eq!(
        service.consume(fixture_consumption(&decision, &binding)),
        Err(EgressError::DecisionAlreadyConsumed)
    );
}

#[test]
fn byte_identical_retry_still_requires_a_new_decision() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");
    service
        .consume(fixture_consumption(&decision, &binding))
        .expect("first consumption");

    let mut retry = fixture_consumption(&decision, &binding);
    retry.invocation_id = id("invocation_retry");

    assert_eq!(
        service.consume(retry),
        Err(EgressError::DecisionAlreadyConsumed)
    );
}

#[test]
fn drifted_region_is_rejected_without_consuming_the_decision() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");
    let mut drifted = fixture_binding(&manifest);
    drifted.region = "westus".to_owned();
    let drifted = drifted.seal().expect("drifted binding shape");

    assert_eq!(
        service.consume(fixture_consumption(&decision, &drifted)),
        Err(EgressError::DecisionBindingMismatch)
    );
    assert!(service
        .consume(fixture_consumption(&decision, &binding))
        .is_ok());
}

#[test]
fn binding_that_does_not_match_the_manifest_cannot_be_approved() {
    let manifest = fixture_manifest();
    let mut draft = fixture_binding(&manifest);
    draft.purpose = "analysis".to_owned();
    let binding = draft.seal().expect("binding shape");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);

    assert_eq!(
        service.approve(fixture_approval(&manifest, &binding)),
        Err(EgressError::DecisionBindingMismatch)
    );
}

#[test]
fn expired_decision_is_terminal() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");
    let mut input = fixture_consumption(&decision, &binding);
    input.consumed_at = UnixMillis(decision.expires_at().0 + 1);

    assert_eq!(service.consume(input), Err(EgressError::DecisionExpired));
    assert_eq!(
        service.consume(fixture_consumption(&decision, &binding)),
        Err(EgressError::DecisionExpired)
    );
}

#[test]
fn duplicate_decision_identifier_is_rejected() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    service
        .approve(fixture_approval(&manifest, &binding))
        .expect("first decision");

    assert_eq!(
        service.approve(fixture_approval(&manifest, &binding)),
        Err(EgressError::DecisionAlreadyExists)
    );
}

#[test]
fn consumption_is_read_only_and_serializes_without_private_authority() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");
    let consumption = service
        .consume(fixture_consumption(&decision, &binding))
        .expect("consumption");
    let serialized = serde_json::to_value(&consumption).expect("serialized consumption");

    consumption.verify().expect("sealed consumption");
    assert_eq!(serialized["invocationId"], "invocation_001");
    assert_eq!(
        serialized["consumptionHash"],
        consumption.consumption_hash().to_string()
    );
    assert!(serialized.get("authority").is_none());
}

#[test]
fn cancelled_decision_is_terminal_and_cannot_authorize_egress() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");

    service
        .cancel(CancelDecisionInput {
            decision: &decision,
            cancelled_at: UnixMillis(2_000),
        })
        .expect("cancel pending decision");

    assert_eq!(
        service.consume(fixture_consumption(&decision, &binding)),
        Err(EgressError::DecisionCancelled)
    );
    assert_eq!(
        service.cancel(CancelDecisionInput {
            decision: &decision,
            cancelled_at: UnixMillis(2_100),
        }),
        Err(EgressError::DecisionCancelled)
    );
}

#[test]
fn cancellation_and_consumption_race_has_exactly_one_terminal_winner() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");
    let barrier = Arc::new(Barrier::new(2));
    let service = &service;
    let decision = &decision;
    let binding = &binding;

    std::thread::scope(|scope| {
        let consume_barrier = Arc::clone(&barrier);
        let consume = scope.spawn(move || {
            consume_barrier.wait();
            service.consume(fixture_consumption(decision, binding))
        });
        let cancel_barrier = Arc::clone(&barrier);
        let cancel = scope.spawn(move || {
            cancel_barrier.wait();
            service.cancel(CancelDecisionInput {
                decision,
                cancelled_at: UnixMillis(2_000),
            })
        });

        let consumed = consume.join().expect("consume thread");
        let cancelled = cancel.join().expect("cancel thread");
        assert_ne!(consumed.is_ok(), cancelled.is_ok());
        assert!(matches!(
            (consumed, cancelled),
            (Ok(_), Err(EgressError::DecisionAlreadyConsumed))
                | (Err(EgressError::DecisionCancelled), Ok(()))
        ));
    });
}

#[test]
fn pending_decision_cannot_be_cancelled_or_consumed_after_memory_ledger_restart() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let original_ledger = MemoryDecisionLedger::default();
    let decision = ConsentService::new(&original_ledger)
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");
    let restarted_ledger = MemoryDecisionLedger::default();
    let restarted = ConsentService::new(&restarted_ledger);

    assert_eq!(
        restarted.cancel(CancelDecisionInput {
            decision: &decision,
            cancelled_at: UnixMillis(2_000),
        }),
        Err(EgressError::DecisionUnknown)
    );
    assert_eq!(
        restarted.consume(fixture_consumption(&decision, &binding)),
        Err(EgressError::DecisionUnknown)
    );
}

#[test]
fn decision_ledger_is_bounded_and_prunes_expired_authority_without_replay() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let first = service
        .approve(ApproveDecisionInput {
            decision_id: id("decision_0000"),
            ..fixture_approval(&manifest, &binding)
        })
        .expect("first decision");
    service
        .consume(fixture_consumption(&first, &binding))
        .expect("first consumption");

    for index in 1..MemoryDecisionLedger::MAX_TRACKED_DECISIONS {
        service
            .approve(ApproveDecisionInput {
                decision_id: id(&format!("decision_{index:04}")),
                ..fixture_approval(&manifest, &binding)
            })
            .expect("decision within fixed ledger capacity");
    }

    assert_eq!(
        service.approve(ApproveDecisionInput {
            decision_id: id("decision_at_capacity"),
            ..fixture_approval(&manifest, &binding)
        }),
        Err(EgressError::DecisionAlreadyExists)
    );

    service
        .approve(ApproveDecisionInput {
            manifest: &manifest,
            binding: &binding,
            decision_id: id("decision_after_expiry"),
            issued_at: UnixMillis(31_501),
            expires_at: UnixMillis(60_000),
        })
        .expect("expired authority was pruned before bounded insertion");

    let mut replay = fixture_consumption(&first, &binding);
    replay.consumed_at = UnixMillis(32_000);
    assert_eq!(service.consume(replay), Err(EgressError::DecisionUnknown));
    assert_eq!(
        service.approve(ApproveDecisionInput {
            manifest: &manifest,
            binding: &binding,
            decision_id: id("decision_0000"),
            issued_at: UnixMillis(31_501),
            expires_at: UnixMillis(60_000),
        }),
        Err(EgressError::DecisionAlreadyExists)
    );
}

#[test]
fn pending_decision_exposes_only_sealed_bridge_evidence() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");
    let ledger = MemoryDecisionLedger::default();
    let service = ConsentService::new(&ledger);
    let decision = service
        .approve(fixture_approval(&manifest, &binding))
        .expect("decision");

    let evidence = service
        .evidence(DecisionEvidenceInput {
            decision: &decision,
            observed_at: UnixMillis(2_000),
        })
        .expect("sealed host evidence");

    assert_eq!(evidence.decision_id(), decision.decision_id());
    assert_eq!(evidence.manifest_hash(), manifest.manifest_hash);
    assert_eq!(evidence.invocation_binding_hash(), binding.binding_hash);
    assert_eq!(
        evidence.consent_disclosure_hash(),
        binding.draft.consent_disclosure_hash
    );
    assert_eq!(evidence.policy_hash(), binding.draft.policy_hash);
    assert_eq!(evidence.installation_id(), &binding.draft.installation_id);
    assert_eq!(
        evidence.session_authority_hash(),
        binding.draft.session_authority_hash
    );
    assert_eq!(evidence.issued_at(), UnixMillis(1_500));
    assert_eq!(evidence.expires_at(), UnixMillis(31_500));
    assert_eq!(evidence.observed_at(), UnixMillis(2_000));
    assert_eq!(format!("{evidence:?}"), "ContextDecisionEvidence { .. }");
}

#[test]
fn terminal_or_unregistered_decisions_cannot_expose_bridge_evidence() {
    let manifest = fixture_manifest();
    let binding = fixture_binding(&manifest).seal().expect("binding");

    let consumed_ledger = MemoryDecisionLedger::default();
    let consumed_service = ConsentService::new(&consumed_ledger);
    let consumed = consumed_service
        .approve(ApproveDecisionInput {
            decision_id: id("decision_consumed"),
            ..fixture_approval(&manifest, &binding)
        })
        .expect("consumed decision");
    consumed_service
        .consume(ConsumeDecisionInput {
            decision: &consumed,
            binding: &binding,
            invocation_id: id("invocation_consumed"),
            consumed_at: UnixMillis(2_000),
        })
        .expect("consume decision");
    assert!(matches!(
        consumed_service.evidence(DecisionEvidenceInput {
            decision: &consumed,
            observed_at: UnixMillis(2_100),
        }),
        Err(EgressError::DecisionAlreadyConsumed)
    ));

    let cancelled_ledger = MemoryDecisionLedger::default();
    let cancelled_service = ConsentService::new(&cancelled_ledger);
    let cancelled = cancelled_service
        .approve(ApproveDecisionInput {
            decision_id: id("decision_cancelled"),
            ..fixture_approval(&manifest, &binding)
        })
        .expect("cancelled decision");
    cancelled_service
        .cancel(CancelDecisionInput {
            decision: &cancelled,
            cancelled_at: UnixMillis(2_000),
        })
        .expect("cancel decision");
    assert!(matches!(
        cancelled_service.evidence(DecisionEvidenceInput {
            decision: &cancelled,
            observed_at: UnixMillis(2_100),
        }),
        Err(EgressError::DecisionCancelled)
    ));

    let expired_ledger = MemoryDecisionLedger::default();
    let expired_service = ConsentService::new(&expired_ledger);
    let expired = expired_service
        .approve(ApproveDecisionInput {
            decision_id: id("decision_expired"),
            ..fixture_approval(&manifest, &binding)
        })
        .expect("expired decision");
    assert!(matches!(
        expired_service.evidence(DecisionEvidenceInput {
            decision: &expired,
            observed_at: UnixMillis(31_501),
        }),
        Err(EgressError::DecisionExpired)
    ));

    let pruned_ledger = MemoryDecisionLedger::default();
    let pruned_service = ConsentService::new(&pruned_ledger);
    let pruned = pruned_service
        .approve(ApproveDecisionInput {
            decision_id: id("decision_pruned"),
            ..fixture_approval(&manifest, &binding)
        })
        .expect("decision before pruning");
    pruned_service
        .approve(ApproveDecisionInput {
            manifest: &manifest,
            binding: &binding,
            decision_id: id("decision_prune_trigger"),
            issued_at: UnixMillis(31_501),
            expires_at: UnixMillis(60_000),
        })
        .expect("prune trigger");
    assert!(matches!(
        pruned_service.evidence(DecisionEvidenceInput {
            decision: &pruned,
            observed_at: UnixMillis(32_000),
        }),
        Err(EgressError::DecisionUnknown)
    ));

    let restarted_ledger = MemoryDecisionLedger::default();
    let restarted_service = ConsentService::new(&restarted_ledger);
    assert!(matches!(
        restarted_service.evidence(DecisionEvidenceInput {
            decision: &pruned,
            observed_at: UnixMillis(2_000),
        }),
        Err(EgressError::DecisionUnknown)
    ));
}
