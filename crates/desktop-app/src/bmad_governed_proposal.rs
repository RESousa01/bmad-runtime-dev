//! ADR-0007: converts a stored, verified `governed_change_set` capability
//! result into ordinary D3 proposed changes.
//!
//! The model's declared preimage hashes are a staleness tripwire: every
//! replace/delete target is re-read through governed workspace I/O and
//! compared before any proposal exists. The output of this module feeds
//! the existing `changes.propose` review path unchanged — model origin
//! grants nothing.

use desktop_runtime::{
    sha256_bytes, BmadCandidateChange, BmadCapabilityOutput, LocalError, LocalErrorCode,
    ProposedFileChange,
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
    let value: serde_json::Value = serde_json::from_str(result_json)
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
