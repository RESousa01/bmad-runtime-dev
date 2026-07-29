//! Generated-contract artifact transitions.

use crate::generated_contracts::{BmadArtifact, BmadArtifactState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadArtifactIntent {
    Validate,
    Persist,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadArtifactTransitionError {
    InvalidTransition {
        from: BmadArtifactState,
        intent: BmadArtifactIntent,
    },
}

/// Applies one pure lifecycle intent to a generated artifact contract.
///
/// # Errors
///
/// Returns [`BmadArtifactTransitionError::InvalidTransition`] when validation,
/// persistence, or rejection is not valid for the current state.
pub fn apply_artifact_intent(
    artifact: &mut BmadArtifact,
    intent: BmadArtifactIntent,
) -> Result<(), BmadArtifactTransitionError> {
    let from = artifact.state;
    artifact.state = match (from, intent) {
        (BmadArtifactState::Proposed, BmadArtifactIntent::Validate) => BmadArtifactState::Validated,
        (BmadArtifactState::Validated, BmadArtifactIntent::Persist) => BmadArtifactState::Persisted,
        (
            BmadArtifactState::Proposed | BmadArtifactState::Validated,
            BmadArtifactIntent::Reject,
        ) => BmadArtifactState::Rejected,
        _ => return Err(BmadArtifactTransitionError::InvalidTransition { from, intent }),
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;

    use super::*;

    fn artifact() -> BmadArtifact {
        serde_json::from_value(json!({
            "artifactId": "artifact_01J00000000000000000000000",
            "contentHash": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            "evidenceHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            "kind": "document",
            "schemaVersion": "sapphirus.bmad-artifact.v1",
            "sessionId": "session_01J00000000000000000000000",
            "state": "proposed"
        }))
        .expect("generated artifact")
    }

    #[test]
    fn artifact_transition_matrix_requires_validation_before_persistence() {
        let states = [
            BmadArtifactState::Proposed,
            BmadArtifactState::Validated,
            BmadArtifactState::Persisted,
            BmadArtifactState::Rejected,
        ];
        let intents = [
            BmadArtifactIntent::Validate,
            BmadArtifactIntent::Persist,
            BmadArtifactIntent::Reject,
        ];
        for from in states {
            for intent in intents {
                let expected = match (from, intent) {
                    (BmadArtifactState::Proposed, BmadArtifactIntent::Validate) => {
                        Some(BmadArtifactState::Validated)
                    }
                    (BmadArtifactState::Validated, BmadArtifactIntent::Persist) => {
                        Some(BmadArtifactState::Persisted)
                    }
                    (
                        BmadArtifactState::Proposed | BmadArtifactState::Validated,
                        BmadArtifactIntent::Reject,
                    ) => Some(BmadArtifactState::Rejected),
                    _ => None,
                };
                let mut value = artifact();
                value.state = from;
                let result = apply_artifact_intent(&mut value, intent);
                assert_eq!(result.is_ok(), expected.is_some(), "{from:?} + {intent:?}");
                assert_eq!(value.state, expected.unwrap_or(from));
            }
        }
    }
}
