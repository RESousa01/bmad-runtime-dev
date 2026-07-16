//! Governed local edits: pure construction of a Windows patch candidate from
//! an untrusted proposed change set plus host-observed preimages, and the
//! exact review projection whose canonical hash an approval decision binds.
//!
//! Nothing in this module performs I/O. Proposed content is untrusted data:
//! it can never name absolute paths, authority fields, limits, or policy. The
//! host observes preimages itself and this module fails closed on any
//! mismatch between the proposal and those observations.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    canonical_hash, sha256_bytes, AuthorityRef, CandidateCommon, CanonicalHashError, ContractId,
    DeclaredWrite, DeclaredWriteOperation, DeliveryModel, DomainValidationError, ExecutionLimits,
    LocalPathPreimage, MutableInputBinding, NativePatchEngineAudience, PatchOperation, PatchSet,
    RelativeWorkspacePath, RollbackClass, Sha256Digest, UnixMillis, WindowsPatchCandidate,
    WindowsPatchCandidateDraft, WorkspaceTarget, HARD_MAX_CHANGED_FILES,
};

pub const CHANGES_REVIEW_SCHEMA: &str = "sapphirus.changes-review.v1";

/// One untrusted proposed file change from the renderer or a later model
/// adapter. Paths are validated relative workspace paths; everything else the
/// candidate needs is observed and bound by the host.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "change", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProposedFileChange {
    SetContent {
        #[serde(rename = "relativePath")]
        relative_path: RelativeWorkspacePath,
        content: String,
    },
    Delete {
        #[serde(rename = "relativePath")]
        relative_path: RelativeWorkspacePath,
    },
}

impl ProposedFileChange {
    #[must_use]
    pub const fn relative_path(&self) -> &RelativeWorkspacePath {
        match self {
            Self::SetContent { relative_path, .. } | Self::Delete { relative_path } => {
                relative_path
            }
        }
    }
}

/// Host-observed facts about one proposed path, captured through the governed
/// workspace broker immediately before candidate construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedPreimage {
    pub relative_path: RelativeWorkspacePath,
    pub exists: bool,
    pub content: Option<String>,
    pub content_hash: Option<Sha256Digest>,
    pub file_identity_hash: Option<Sha256Digest>,
    pub metadata_hash: Option<Sha256Digest>,
}

/// Host-owned authority facts bound into one changes proposal.
#[derive(Clone, Debug)]
pub struct ChangesProposalBinding {
    pub proposal_id: ContractId,
    pub candidate_id: ContractId,
    pub project_id: ContractId,
    pub run_id: ContractId,
    pub owner_scope_ref: ContractId,
    pub authority_ref: AuthorityRef,
    pub policy_context_hash: Sha256Digest,
    pub workspace_target: WorkspaceTarget,
    pub executor_audience: NativePatchEngineAudience,
    pub mutable_inputs: Vec<MutableInputBinding>,
    pub created_at: UnixMillis,
    pub expires_at: UnixMillis,
}

/// The sealed output of proposal construction.
#[derive(Clone, Debug)]
pub struct PreparedChangesProposal {
    pub patch: PatchSet,
    pub candidate: WindowsPatchCandidate,
    pub proposal_hash: Sha256Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangesProposalKind {
    Edit,
    Undo,
}

/// One reviewed file change exactly as displayed to the user.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeReview {
    pub relative_path: RelativeWorkspacePath,
    pub operation: DeclaredWriteOperation,
    pub before_content: Option<String>,
    pub after_content: Option<String>,
    pub before_hash: Option<Sha256Digest>,
    pub after_hash: Option<Sha256Digest>,
    pub before_bytes: u64,
    pub after_bytes: u64,
}

/// The bounded renderer review projection. Its canonical hash is the
/// `displayed_diff_hash` an approval decision must echo, so approval binds
/// the exact bytes the user reviewed, not merely a candidate identifier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesReviewProjection {
    pub schema_version: String,
    pub proposal_id: ContractId,
    pub candidate_id: ContractId,
    pub candidate_hash: Sha256Digest,
    pub workspace_id: String,
    pub workspace_grant_epoch: u64,
    pub proposal_kind: ChangesProposalKind,
    pub source_execution_id: Option<ContractId>,
    pub files: Vec<FileChangeReview>,
    pub total_changed_bytes: u64,
    pub created_at: UnixMillis,
    pub expires_at: UnixMillis,
}

