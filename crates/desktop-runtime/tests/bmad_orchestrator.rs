//! Integration coverage for ENG-08: deterministic action and model
//! orchestration.
//!
//! These tests exercise `orchestrator::plan_source_steps`,
//! `action_plan::build_plan`/`validate_plan`, and
//! `model_port::accept_model_response` purely through
//! `desktop_runtime`'s public surface, mirroring how a host caller would.

use desktop_runtime::generated_contracts::{
    BmadActionPlan, BmadActionPlanAuthorityClass, BmadActionPlanBudget, BmadActionPlanEffect,
    BmadActionPlanEffectEffectClass, BmadActionPlanKind,
};
use desktop_runtime::{
    accept_model_response, build_plan, plan_source_steps, record_retry, validate_plan,
    BmadActionPlanRequest, BmadClosedOutputSchema, BmadModelCallUsage, BmadModelPortError,
    BmadOrchestratorError, BmadSourceStep,
};
use serde::Deserialize;
use serde_json::{json, Value};

fn budget(max_model_calls: i64) -> BmadActionPlanBudget {
    serde_json::from_value(json!({
        "maxModelCalls": max_model_calls,
        "maxRepairs": 1,
        "maxRetries": 1,
        "maxTokens": 1000
    }))
    .expect("well-formed test budget")
}

fn steps() -> Vec<BmadSourceStep> {
    vec![
        BmadSourceStep {
            plan_id: "plan_01J00000000000000000000001".to_owned(),
            source_id: "source_01J00000000000000000000000".to_owned(),
            step_index: 0,
            text: "write:notes/summary.md".to_owned(),
        },
        BmadSourceStep {
            plan_id: "plan_01J00000000000000000000002".to_owned(),
            source_id: "source_01J00000000000000000000000".to_owned(),
            step_index: 1,
            text: "read:notes/spec.md".to_owned(),
        },
        BmadSourceStep {
            plan_id: "plan_01J00000000000000000000003".to_owned(),
            source_id: "source_01J00000000000000000000000".to_owned(),
            step_index: 2,
            text: "model:analyze".to_owned(),
        },
    ]
}

