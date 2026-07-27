//! ADR-0007: converts a stored, verified `governed_change_set` capability
//! result into ordinary D3 proposed changes.
//!
//! The model's declared preimage hashes are a staleness tripwire: every
//! replace/delete target is re-read through governed workspace I/O and
//! compared before any proposal exists. The output of this module feeds
//! the existing `changes.propose` review path unchanged — model origin
//! grants nothing.
//!
//! The comparison is against live on-disk bytes, and it must stay that way:
//! comparing against anything the model was shown would make the tripwire
//! agree with itself. What the model echoes is the *pre-redaction* hash of
//! the workspace file (`AuthorizedContextItem::original_content_hash`,
//! printed as `reviewedContentHash`), which is exactly what a fresh read of
//! an unchanged file produces. Feeding the model the post-redaction hash
//! instead — as this path once did — made every replace or delete against a
//! file containing a redactable pattern fail here permanently, under a
//! message claiming the workspace had moved when nothing had.

use desktop_runtime::{
    deserialize_strict_json, sha256_bytes, BmadCandidateChange, BmadCapabilityOutput, LocalError,
    LocalErrorCode, ProposedFileChange,
};

use crate::bmad_capability_host::parse_capability_result;
use crate::state::HostState;

const MAX_PROPOSAL_FILE_BYTES: u64 = 1_048_576;

fn integrity_error(message: &'static str) -> LocalError {
    LocalError::new(LocalErrorCode::IntegrityFailure, message, false)
}