impl ChangesReviewProjection {
    /// Computes the canonical hash of the exact displayed review content.
    ///
    /// # Errors
    ///
    /// Returns [`CanonicalHashError`] when canonical serialization fails.
    pub fn displayed_diff_hash(&self) -> Result<Sha256Digest, CanonicalHashError> {
        canonical_hash("changes-review", 1, self)
    }
}

#[derive(Debug, Error)]
pub enum EditsError {
    #[error("a proposal must contain at least one file change")]
    EmptyProposal,
    #[error("the proposal exceeds the governed changed-file limit")]
    TooManyChanges,
    #[error("the proposal names the same path more than once")]
    DuplicatePath,
    #[error("an observed preimage does not align with the proposed change set")]
    PreimageMismatch,
    #[error("the proposed content is identical to the observed file")]
    NoOpChange,
    #[error("the proposed deletion targets a file that does not exist")]
    MissingTarget,
    #[error(transparent)]
    InvalidDomain(#[from] DomainValidationError),
    #[error(transparent)]
    Hash(#[from] CanonicalHashError),
}

/// Builds a sealed Windows patch candidate from an untrusted change set and
/// host-observed preimages.
///
/// # Errors
///
/// Returns [`EditsError`] when the change set is empty, oversized, aliased,
/// misaligned with the observed preimages, a no-op, or when any candidate
/// domain invariant is violated.
pub fn build_changes_candidate(
    binding: &ChangesProposalBinding,
    changes: &[ProposedFileChange],
    preimages: &[ObservedPreimage],
) -> Result<PreparedChangesProposal, EditsError> {
    if changes.is_empty() {
        return Err(EditsError::EmptyProposal);
    }
    if changes.len() > HARD_MAX_CHANGED_FILES as usize {
        return Err(EditsError::TooManyChanges);
    }
    if changes.len() != preimages.len() {
        return Err(EditsError::PreimageMismatch);
    }

    let mut pairs: Vec<(&ProposedFileChange, &ObservedPreimage)> = Vec::new();
    for (change, preimage) in changes.iter().zip(preimages) {
        if change.relative_path() != &preimage.relative_path {
            return Err(EditsError::PreimageMismatch);
        }
        pairs.push((change, preimage));
    }
    pairs
        .sort_by(|(left, _), (right, _)| left.relative_path().canonical_cmp(right.relative_path()));
    let mut case_folded = std::collections::BTreeSet::new();
    if pairs
        .iter()
        .any(|(change, _)| !case_folded.insert(change.relative_path().case_folded()))
    {
        return Err(EditsError::DuplicatePath);
    }

    let (operations, candidate_preimages) = build_patch_operations(&pairs)?;

    let patch = PatchSet::new(operations);
    let patch_hash = patch.content_hash()?;
    let declared_writes = patch
        .operations
        .iter()
        .map(|operation| DeclaredWrite {
            path_pattern: operation.relative_path().clone(),
            operation: operation.declared_operation(),
            preimage_hash: operation.preimage_hash(),
        })
        .collect::<Vec<_>>();
    let proposal_hash = canonical_hash(
        "changes-proposal",
        1,
        &ProposalHashInput {
            proposal_id: &binding.proposal_id,
            workspace_target: &binding.workspace_target,
            changes: pairs.iter().map(|(change, _)| *change).collect(),
        },
    )?;

    let candidate = WindowsPatchCandidateDraft {
        schema_version: "sapphirus.candidate-action.v1".to_owned(),
        common: CandidateCommon {
            candidate_id: binding.candidate_id.clone(),
            project_id: binding.project_id.clone(),
            run_id: binding.run_id.clone(),
            proposal_id: binding.proposal_id.clone(),
            proposal_hash,
            authority_ref: binding.authority_ref.clone(),
            owner_scope_ref: binding.owner_scope_ref.clone(),
            policy_context_hash: binding.policy_context_hash,
            mutable_inputs: binding.mutable_inputs.clone(),
            declared_writes,
            limits: ExecutionLimits::governed_patch_defaults(),
            rollback_class: RollbackClass::FileTracked,
            created_at: binding.created_at,
            expires_at: binding.expires_at,
        },
        delivery_model: DeliveryModel::WindowsLocal,
        action_kind: "patch_apply".to_owned(),
        workspace_target: binding.workspace_target.clone(),
        executor_audience: binding.executor_audience.clone(),
        patch_ref: format!("cas://sha256/{}", patch_hash.hex_value()),
        patch_hash,
        preimages: candidate_preimages,
    }
    .seal()?;

    Ok(PreparedChangesProposal {
        patch,
        candidate,
        proposal_hash,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProposalHashInput<'a> {
    proposal_id: &'a ContractId,
    workspace_target: &'a WorkspaceTarget,
    changes: Vec<&'a ProposedFileChange>,
}

fn build_patch_operations(
    pairs: &[(&ProposedFileChange, &ObservedPreimage)],
) -> Result<(Vec<PatchOperation>, Vec<LocalPathPreimage>), EditsError> {
    let mut operations = Vec::with_capacity(pairs.len());
    let mut candidate_preimages = Vec::with_capacity(pairs.len());
    for (change, observed) in pairs {
        let operation = match change {
            ProposedFileChange::SetContent {
                relative_path,
                content,
            } => {
                if observed.exists {
                    let (Some(content_hash), Some(_), Some(_)) = (
                        observed.content_hash,
                        observed.file_identity_hash,
                        observed.metadata_hash,
                    ) else {
                        return Err(EditsError::PreimageMismatch);
                    };
                    if sha256_bytes(content.as_bytes()) == content_hash {
                        return Err(EditsError::NoOpChange);
                    }
                    PatchOperation::replace(relative_path.clone(), content_hash, content.clone())
                } else {
                    if observed.content_hash.is_some()
                        || observed.file_identity_hash.is_some()
                        || observed.metadata_hash.is_some()
                    {
                        return Err(EditsError::PreimageMismatch);
                    }
                    PatchOperation::create(relative_path.clone(), content.clone())
                }
            }
            ProposedFileChange::Delete { relative_path } => {
                if !observed.exists {
                    return Err(EditsError::MissingTarget);
                }
                let (Some(content_hash), Some(_), Some(_)) = (
                    observed.content_hash,
                    observed.file_identity_hash,
                    observed.metadata_hash,
                ) else {
                    return Err(EditsError::PreimageMismatch);
                };
                PatchOperation::delete(relative_path.clone(), content_hash)
            }
        };
        candidate_preimages.push(LocalPathPreimage {
            relative_path: observed.relative_path.clone(),
            exists: observed.exists,
            file_identity_hash: observed.file_identity_hash,
            content_hash: observed.content_hash,
            metadata_hash: observed.metadata_hash,
        });
        operations.push(operation);
    }
    Ok((operations, candidate_preimages))
}

/// Builds the exact review projection for a prepared proposal.
///
/// # Errors
///
/// Returns [`EditsError::PreimageMismatch`] when the preimages do not cover
/// the patch operations exactly.
pub fn build_changes_review(
    prepared: &PreparedChangesProposal,
    preimages: &[ObservedPreimage],
    workspace_id: &str,
    workspace_grant_epoch: u64,
    proposal_kind: ChangesProposalKind,
    source_execution_id: Option<ContractId>,
) -> Result<ChangesReviewProjection, EditsError> {
    let mut files = Vec::with_capacity(prepared.patch.operations.len());
    for operation in &prepared.patch.operations {
        let observed = preimages
            .iter()
            .find(|preimage| &preimage.relative_path == operation.relative_path())
            .ok_or(EditsError::PreimageMismatch)?;
        let before_bytes = observed
            .content
            .as_ref()
            .map_or(0, |content| content.len() as u64);
        let after_bytes = operation
            .content()
            .map_or(0, |content| content.len() as u64);
        files.push(FileChangeReview {
            relative_path: operation.relative_path().clone(),
            operation: operation.declared_operation(),
            before_content: observed.content.clone(),
            after_content: operation.content().map(str::to_owned),
            before_hash: observed.content_hash,
            after_hash: operation.postimage_hash(),
            before_bytes,
            after_bytes,
        });
    }
    Ok(ChangesReviewProjection {
        schema_version: CHANGES_REVIEW_SCHEMA.to_owned(),
        proposal_id: prepared.candidate.draft.common.proposal_id.clone(),
        candidate_id: prepared.candidate.draft.common.candidate_id.clone(),
        candidate_hash: prepared.candidate.candidate_hash,
        workspace_id: workspace_id.to_owned(),
        workspace_grant_epoch,
        proposal_kind,
        source_execution_id,
        files,
        total_changed_bytes: prepared.patch.changed_bytes(),
        created_at: prepared.candidate.draft.common.created_at,
        expires_at: prepared.candidate.draft.common.expires_at,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_changes_candidate, build_changes_review, ChangesProposalBinding, ChangesProposalKind,
        EditsError, ObservedPreimage, ProposedFileChange,
    };
    use crate::{
        canonical_hash, sha256_bytes, AuthorityRef, ContractId, DeclaredWriteOperation, InputKind,
        MutableInputBinding, NativePatchEngineAudience, RelativeWorkspacePath, UnixMillis,
        WorkspaceTarget,
    };

    fn id(value: &str) -> Result<ContractId, Box<dyn std::error::Error>> {
        Ok(ContractId::new(value)?)
    }

    fn path(value: &str) -> Result<RelativeWorkspacePath, Box<dyn std::error::Error>> {
        Ok(RelativeWorkspacePath::new(value)?)
    }

    fn binding() -> Result<ChangesProposalBinding, Box<dyn std::error::Error>> {
        Ok(ChangesProposalBinding {
            proposal_id: id("proposal_1")?,
            candidate_id: id("candidate_1")?,
            project_id: id("project_1")?,
            run_id: id("run_1")?,
            owner_scope_ref: id("owner_1")?,
            authority_ref: AuthorityRef {
                authority_kind: "desktop_local_store".to_owned(),
                authority_id: id("authority_1")?,
                installation_id: id("install_1")?,
                local_store_id: id("store_1")?,
                authority_epoch: 1,
            },
            policy_context_hash: sha256_bytes(b"policy-context"),
            workspace_target: WorkspaceTarget {
                target_kind: "local_folder_capability".to_owned(),
                workspace_capability_id: id("workspace_1")?,
                grant_epoch: 2,
                root_identity_hash: sha256_bytes(b"root"),
                filesystem_capability_hash: sha256_bytes(b"filesystem"),
                base_checkpoint_id: id("checkpoint_genesis")?,
                workspace_manifest_hash: sha256_bytes(b"manifest"),
            },
            executor_audience: NativePatchEngineAudience {
                audience_kind: "native_patch_engine".to_owned(),
                installation_id: id("install_1")?,
                host_build_id: "desktop-test".to_owned(),
                host_binary_sha256: sha256_bytes(b"binary"),
                patch_engine_profile_hash: sha256_bytes(b"profile"),
            },
            mutable_inputs: vec![MutableInputBinding {
                input_kind: InputKind::WorkspaceManifest,
                input_id: "manifest_1".to_owned(),
                content_hash: sha256_bytes(b"manifest"),
            }],
            created_at: UnixMillis(1_000),
            expires_at: UnixMillis(600_000),
        })
    }

    fn existing_preimage(
        value: &str,
        content: &str,
    ) -> Result<ObservedPreimage, Box<dyn std::error::Error>> {
        Ok(ObservedPreimage {
            relative_path: path(value)?,
            exists: true,
            content: Some(content.to_owned()),
            content_hash: Some(sha256_bytes(content.as_bytes())),
            file_identity_hash: Some(sha256_bytes(format!("identity:{value}").as_bytes())),
            metadata_hash: Some(sha256_bytes(format!("metadata:{value}").as_bytes())),
        })
    }

    fn absent_preimage(value: &str) -> Result<ObservedPreimage, Box<dyn std::error::Error>> {
        Ok(ObservedPreimage {
            relative_path: path(value)?,
            exists: false,
            content: None,
            content_hash: None,
            file_identity_hash: None,
            metadata_hash: None,
        })
    }

    #[test]
    fn builds_a_sealed_candidate_and_bound_review() -> Result<(), Box<dyn std::error::Error>> {
        let changes = vec![
            ProposedFileChange::SetContent {
                relative_path: path("src/new.rs")?,
                content: "pub fn created() {}\n".to_owned(),
            },
            ProposedFileChange::SetContent {
                relative_path: path("main.rs")?,
                content: "fn main() { updated(); }\n".to_owned(),
            },
            ProposedFileChange::Delete {
                relative_path: path("old.txt")?,
            },
        ];
        let preimages = vec![
            absent_preimage("src/new.rs")?,
            existing_preimage("main.rs", "fn main() {}\n")?,
            existing_preimage("old.txt", "legacy\n")?,
        ];
        let prepared = build_changes_candidate(&binding()?, &changes, &preimages)?;
        prepared.candidate.verify()?;
        assert_eq!(prepared.patch.operations.len(), 3);
        // Canonical ordering sorts by UTF-16 path order regardless of input order.
        assert_eq!(
            prepared.patch.operations[0].relative_path().as_str(),
            "main.rs"
        );

        let review = build_changes_review(
            &prepared,
            &preimages,
            "workspace_1",
            2,
            ChangesProposalKind::Edit,
            None,
        )?;
        assert_eq!(review.files.len(), 3);
        assert_eq!(review.files[0].operation, DeclaredWriteOperation::Modify);
        assert_eq!(
            review.files[0].before_content.as_deref(),
            Some("fn main() {}\n")
        );
        assert_eq!(
            review.files[0].after_content.as_deref(),
            Some("fn main() { updated(); }\n")
        );
        let first_hash = review.displayed_diff_hash()?;
        let mut tampered = review.clone();
        tampered.files[0].after_content = Some("fn main() { evil(); }\n".to_owned());
        assert_ne!(first_hash, tampered.displayed_diff_hash()?);
        Ok(())
    }

    #[test]
    fn rejects_empty_noop_duplicate_and_missing_targets() -> Result<(), Box<dyn std::error::Error>>
    {
        let bound = binding()?;
        assert!(matches!(
            build_changes_candidate(&bound, &[], &[]),
            Err(EditsError::EmptyProposal)
        ));

        let noop = vec![ProposedFileChange::SetContent {
            relative_path: path("main.rs")?,
            content: "fn main() {}\n".to_owned(),
        }];
        let noop_preimages = vec![existing_preimage("main.rs", "fn main() {}\n")?];
        assert!(matches!(
            build_changes_candidate(&bound, &noop, &noop_preimages),
            Err(EditsError::NoOpChange)
        ));

        let duplicated = vec![
            ProposedFileChange::SetContent {
                relative_path: path("Main.rs")?,
                content: "a".to_owned(),
            },
            ProposedFileChange::SetContent {
                relative_path: path("main.rs")?,
                content: "b".to_owned(),
            },
        ];
        let duplicate_preimages = vec![absent_preimage("Main.rs")?, absent_preimage("main.rs")?];
        assert!(matches!(
            build_changes_candidate(&bound, &duplicated, &duplicate_preimages),
            Err(EditsError::DuplicatePath)
        ));

        let missing_delete = vec![ProposedFileChange::Delete {
            relative_path: path("ghost.txt")?,
        }];
        let missing_preimages = vec![absent_preimage("ghost.txt")?];
        assert!(matches!(
            build_changes_candidate(&bound, &missing_delete, &missing_preimages),
            Err(EditsError::MissingTarget)
        ));
        Ok(())
    }

    #[test]
    fn rejects_misaligned_preimages() -> Result<(), Box<dyn std::error::Error>> {
        let bound = binding()?;
        let changes = vec![ProposedFileChange::SetContent {
            relative_path: path("main.rs")?,
            content: "updated".to_owned(),
        }];
        let wrong_path = vec![existing_preimage("other.rs", "content")?];
        assert!(matches!(
            build_changes_candidate(&bound, &changes, &wrong_path),
            Err(EditsError::PreimageMismatch)
        ));
        assert!(matches!(
            build_changes_candidate(&bound, &changes, &[]),
            Err(EditsError::PreimageMismatch)
        ));
        Ok(())
    }

    #[test]
    fn proposal_hash_binds_change_content() -> Result<(), Box<dyn std::error::Error>> {
        let bound = binding()?;
        let preimages = vec![absent_preimage("new.txt")?];
        let first = build_changes_candidate(
            &bound,
            &[ProposedFileChange::SetContent {
                relative_path: path("new.txt")?,
                content: "one".to_owned(),
            }],
            &preimages,
        )?;
        let second = build_changes_candidate(
            &bound,
            &[ProposedFileChange::SetContent {
                relative_path: path("new.txt")?,
                content: "two".to_owned(),
            }],
            &preimages,
        )?;
        assert_ne!(first.proposal_hash, second.proposal_hash);
        assert_ne!(
            first.candidate.candidate_hash,
            second.candidate.candidate_hash
        );
        // The proposal hash lives in the canonical candidate purpose domain.
        let _ = canonical_hash("changes-proposal", 1, &serde_json::json!({}))?;
        Ok(())
    }
}