fn plans() -> Vec<BmadActionPlan> {
    plan_source_steps(
        "capability_01J00000000000000000000000",
        "session_01J00000000000000000000000",
        &budget(2),
        &steps(),
    )
    .expect("declared steps build")
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct NoteFinding {
    summary: String,
}

impl BmadClosedOutputSchema for NoteFinding {
    fn schema_id() -> &'static str {
        "sapphirus.bmad-model-note.v1"
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OtherSchema {
    value: i64,
}

impl BmadClosedOutputSchema for OtherSchema {
    fn schema_id() -> &'static str {
        "sapphirus.other-schema.v1"
    }
}

// Acceptance 1: skill text cannot directly write/run/install/schedule.
// A step declaring a filesystem write becomes a `write_proposal` plan
// carrying `requires_approval` and a hashed effect proposal — never an
// executed effect, because the orchestrator has no execution port to have
// executed it with. We prove the "no executed effect anywhere" half by
// serializing every emitted plan and checking the JSON never contains an
// execution-shaped key (`executed`, `handle`, `result`, `stdout`, `pid`),
// only the closed effect-proposal shape the contract defines.
#[test]
fn write_step_yields_a_requires_approval_proposal_with_no_executed_effect() {
    let emitted = plans();
    let write_plan = &emitted[0];
    assert_eq!(write_plan.kind, BmadActionPlanKind::WriteProposal);
    assert_eq!(
        write_plan.authority_class,
        BmadActionPlanAuthorityClass::RequiresApproval
    );
    assert!(write_plan.effect.is_some());

    let serialized = serde_json::to_value(&emitted).expect("plans serialize");
    let text = serialized.to_string();
    for forbidden in ["executed", "\"handle\"", "\"result\"", "stdout", "\"pid\""] {
        assert!(
            !text.contains(forbidden),
            "serialized plan set must never carry an execution handle, found {forbidden}"
        );
    }
}

// Acceptance 2 (authority cannot be declared, only derived): a
// command_proposal or script_plan hand-declaring `authorityClass: none`
// fails closed at validation, proving authority is never trusted from a
// caller-supplied field.
#[test]
fn command_proposal_declaring_none_authority_is_rejected() {
    let request = BmadActionPlanRequest {
        capability_id: "capability_01J00000000000000000000000".to_owned(),
        session_id: "session_01J00000000000000000000000".to_owned(),
        plan_id: "plan_01J00000000000000000000004".to_owned(),
        step_index: 0,
        kind: BmadActionPlanKind::CommandProposal,
        source_id: "source_01J00000000000000000000000".to_owned(),
        step_hash: desktop_runtime::Sha256Digest::from_bytes([3; 32]),
        customization_hash: None,
        budget: budget(1),
        model_call: None,
        reads: Vec::new(),
        effect: Some(BmadActionPlanEffect {
            effect_class: BmadActionPlanEffectEffectClass::Process,
            reversible: false,
            target_hash: desktop_runtime::Sha256Digest::from_bytes([4; 32])
                .to_string()
                .parse()
                .expect("well-formed digest"),
        }),
    };
    let mut plan = build_plan(&request).expect("plan builds with correct authority");
    assert_eq!(
        plan.authority_class,
        BmadActionPlanAuthorityClass::RequiresApproval
    );

    // An adversarial declaration lowering authority after construction.
    plan.authority_class = BmadActionPlanAuthorityClass::None;
    assert_eq!(
        validate_plan(&plan),
        Err(desktop_runtime::BmadActionPlanError::AuthorityMismatch)
    );
}

// Acceptance 2: a model response failing its declared closed output schema
// is rejected, and accepting a response can never raise the plan's
// authority — the accepted value's type (`NoteFinding`) has no authority
// field at all, so there is nothing in it to raise.
#[test]
fn model_response_failing_its_closed_schema_is_rejected_and_cannot_raise_authority() {
    let model_plan = &plans()[2];
    assert_eq!(model_plan.kind, BmadActionPlanKind::ModelCall);
    assert_eq!(
        model_plan.authority_class,
        BmadActionPlanAuthorityClass::None
    );

    let mut usage = BmadModelCallUsage::default();

    // Wrong schema id entirely.
    let mismatched =
        accept_model_response::<OtherSchema>(model_plan, &json!({ "value": 1 }), 10, &mut usage);
    assert_eq!(mismatched, Err(BmadModelPortError::SchemaMismatch));

    // Correct schema id, but the payload carries an extra field a closed
    // schema must reject.
    let malformed: Value = json!({ "summary": "ok", "extra": true });
    let rejected = accept_model_response::<NoteFinding>(model_plan, &malformed, 10, &mut usage);
    assert_eq!(rejected, Err(BmadModelPortError::SchemaRejected));
    assert_eq!(
        usage,
        BmadModelCallUsage::default(),
        "a rejected call must not consume budget"
    );

    let accepted = accept_model_response::<NoteFinding>(
        model_plan,
        &json!({ "summary": "ok" }),
        10,
        &mut usage,
    )
    .expect("well-formed closed-schema response is accepted");
    assert_eq!(accepted.summary, "ok");
}

// Budget enforcement: exceeding maxModelCalls fails closed even for an
// otherwise well-formed response.
#[test]
fn exceeding_max_model_calls_fails_closed() {
    let model_plan = &plans()[2];
    let mut usage = BmadModelCallUsage::default();
    accept_model_response::<NoteFinding>(model_plan, &json!({ "summary": "one" }), 1, &mut usage)
        .expect("first call within budget");
    // budget() sets maxModelCalls to the caller-supplied value; plans() used 2.
    accept_model_response::<NoteFinding>(model_plan, &json!({ "summary": "two" }), 1, &mut usage)
        .expect("second call within budget");
    let third = accept_model_response::<NoteFinding>(
        model_plan,
        &json!({ "summary": "three" }),
        1,
        &mut usage,
    );
    assert_eq!(third, Err(BmadModelPortError::BudgetExceeded));
}

// Budget enforcement: exceeding maxTokens fails closed.
#[test]
fn exceeding_max_tokens_fails_closed() {
    let model_plan = &plans()[2];
    let mut usage = BmadModelCallUsage::default();
    let result = accept_model_response::<NoteFinding>(
        model_plan,
        &json!({ "summary": "too many tokens" }),
        1_000_000,
        &mut usage,
    );
    assert_eq!(result, Err(BmadModelPortError::BudgetExceeded));
}

// Budget enforcement: exceeding maxRetries fails closed.
#[test]
fn exceeding_max_retries_fails_closed() {
    let model_plan = &plans()[2];
    let mut usage = BmadModelCallUsage::default();
    record_retry(model_plan, &mut usage).expect("first retry within budget");
    let second = record_retry(model_plan, &mut usage);
    assert_eq!(second, Err(BmadModelPortError::BudgetExceeded));
}

// Budget enforcement: exceeding maxRepairs fails closed.
#[test]
fn exceeding_max_repairs_fails_closed() {
    let model_plan = &plans()[2];
    let mut usage = BmadModelCallUsage::default();
    desktop_runtime::record_repair(model_plan, &mut usage).expect("first repair within budget");
    let second = desktop_runtime::record_repair(model_plan, &mut usage);
    assert_eq!(second, Err(BmadModelPortError::BudgetExceeded));
}

// Determinism: the same declared steps produce the same planHash sequence.
#[test]
fn identical_declared_steps_produce_identical_plan_hash_sequences() {
    let first = plans()
        .into_iter()
        .map(|plan| plan.plan_hash)
        .collect::<Vec<_>>();
    let second = plans()
        .into_iter()
        .map(|plan| plan.plan_hash)
        .collect::<Vec<_>>();
    assert_eq!(first, second);
}

// Acceptance 3: ordinary chat remains independent. This is established
// structurally, not by a runtime flag: this whole test file never imports
// any chat/session-transcript type, and every public function used above
// (`plan_source_steps`, `build_plan`, `validate_plan`,
// `accept_model_response`) takes only BMAD source-step/plan inputs
// (`BmadSourceStep`, `BmadActionPlanRequest`, `BmadActionPlan`, closed
// model-response payloads) and returns only `BmadActionPlan`/typed model
// values — never a chat message, transcript, or session type. If any of
// these signatures ever grew a chat-shaped parameter, this file would fail
// to compile without importing `desktop_runtime::chat`, which it does not.
#[test]
fn orchestrator_public_api_takes_no_chat_or_transcript_types() {
    let emitted = plans();
    assert_eq!(emitted.len(), steps().len());
}

// Path safety: an absolute path, a `..` segment, or a backslash is rejected.
#[test]
fn unsafe_read_paths_are_rejected() {
    for text in [
        "read:/etc/passwd",
        "read:../secret.md",
        "read:notes\\spec.md",
    ] {
        let unsafe_steps = vec![BmadSourceStep {
            plan_id: "plan_01J00000000000000000000005".to_owned(),
            source_id: "source_01J00000000000000000000000".to_owned(),
            step_index: 0,
            text: text.to_owned(),
        }];
        let result = plan_source_steps(
            "capability_01J00000000000000000000000",
            "session_01J00000000000000000000000",
            &budget(1),
            &unsafe_steps,
        );
        assert!(
            matches!(result, Err(BmadOrchestratorError::InvalidPlan)),
            "expected {text} to be rejected as an unsafe path"
        );
    }
}

// Contract conformance: a plan round-trips through serde_json::Value into
// the generated contract type.
#[test]
fn a_plan_round_trips_through_the_generated_contract_type() {
    let plan = &plans()[0];
    let value = serde_json::to_value(plan).expect("plan serializes");
    let reparsed: BmadActionPlan =
        serde_json::from_value(value).expect("plan round-trips through the generated contract");
    assert_eq!(reparsed.kind, plan.kind);
    assert_eq!(reparsed.plan_hash, plan.plan_hash);
    validate_plan(&reparsed).expect("round-tripped plan still validates");
}