/// Parses one stored capability-result JSON and converts its change set
/// into D3 proposed changes, verifying model-declared preimages against
/// freshly observed workspace bytes.
///
/// # Errors
///
/// Fails closed when the stored result is not a governed change set, a
/// declared preimage no longer matches the workspace (the workspace moved
/// after the model saw it), a delete target is missing, or a create
/// target already exists.
pub(crate) fn proposed_changes_from_stored_result(
    state: &HostState,
    workspace_id: &str,
    result_json: &str,
) -> Result<Vec<ProposedFileChange>, LocalError> {
    let value: serde_json::Value = deserialize_strict_json(result_json.as_bytes())
        .map_err(|_| integrity_error("The stored capability result is unreadable."))?;
    let output = parse_capability_result(&value)
        .map_err(|_| integrity_error("The stored capability result failed verification."))?;
    let BmadCapabilityOutput::GovernedChangeSet(change_set) = output else {
        return Err(integrity_error(
            "Only governed change sets can enter the changes review.",
        ));
    };

    let mut proposed = Vec::with_capacity(change_set.changes.len());
    for change in change_set.changes {
        match change {
            BmadCandidateChange::Create { path, content } => {
                // A create against an existing file is a stale model view.
                if state
                    .workspace
                    .read_text(workspace_id, path.as_str(), MAX_PROPOSAL_FILE_BYTES)
                    .is_ok()
                {
                    return Err(integrity_error(
                        "A file the model would create already exists; re-run against current context.",
                    ));
                }
                // Nothing is on disk, so every redaction marker in the body is
                // net-new by definition. See the `Replace` arm for why.
                if desktop_egress::reintroduced_redaction_marker("", &content).is_some() {
                    return Err(integrity_error(
                        "The model's new file contains redacted-secret markers from the context it was shown; re-run against current context.",
                    ));
                }
                proposed.push(ProposedFileChange::SetContent {
                    relative_path: path,
                    content,
                });
            }
            BmadCandidateChange::Replace {
                path,
                content,
                preimage_sha256,
            } => {
                let observed = state
                    .workspace
                    .read_text(workspace_id, path.as_str(), MAX_PROPOSAL_FILE_BYTES)
                    .map_err(|_| {
                        integrity_error(
                            "A file the model would replace could not be read; re-run against current context.",
                        )
                    })?;
                if observed.truncated
                    || sha256_bytes(observed.content.as_bytes()) != preimage_sha256
                {
                    return Err(integrity_error(
                        "The workspace changed after the model saw it; re-run against current context.",
                    ));
                }
                // The preimage check above is a *staleness* tripwire, and it
                // passes here by construction: the file has not moved. What it
                // cannot see is a faithful echo of the *redacted view* the model
                // was shown. Egress rewrites secrets to `[REDACTED:…]` before
                // the body leaves the machine, so a whole-file replace that
                // repeats what the model read writes those markers over the
                // user's real credential — destroying it, in the one lane whose
                // whole purpose is governed writes.
                //
                // Net-new marker counts, not presence: a file may legitimately
                // contain the literal text (a redaction corpus, a ticket quoting
                // an egress preview), and refusing those would be another false
                // alarm of exactly the kind this codebase keeps producing.
                if desktop_egress::reintroduced_redaction_marker(&observed.content, &content)
                    .is_some()
                {
                    return Err(integrity_error(
                        "The model's replacement writes redacted-secret markers over real content; re-run against current context.",
                    ));
                }
                proposed.push(ProposedFileChange::SetContent {
                    relative_path: path,
                    content,
                });
            }
            BmadCandidateChange::Delete {
                path,
                preimage_sha256,
            } => {
                let observed = state
                    .workspace
                    .read_text(workspace_id, path.as_str(), MAX_PROPOSAL_FILE_BYTES)
                    .map_err(|_| {
                        integrity_error(
                            "A file the model would delete could not be read; re-run against current context.",
                        )
                    })?;
                if observed.truncated
                    || sha256_bytes(observed.content.as_bytes()) != preimage_sha256
                {
                    return Err(integrity_error(
                        "The workspace changed after the model saw it; re-run against current context.",
                    ));
                }
                proposed.push(ProposedFileChange::Delete {
                    relative_path: path,
                });
            }
        }
    }
    Ok(proposed)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::proposed_changes_from_stored_result;
    use crate::state::HostState;
    use desktop_runtime::sha256_bytes;

    type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

    /// The literal a model echoes back after egress rewrote a secret. Split so
    /// this file never carries a whole marker as one token, matching the way
    /// `desktop-egress`'s own fixtures avoid tripping the repository scanner.
    const MARKER: &str = concat!("[REDACTED:", "credential]");

    fn stored_replace(path: &str, content: &str, preimage: &str) -> String {
        let digest = format!("sha256:{}", sha256_bytes(preimage.as_bytes()).hex_value());
        let content = serde_json::to_string(content).expect("content json");
        format!(
            r#"{{"resultKind":"governed_change_set","governedChangeSet":{{"schemaVersion":"sapphirus.bmad-governed-change-set.v1","summary":"Rotate the key.","changes":[{{"operation":"replace","path":"{path}","content":{content},"preimageSha256":"{digest}"}}]}}}}"#
        )
    }

    fn granted_workspace(body: &str) -> TestResult<(HostState, String, tempfile::TempDir)> {
        let storage = tempfile::tempdir()?;
        let state = HostState::initialize(Some(storage.path().join("authority")))
            .map_err(|error| format!("{error:?}"))?;
        let root = tempfile::tempdir()?;
        std::fs::write(root.path().join("notes.md"), body)?;
        let projection = state
            .workspace
            .grant("project_forge_redaction", root.path())
            .map_err(|error| format!("{error:?}"))?;
        // `root` is returned so the directory outlives the broker grant.
        let _ = storage.keep();
        Ok((state, projection.workspace_id, root))
    }

    /// The preimage check above this guard is a *staleness* tripwire and passes
    /// by construction here — the file has not moved. Only the marker check can
    /// see that the model is echoing the redacted view it was shown, which would
    /// write `[REDACTED:…]` over the user's real credential.
    ///
    /// This test exists because the guard was built in `desktop-egress` and
    /// initially not wired into this path at all. An unwired guard reads like
    /// protection and is not.
    #[test]
    fn a_replacement_echoing_the_redacted_view_is_refused() -> TestResult {
        let on_disk = "key: hunter2-correct-horse\n";
        let (state, workspace_id, _root) = granted_workspace(on_disk)?;

        let echoed = format!("key: {MARKER}\n");
        let error = proposed_changes_from_stored_result(
            &state,
            &workspace_id,
            &stored_replace("notes.md", &echoed, on_disk),
        )
        .expect_err("a body reintroducing a redaction marker must be refused");
        assert!(
            error.safe_message.contains("redacted-secret markers"),
            "unexpected refusal: {}",
            error.safe_message,
        );
        Ok(())
    }

    /// The control. Without it the test above would also pass if this path
    /// refused every replacement.
    #[test]
    fn an_ordinary_replacement_is_still_accepted() -> TestResult {
        let on_disk = "key: hunter2-correct-horse\n";
        let (state, workspace_id, _root) = granted_workspace(on_disk)?;

        let proposed = proposed_changes_from_stored_result(
            &state,
            &workspace_id,
            &stored_replace("notes.md", "key: rotated-value\n", on_disk),
        )
        .map_err(|error| error.safe_message)?;
        assert_eq!(proposed.len(), 1);
        Ok(())
    }

    /// Net-new counts, not presence. A file that legitimately documents the
    /// marker text — a redaction corpus, a ticket quoting an egress preview —
    /// must stay editable, or the guard becomes the next false alarm.
    #[test]
    fn a_file_that_already_documents_a_marker_stays_editable() -> TestResult {
        let on_disk = format!("The scanner writes {MARKER} in place of a secret.\n");
        let (state, workspace_id, _root) = granted_workspace(&on_disk)?;

        let edited = format!("The scanner writes {MARKER} in place of any secret.\n");
        let proposed = proposed_changes_from_stored_result(
            &state,
            &workspace_id,
            &stored_replace("notes.md", &edited, &on_disk),
        )
        .map_err(|error| error.safe_message)?;
        assert_eq!(proposed.len(), 1);
        Ok(())
    }
}
