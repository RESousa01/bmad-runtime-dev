//! Generated-contract session transitions.

use crate::generated_contracts::{BmadSession, BmadSessionSessionControl, BmadSessionState};

/// Host intent accepted by the session domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadSessionIntent {
    Start,
    RequireInput,
    Pause,
    Resume,
    Complete,
    Fail,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadSessionTransitionError {
    InvalidTransition {
        from: BmadSessionState,
        intent: BmadSessionIntent,
    },
}

/// Applies one pure transition to the generated BMAD session contract.
///
/// The caller persists the returned contract only after its own authority
/// checks. No host request, renderer, model, or provider type crosses here.
///
/// # Errors
///
/// Returns [`BmadSessionTransitionError::InvalidTransition`] when the intent is
/// not valid for the contract's current state.
pub fn apply_session_intent(
    session: &mut BmadSession,
    intent: BmadSessionIntent,
) -> Result<(), BmadSessionTransitionError> {
    let from = session.state;
    let next = match (from, intent) {
        (BmadSessionState::Created, BmadSessionIntent::Start)
        | (BmadSessionState::InputRequired | BmadSessionState::Paused, BmadSessionIntent::Resume) => {
            BmadSessionState::Running
        }
        (BmadSessionState::Running, BmadSessionIntent::RequireInput) => {
            BmadSessionState::InputRequired
        }
        (BmadSessionState::Running, BmadSessionIntent::Pause) => BmadSessionState::Paused,
        (BmadSessionState::Running, BmadSessionIntent::Complete) => BmadSessionState::Completed,
        (BmadSessionState::Running, BmadSessionIntent::Fail) => BmadSessionState::Failed,
        (
            BmadSessionState::Created
            | BmadSessionState::Running
            | BmadSessionState::InputRequired
            | BmadSessionState::Paused,
            BmadSessionIntent::Cancel,
        ) => BmadSessionState::Cancelled,
        _ => return Err(BmadSessionTransitionError::InvalidTransition { from, intent }),
    };
    session.state = next;
    session.control = control_for(next);
    Ok(())
}

const fn control_for(state: BmadSessionState) -> BmadSessionSessionControl {
    BmadSessionSessionControl {
        cancel_allowed: matches!(
            state,
            BmadSessionState::Created
                | BmadSessionState::Running
                | BmadSessionState::InputRequired
                | BmadSessionState::Paused
        ),
        pause_allowed: matches!(state, BmadSessionState::Running),
        resume_allowed: matches!(
            state,
            BmadSessionState::InputRequired | BmadSessionState::Paused
        ),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;

    use super::*;

    fn session() -> BmadSession {
        serde_json::from_value(json!({
            "agentProfile": "stateless",
            "artifacts": [],
            "capabilityId": "capability_01J00000000000000000000000",
            "control": {
                "cancelAllowed": true,
                "pauseAllowed": false,
                "resumeAllowed": false
            },
            "evidence": [],
            "mode": "interactive",
            "schemaVersion": "sapphirus.bmad-session.v1",
            "version": 1,
            "identities": {
                "sourceId": "source_01J00000000000000000000000",
                "packageId": "package_01J00000000000000000000000",
                "configHash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "customizationHash": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "modelId": "model_01J00000000000000000000000",
                "taskId": "task_01J00000000000000000000000",
                "workspaceId": "workspace_01J00000000000000000000000",
                "policyHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222"
            },
            "evidenceHash": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            "sessionId": "session_01J00000000000000000000000",
            "state": "created"
        }))
        .expect("generated session")
    }

    #[test]
    fn generated_session_transition_matrix_is_closed() {
        let states = [
            BmadSessionState::Created,
            BmadSessionState::Running,
            BmadSessionState::InputRequired,
            BmadSessionState::Paused,
            BmadSessionState::Completed,
            BmadSessionState::Failed,
            BmadSessionState::Cancelled,
        ];
        let intents = [
            BmadSessionIntent::Start,
            BmadSessionIntent::RequireInput,
            BmadSessionIntent::Pause,
            BmadSessionIntent::Resume,
            BmadSessionIntent::Complete,
            BmadSessionIntent::Fail,
            BmadSessionIntent::Cancel,
        ];

        for from in states {
            for intent in intents {
                let expected = match (from, intent) {
                    (BmadSessionState::Created, BmadSessionIntent::Start)
                    | (
                        BmadSessionState::InputRequired | BmadSessionState::Paused,
                        BmadSessionIntent::Resume,
                    ) => Some(BmadSessionState::Running),
                    (BmadSessionState::Running, BmadSessionIntent::RequireInput) => {
                        Some(BmadSessionState::InputRequired)
                    }
                    (BmadSessionState::Running, BmadSessionIntent::Pause) => {
                        Some(BmadSessionState::Paused)
                    }
                    (BmadSessionState::Running, BmadSessionIntent::Complete) => {
                        Some(BmadSessionState::Completed)
                    }
                    (BmadSessionState::Running, BmadSessionIntent::Fail) => {
                        Some(BmadSessionState::Failed)
                    }
                    (
                        BmadSessionState::Created
                        | BmadSessionState::Running
                        | BmadSessionState::InputRequired
                        | BmadSessionState::Paused,
                        BmadSessionIntent::Cancel,
                    ) => Some(BmadSessionState::Cancelled),
                    _ => None,
                };
                let mut value = session();
                value.state = from;
                let result = apply_session_intent(&mut value, intent);
                assert_eq!(result.is_ok(), expected.is_some(), "{from:?} + {intent:?}");
                if let Some(next) = expected {
                    assert_eq!(value.state, next);
                    assert_eq!(
                        value.control.pause_allowed,
                        next == BmadSessionState::Running
                    );
                    assert_eq!(
                        value.control.resume_allowed,
                        matches!(
                            next,
                            BmadSessionState::InputRequired | BmadSessionState::Paused
                        )
                    );
                } else {
                    assert_eq!(value.state, from, "invalid transition mutated state");
                }
            }
        }
    }
}
