//! Completion intent planning.
//!
//! Completion hooks remain inert data here. The host must authorize and
//! execute each returned intent through its normal Airlock path.

use crate::generated_contracts::{
    BmadArtifact, BmadArtifactState, BmadSession, BmadSessionCompletionHookHook, BmadSessionState,
    CommonContractId,
};

#[derive(Clone, Debug)]
pub enum BmadCompletionIntent {
    PersistArtifact(CommonContractId),
    EmitReceipt(CommonContractId),
    NotifyLocal(CommonContractId),
    ScheduleFollowup(CommonContractId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmadCompletionError {
    SessionNotCompleted,
    ArtifactNotReady,
}

/// Converts enabled completion hooks into typed, still-inert host intents.
///
/// # Errors
///
/// Fails closed until the session is complete and no proposed or rejected
/// artifact remains.
pub fn plan_completion(
    session: &BmadSession,
    artifacts: &[BmadArtifact],
) -> Result<Vec<BmadCompletionIntent>, BmadCompletionError> {
    if session.state != BmadSessionState::Completed {
        return Err(BmadCompletionError::SessionNotCompleted);
    }
    if artifacts.iter().any(|artifact| {
        !matches!(
            artifact.state,
            BmadArtifactState::Validated | BmadArtifactState::Persisted
        )
    }) {
        return Err(BmadCompletionError::ArtifactNotReady);
    }

    let mut intents = Vec::new();
    for hook in session.completion_hooks.iter().filter(|hook| hook.enabled) {
        match hook.hook {
            BmadSessionCompletionHookHook::PersistArtifacts => {
                intents.extend(
                    artifacts
                        .iter()
                        .filter(|artifact| artifact.state == BmadArtifactState::Validated)
                        .map(|artifact| {
                            BmadCompletionIntent::PersistArtifact(artifact.artifact_id.clone())
                        }),
                );
            }
            BmadSessionCompletionHookHook::EmitReceipt => {
                intents.push(BmadCompletionIntent::EmitReceipt(
                    session.session_id.clone(),
                ));
            }
            BmadSessionCompletionHookHook::NotifyLocal => {
                intents.push(BmadCompletionIntent::NotifyLocal(
                    session.session_id.clone(),
                ));
            }
            BmadSessionCompletionHookHook::ScheduleFollowup => {
                intents.push(BmadCompletionIntent::ScheduleFollowup(
                    session.session_id.clone(),
                ));
            }
        }
    }
    Ok(intents)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use serde_json::json;

    use super::*;

    fn session(state: &str, hooks: &serde_json::Value) -> BmadSession {
        serde_json::from_value(json!({
            "agentProfile": "stateless",
            "artifacts": [],
            "capabilityId": "capability_01J00000000000000000000000",
            "completionHooks": hooks,
            "control": {
                "cancelAllowed": false,
                "pauseAllowed": false,
                "resumeAllowed": false
            },
            "evidence": [],
            "mode": "headless",
            "schemaVersion": "sapphirus.bmad-session.v1",
            "sessionId": "session_01J00000000000000000000000",
            "state": state,
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
            "evidenceHash": "sha256:3333333333333333333333333333333333333333333333333333333333333333"
        }))
        .expect("generated session")
    }

    fn artifact(state: &str) -> BmadArtifact {
        serde_json::from_value(json!({
            "artifactId": "artifact_01J00000000000000000000000",
            "contentHash": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            "evidenceHash": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            "kind": "document",
            "schemaVersion": "sapphirus.bmad-artifact.v1",
            "sessionId": "session_01J00000000000000000000000",
            "state": state
        }))
        .expect("generated artifact")
    }

    #[test]
    fn completion_rejects_nonterminal_sessions_and_unready_artifacts() {
        for state in [
            "created",
            "running",
            "input_required",
            "paused",
            "failed",
            "cancelled",
        ] {
            assert_eq!(
                plan_completion(&session(state, &json!([])), &[])
                    .expect_err("non-completed session"),
                BmadCompletionError::SessionNotCompleted
            );
        }
        for state in ["proposed", "rejected"] {
            assert_eq!(
                plan_completion(&session("completed", &json!([])), &[artifact(state)])
                    .expect_err("unready artifact"),
                BmadCompletionError::ArtifactNotReady
            );
        }
    }

    #[test]
    fn completion_hook_matrix_returns_only_inert_enabled_intents() {
        let disabled = session(
            "completed",
            &json!([
                {"enabled": false, "hook": "persist_artifacts"},
                {"enabled": false, "hook": "emit_receipt"},
                {"enabled": false, "hook": "notify_local"},
                {"enabled": false, "hook": "schedule_followup"}
            ]),
        );
        assert!(plan_completion(&disabled, &[artifact("validated")])
            .expect("disabled")
            .is_empty());

        let enabled = session(
            "completed",
            &json!([
                {"enabled": true, "hook": "persist_artifacts"},
                {"enabled": true, "hook": "emit_receipt"},
                {"enabled": true, "hook": "notify_local"},
                {"enabled": true, "hook": "schedule_followup"}
            ]),
        );
        let intents =
            plan_completion(&enabled, &[artifact("validated")]).expect("completion intents");
        assert!(matches!(
            intents.as_slice(),
            [
                BmadCompletionIntent::PersistArtifact(_),
                BmadCompletionIntent::EmitReceipt(_),
                BmadCompletionIntent::NotifyLocal(_),
                BmadCompletionIntent::ScheduleFollowup(_)
            ]
        ));
        assert_eq!(
            plan_completion(&enabled, &[artifact("persisted")])
                .expect("already persisted")
                .len(),
            3
        );
    }
}
