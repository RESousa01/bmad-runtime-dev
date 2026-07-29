#![allow(clippy::expect_used, clippy::unwrap_used)]

use desktop_runtime::generated_contracts::{
    BmadInvocation, BmadInvocationDeclaredAction, BmadInvocationDeclaredInput,
    BmadInvocationDeclaredInputValueKind, BmadInvocationMode, BmadInvocationResolvedInputOrigin,
    CommonContractId,
};
use desktop_runtime::{BmadInvocationError, BmadInvocationRequest, BmadInvocationResolver};

fn contract_id(value: &str) -> CommonContractId {
    value.parse().expect("valid contract id")
}

fn declared_input(
    name: &str,
    required: bool,
    value_kind: BmadInvocationDeclaredInputValueKind,
    prompt: Option<&str>,
) -> BmadInvocationDeclaredInput {
    BmadInvocationDeclaredInput {
        allowed_values: Vec::new(),
        name: name.try_into().expect("valid declared input name"),
        prompt: prompt.map(|prompt| prompt.try_into().expect("valid prompt")),
        required,
        value_kind,
    }
}

fn action_with_one_required_path_input() -> BmadInvocationDeclaredAction {
    BmadInvocationDeclaredAction {
        declared_inputs: vec![declared_input(
            "target_path",
            true,
            BmadInvocationDeclaredInputValueKind::Path,
            Some("Which path should the workflow write to?"),
        )],
        name: "run-workflow".try_into().expect("valid action name"),
    }
}

fn ids() -> (CommonContractId, CommonContractId, CommonContractId) {
    (
        contract_id("invocation_01J00000000000000000000000"),
        contract_id("capability_01J00000000000000000000000"),
        contract_id("session_01J00000000000000000000000"),
    )
}

fn resolve(
    action: &BmadInvocationDeclaredAction,
    mode: BmadInvocationMode,
    max_prompted_inputs: i64,
    supplied: &[(String, BmadInvocationResolvedInputOrigin, serde_json::Value)],
) -> Result<BmadInvocation, BmadInvocationError> {
    let (invocation_id, capability_id, session_id) = ids();
    BmadInvocationResolver::resolve(&BmadInvocationRequest {
        invocation_id,
        capability_id,
        session_id,
        mode,
        action,
        supplied,
        max_model_calls: 8,
        max_retries: 3,
        requested_max_prompted_inputs: max_prompted_inputs,
    })
}

// Break caught: acceptance criterion 1 (every declared headless action passes) regressing to a
// resolver that leaves a fully-supplied headless action stuck with a phantom missing input.
#[test]
fn bmad_invocation_headless_fully_supplied_action_is_ready_with_no_missing_inputs() {
    let action = action_with_one_required_path_input();
    let supplied = vec![(
        "target_path".to_owned(),
        BmadInvocationResolvedInputOrigin::Argument,
        serde_json::json!("/workspace/out"),
    )];

    let invocation = resolve(&action, BmadInvocationMode::Headless, 0, &supplied)
        .expect("fully-supplied headless action resolves");

    assert_eq!(invocation.ready, Some(true));
    assert!(invocation.missing_inputs.is_empty());
    assert_eq!(invocation.resolved_inputs.len(), 1);
    assert_eq!(
        invocation.resolved_inputs[0].origin,
        BmadInvocationResolvedInputOrigin::Argument
    );
}

// Break caught: acceptance criterion 3 (headless never invents required decisions). A resolver
// that fills a missing required input from an empty string, a heuristic, or the declared value
// kind's zero value would pass a naive "ready" check while smuggling a fabricated value through.
#[test]
fn bmad_invocation_headless_missing_required_input_yields_input_requirement_and_never_fabricates_a_value(
) {
    let action = action_with_one_required_path_input();

    let invocation = resolve(&action, BmadInvocationMode::Headless, 0, &[])
        .expect("headless resolution without a value still produces a contract");

    assert_eq!(invocation.ready, Some(false));
    assert!(invocation.resolved_inputs.is_empty());
    assert_eq!(invocation.missing_inputs.len(), 1);
    let requirement = &invocation.missing_inputs[0];
    assert_eq!(requirement.name.as_str(), "target_path");
    assert_eq!(requirement.code.as_str(), "missing_required_input");

    // No resolved input exists for the missing name in either mode's output, and the serialized
    // contract carries no free-form "value" field a fabricated answer could hide behind.
    let serialized = serde_json::to_value(&invocation).expect("invocation serializes");
    let text = serde_json::to_string(&serialized).expect("invocation serializes to text");
    assert!(!text.contains("\"value\":"));
    assert!(invocation
        .resolved_inputs
        .iter()
        .all(|resolved| resolved.name.as_str() != "target_path"));
}

// Break caught: the parity guarantee (acceptance 1 read together with the mode-difference rule)
// collapsing if headless or interactive resolution paths diverge for equivalent supplied inputs,
// e.g. one path hashing the mode or budget into resolvedInputsHash.
#[test]
fn bmad_invocation_interactive_and_headless_produce_identical_resolved_inputs_hash_for_the_same_supplied_values(
) {
    let action = action_with_one_required_path_input();
    let supplied = vec![(
        "target_path".to_owned(),
        BmadInvocationResolvedInputOrigin::Argument,
        serde_json::json!("/workspace/out"),
    )];

    let headless = resolve(&action, BmadInvocationMode::Headless, 0, &supplied)
        .expect("headless resolves with the supplied argument");
    let interactive = resolve(&action, BmadInvocationMode::Interactive, 4, &supplied)
        .expect("interactive resolves with the supplied argument");

    assert_eq!(
        headless.resolved_inputs_hash.as_str(),
        interactive.resolved_inputs_hash.as_str()
    );
    assert_ne!(headless.mode, interactive.mode);
    assert_eq!(headless.budget.max_prompted_inputs, 0);
    assert_eq!(interactive.budget.max_prompted_inputs, 4);
}

// Break caught: acceptance criterion 2 (interactive runs resume after restart). If the resolver
// captured any hidden, non-serializable state, persisting and reloading the contract across a
// restart and resupplying the same origins would not reproduce the same resolution.
#[test]
fn bmad_invocation_interactive_resolution_round_trips_through_json_and_re_resolves_identically() {
    let action = action_with_one_required_path_input();
    let supplied = vec![(
        "target_path".to_owned(),
        BmadInvocationResolvedInputOrigin::Answer,
        serde_json::json!("/workspace/answered"),
    )];

    let before_restart = resolve(&action, BmadInvocationMode::Interactive, 4, &supplied)
        .expect("interactive resolution with an answered prompt");

    // Simulate a restart: the only thing that survives is the serialized contract.
    let persisted = serde_json::to_value(&before_restart).expect("invocation serializes");
    let reloaded: BmadInvocation =
        serde_json::from_value(persisted).expect("invocation deserializes back to the contract");
    assert_eq!(
        reloaded.resolved_inputs_hash.as_str(),
        before_restart.resolved_inputs_hash.as_str()
    );

    // Resupplying the same origin/value after "restart" re-resolves to an identical invocation.
    let after_restart = resolve(&action, BmadInvocationMode::Interactive, 4, &supplied)
        .expect("re-resolution after restart");
    assert_eq!(
        after_restart.resolved_inputs_hash.as_str(),
        reloaded.resolved_inputs_hash.as_str()
    );
    assert_eq!(after_restart.ready, reloaded.ready);
    assert_eq!(
        after_restart.resolved_inputs[0].origin,
        reloaded.resolved_inputs[0].origin
    );
}

// Break caught: the fixed precedence order (argument > answer > customization > config >
// declared_default) silently swapping, which would let a lower-trust origin like config override
// an explicit argument.
#[test]
fn bmad_invocation_argument_origin_beats_config_customization_and_declared_default() {
    let action = action_with_one_required_path_input();
    let supplied = vec![
        (
            "target_path".to_owned(),
            BmadInvocationResolvedInputOrigin::DeclaredDefault,
            serde_json::json!("/default/path"),
        ),
        (
            "target_path".to_owned(),
            BmadInvocationResolvedInputOrigin::Config,
            serde_json::json!("/config/path"),
        ),
        (
            "target_path".to_owned(),
            BmadInvocationResolvedInputOrigin::Customization,
            serde_json::json!("/customization/path"),
        ),
        (
            "target_path".to_owned(),
            BmadInvocationResolvedInputOrigin::Argument,
            serde_json::json!("/argument/path"),
        ),
    ];

    let invocation = resolve(&action, BmadInvocationMode::Headless, 0, &supplied)
        .expect("headless resolves with a winning argument");

    assert_eq!(invocation.resolved_inputs.len(), 1);
    assert_eq!(
        invocation.resolved_inputs[0].origin,
        BmadInvocationResolvedInputOrigin::Argument
    );

    let argument_only = resolve(
        &action,
        BmadInvocationMode::Headless,
        0,
        &[(
            "target_path".to_owned(),
            BmadInvocationResolvedInputOrigin::Argument,
            serde_json::json!("/argument/path"),
        )],
    )
    .expect("headless resolves with only the argument supplied");
    assert_eq!(
        invocation.resolved_inputs_hash.as_str(),
        argument_only.resolved_inputs_hash.as_str(),
        "the losing origins must not perturb the winning value's hash"
    );
}

// Break caught: acceptance criterion 3 read together with the budget rule. A resolver that
// accepted an "answer" origin in headless mode would be recording a prompted answer that headless
// must never produce, and a resolver that let the caller raise maxPromptedInputs above zero in
// headless mode would silently reopen the door to prompting in an unattended run.
#[test]
fn bmad_invocation_headless_budget_forces_zero_prompted_inputs_and_rejects_a_prompted_answer() {
    let action = action_with_one_required_path_input();

    let requested_budget_ignored = resolve(&action, BmadInvocationMode::Headless, 200, &[])
        .expect("headless resolves even with no supplied inputs");
    assert_eq!(requested_budget_ignored.budget.max_prompted_inputs, 0);

    let rejected = resolve(
        &action,
        BmadInvocationMode::Headless,
        0,
        &[(
            "target_path".to_owned(),
            BmadInvocationResolvedInputOrigin::Answer,
            serde_json::json!("/answered/path"),
        )],
    );
    assert_eq!(
        rejected.unwrap_err(),
        BmadInvocationError::HeadlessAnswerRejected
    );
}
