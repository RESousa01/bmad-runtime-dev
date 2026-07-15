use std::collections::{BTreeMap, BTreeSet};

use desktop_runtime::{
    canonical_hash, canonical_json_bytes, BuilderAnalysisContextDecision,
    BuilderAnalysisDecisionConsumption, BuilderAnalysisDecisionInvalidation,
    BuilderAnalysisDecisionInvalidationReason, BuilderAnalysisKind, BuilderAnalysisRun,
    BuilderDraft, BuilderDraftRepository, BuilderDraftRevision, BuilderDraftScope,
    BuilderDraftState, BuilderPersistenceEvent, ContractId,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::{
    append_evidence_in_transaction, canonical_now, is_unique_violation, EvidenceAppend, LocalStore,
    PayloadRef, StoreError,
};

const BUILDER_STATE_KIND: &str = "bmad_builder_draft";
const BUILDER_STATE_SCHEMA: &str = "sapphirus.bmad-builder-draft-state.v1";
const BUILDER_REVISION_KIND: &str = "bmad_builder_revision";
const BUILDER_REVISION_SCHEMA: &str = "sapphirus.bmad-builder-revision.v1";
const BUILDER_ANALYSIS_KIND: &str = "bmad_builder_analysis";
const BUILDER_ANALYSIS_SCHEMA: &str = "sapphirus.bmad-builder-analysis.v1";
const BUILDER_ANALYSIS_DECISION_KIND: &str = "bmad_builder_analysis_decision";
const BUILDER_ANALYSIS_DECISION_SCHEMA: &str =
    "sapphirus.bmad-builder-analysis-context-decision.v1";

#[derive(Debug)]
struct BuilderStateRef {
    version: u64,
    state: String,
    payload: PayloadRef,
}

#[derive(Debug)]
struct BuilderDraftIntegrityRow {
    draft_id: String,
    owner_scope_ref: String,
    project_id: String,
    authoring_session_id: String,
    authority_id: String,
    version: u64,
    state: String,
    payload: PayloadRef,
}

#[derive(Debug)]
struct BuilderRevisionIntegrityRow {
    draft_id: String,
    revision_id: String,
    ordinal: u64,
    revision_hash: String,
    source_inventory_hash: String,
    host_inventory_hash: String,
    payload: PayloadRef,
}

#[derive(Debug)]
struct BuilderAnalysisIntegrityRow {
    draft_id: String,
    analysis_id: String,
    revision_id: String,
    revision_hash: String,
    analysis_kind: String,
    context_decision_id: Option<String>,
    invocation_id: Option<String>,
    decision_consumption_hash: Option<String>,
    payload: PayloadRef,
}

#[derive(Debug)]
struct BuilderDecisionIntegrityRow {
    draft_id: String,
    decision_id: String,
    revision_id: String,
    revision_hash: String,
    scope_hash: String,
    invocation_id: String,
    decision_hash: String,
    disposition: String,
    consumed_analysis_id: Option<String>,
    consumption_id: Option<String>,
    consumption_hash: Option<String>,
    consumed_at: Option<String>,
    invalidation_reason: Option<String>,
    invalidation_version: Option<u64>,
    invalidation_hash: Option<String>,
    invalidated_at: Option<String>,
    payload: PayloadRef,
}

impl BuilderDraftRepository for LocalStore {
    type Error = StoreError;

    fn create_builder_draft(&self, draft: &BuilderDraft) -> Result<(), Self::Error> {
        let scope = draft.scope().ok_or(StoreError::Inconsistent)?;
        if draft.version() != 1
            || draft.state() != BuilderDraftState::Drafting
            || draft.current_revision().is_some()
            || !draft.analyses().is_empty()
            || draft.pending_analysis_decision().is_some()
            || !draft.analysis_consumptions().is_empty()
            || !draft.analysis_decision_invalidations().is_empty()
        {
            return Err(StoreError::StateConflict);
        }
        let state_json = draft.to_persisted_json()?;
        let payload = self.put_payload(
            BUILDER_STATE_KIND,
            BUILDER_STATE_SCHEMA,
            state_json.as_bytes(),
        )?;
        let occurred_at = canonical_now();
        let record = draft.record();
        let event = builder_event(
            record.draft_id.as_str(),
            "bmad.builder.draft_created",
            &payload,
            None,
        )?;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = transaction.execute(
            "INSERT INTO bmad_builder_drafts
             (draft_id, owner_scope_ref, project_id, authoring_session_id, authority_id,
              version, state, state_content_hash, state_kind, state_schema_version,
              state_byte_count, state_key_version, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                record.draft_id.as_str(),
                scope.owner_scope_ref.as_str(),
                scope.project_id.as_str(),
                scope.authoring_session_id.as_str(),
                scope.authority_ref.authority_id.as_str(),
                draft.version(),
                builder_state_name(draft.state()),
                payload.content_hash,
                payload.kind,
                payload.schema_version,
                payload.byte_count,
                payload.key_version,
                occurred_at,
            ],
        );
        match inserted {
            Ok(1) => {}
            Ok(_) => return Err(StoreError::Inconsistent),
            Err(error) if is_unique_violation(&error) => return Err(StoreError::StateConflict),
            Err(error) => return Err(StoreError::Sqlite(error)),
        }
        let _ = append_evidence_in_transaction(&transaction, &event, &occurred_at)?;
        transaction.commit()?;
        Ok(())
    }

    fn load_builder_draft(
        &self,
        scope: &BuilderDraftScope,
        draft_id: &ContractId,
    ) -> Result<Option<BuilderDraft>, Self::Error> {
        let reference = {
            let connection = self.connection.lock();
            connection
                .query_row(
                    "SELECT version, state, state_content_hash, state_kind,
                            state_schema_version, state_byte_count, state_key_version
                     FROM bmad_builder_drafts
                     WHERE draft_id = ?1 AND owner_scope_ref = ?2 AND project_id = ?3
                       AND authoring_session_id = ?4 AND authority_id = ?5",
                    params![
                        draft_id.as_str(),
                        scope.owner_scope_ref.as_str(),
                        scope.project_id.as_str(),
                        scope.authoring_session_id.as_str(),
                        scope.authority_ref.authority_id.as_str(),
                    ],
                    |row| {
                        Ok(BuilderStateRef {
                            version: row.get(0)?,
                            state: row.get(1)?,
                            payload: PayloadRef {
                                content_hash: row.get(2)?,
                                kind: row.get(3)?,
                                schema_version: row.get(4)?,
                                byte_count: row.get(5)?,
                                key_version: row.get(6)?,
                            },
                        })
                    },
                )
                .optional()?
        };
        let Some(reference) = reference else {
            return Ok(None);
        };
        let bytes = self.get_payload(&reference.payload)?;
        let source = std::str::from_utf8(&bytes).map_err(|_| StoreError::Inconsistent)?;
        let draft = BuilderDraft::from_persisted_json(source)?;
        if draft.version() != reference.version
            || builder_state_name(draft.state()) != reference.state
            || draft.record().draft_id != *draft_id
            || draft.scope().as_ref() != Some(scope)
        {
            return Err(StoreError::Inconsistent);
        }
        self.verify_builder_history_for_draft(&draft)?;
        Ok(Some(draft))
    }

    fn persist_builder_transition(
        &self,
        draft: &BuilderDraft,
        expected_previous_version: u64,
        event_kind: BuilderPersistenceEvent,
    ) -> Result<(), Self::Error> {
        let scope = draft.scope().ok_or(StoreError::Inconsistent)?;
        if draft.version() != expected_previous_version.saturating_add(1)
            || !builder_event_matches_state(event_kind, draft.state())
        {
            return Err(StoreError::StateConflict);
        }
        let state_json = draft.to_persisted_json()?;
        let state_payload = self.put_payload(
            BUILDER_STATE_KIND,
            BUILDER_STATE_SCHEMA,
            state_json.as_bytes(),
        )?;
        let history_payload = prepare_history_payload(self, draft, event_kind)?;
        let occurred_at = canonical_now();
        let causation_id = match &history_payload {
            Some((_, HistoryRecord::Revision(value))) => Some(value.revision_id.to_string()),
            Some((_, HistoryRecord::Analysis(value))) => Some(value.analysis_id.to_string()),
            Some((_, HistoryRecord::Decision(value))) => Some(value.decision_id.to_string()),
            None => None,
        };
        let event = builder_event(
            draft.record().draft_id.as_str(),
            event_kind.event_type(),
            &state_payload,
            causation_id,
        )?;
        let mut connection = self.connection.lock();
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = transaction.execute(
            "UPDATE bmad_builder_drafts
             SET version = ?1, state = ?2, state_content_hash = ?3, state_kind = ?4,
                 state_schema_version = ?5, state_byte_count = ?6, state_key_version = ?7,
                 updated_at = ?8
             WHERE draft_id = ?9 AND owner_scope_ref = ?10 AND project_id = ?11
               AND authoring_session_id = ?12 AND authority_id = ?13 AND version = ?14",
            params![
                draft.version(),
                builder_state_name(draft.state()),
                state_payload.content_hash,
                state_payload.kind,
                state_payload.schema_version,
                state_payload.byte_count,
                state_payload.key_version,
                occurred_at,
                draft.record().draft_id.as_str(),
                scope.owner_scope_ref.as_str(),
                scope.project_id.as_str(),
                scope.authoring_session_id.as_str(),
                scope.authority_ref.authority_id.as_str(),
                expected_previous_version,
            ],
        )?;
        if updated != 1 {
            return Err(StoreError::StateConflict);
        }
        if let Some(invalidation) = draft
            .analysis_decision_invalidations()
            .iter()
            .find(|value| value.aggregate_version == draft.version())
        {
            invalidate_analysis_decision(&transaction, draft, invalidation, &occurred_at)?;
        }
        if let Some((payload, history)) = history_payload {
            insert_history(&transaction, draft, &payload, history, &occurred_at)?;
        }
        let _ = append_evidence_in_transaction(&transaction, &event, &occurred_at)?;
        transaction.commit()?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum HistoryRecord<'a> {
    Revision(&'a BuilderDraftRevision),
    Analysis(&'a BuilderAnalysisRun),
    Decision(&'a BuilderAnalysisContextDecision),
}

fn prepare_history_payload<'a>(
    store: &LocalStore,
    draft: &'a BuilderDraft,
    event: BuilderPersistenceEvent,
) -> Result<Option<(PayloadRef, HistoryRecord<'a>)>, StoreError> {
    let (kind, schema, bytes, history) = match event {
        BuilderPersistenceEvent::RevisionAppended => {
            let revision = draft.current_revision().ok_or(StoreError::Inconsistent)?;
            (
                BUILDER_REVISION_KIND,
                BUILDER_REVISION_SCHEMA,
                canonical_json_bytes(revision).map_err(|_| StoreError::Inconsistent)?,
                HistoryRecord::Revision(revision),
            )
        }
        BuilderPersistenceEvent::AnalysisRecorded => {
            let analysis = draft.analyses().last().ok_or(StoreError::Inconsistent)?;
            (
                BUILDER_ANALYSIS_KIND,
                BUILDER_ANALYSIS_SCHEMA,
                canonical_json_bytes(analysis).map_err(|_| StoreError::Inconsistent)?,
                HistoryRecord::Analysis(analysis),
            )
        }
        BuilderPersistenceEvent::AnalysisDecisionIssued => {
            let decision = draft
                .pending_analysis_decision()
                .ok_or(StoreError::Inconsistent)?;
            (
                BUILDER_ANALYSIS_DECISION_KIND,
                BUILDER_ANALYSIS_DECISION_SCHEMA,
                canonical_json_bytes(decision).map_err(|_| StoreError::Inconsistent)?,
                HistoryRecord::Decision(decision),
            )
        }
        _ => return Ok(None),
    };
    Ok(Some((store.put_payload(kind, schema, &bytes)?, history)))
}

fn insert_history(
    transaction: &rusqlite::Transaction<'_>,
    draft: &BuilderDraft,
    payload: &PayloadRef,
    history: HistoryRecord<'_>,
    occurred_at: &str,
) -> Result<(), StoreError> {
    let inserted = match history {
        HistoryRecord::Revision(revision) => transaction.execute(
            "INSERT INTO bmad_builder_revisions
             (revision_id, draft_id, ordinal, revision_hash, source_inventory_hash,
              host_inventory_hash, content_hash, content_kind, content_schema_version, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                revision.revision_id.as_str(),
                draft.record().draft_id.as_str(),
                revision.ordinal,
                revision.revision_hash.to_string(),
                revision.inventory_hash.to_string(),
                revision.host_inventory_hash()?.to_string(),
                payload.content_hash,
                payload.kind,
                payload.schema_version,
                occurred_at,
            ],
        ),
        HistoryRecord::Analysis(analysis) => {
            let binding = analysis.model_binding();
            let inserted = transaction.execute(
                "INSERT INTO bmad_builder_analyses
                 (analysis_id, draft_id, revision_id, revision_hash, analysis_kind,
                  context_decision_id, invocation_id, decision_consumption_hash,
                  content_hash, content_kind, content_schema_version, recorded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    analysis.analysis_id.as_str(),
                    draft.record().draft_id.as_str(),
                    analysis.revision_id.as_str(),
                    analysis.revision_hash.to_string(),
                    analysis_kind_name(analysis.analysis_kind),
                    binding.map(|value| value.context_decision_id.as_str()),
                    binding.map(|value| value.invocation_id.as_str()),
                    binding.map(|value| value.context_decision_consumption_hash.to_string()),
                    payload.content_hash,
                    payload.kind,
                    payload.schema_version,
                    occurred_at,
                ],
            );
            match inserted {
                Ok(1) => {
                    if let Some(binding) = binding {
                        let consumption = draft
                            .analysis_consumptions()
                            .iter()
                            .find(|value| value.analysis_id == analysis.analysis_id)
                            .ok_or(StoreError::Inconsistent)?;
                        consume_analysis_decision(
                            transaction,
                            draft,
                            analysis,
                            binding.context_decision_id.as_str(),
                            consumption,
                        )?;
                    }
                    Ok(1)
                }
                other => other,
            }
        }
        HistoryRecord::Decision(decision) => transaction.execute(
            "INSERT INTO bmad_builder_analysis_decisions
             (decision_id, draft_id, revision_id, revision_hash, scope_hash,
              invocation_id, decision_hash, disposition, content_hash, content_kind,
              content_schema_version, recorded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, ?9, ?10, ?11)",
            params![
                decision.decision_id.as_str(),
                draft.record().draft_id.as_str(),
                decision.revision_id.as_str(),
                decision.revision_hash.to_string(),
                decision.scope_hash.to_string(),
                decision.invocation_id.as_str(),
                decision.decision_hash.to_string(),
                payload.content_hash,
                payload.kind,
                payload.schema_version,
                occurred_at,
            ],
        ),
    };
    match inserted {
        Ok(1) => Ok(()),
        Ok(_) => Err(StoreError::Inconsistent),
        Err(error) if is_unique_violation(&error) => Err(StoreError::StateConflict),
        Err(error) => Err(StoreError::Sqlite(error)),
    }
}

fn consume_analysis_decision(
    transaction: &rusqlite::Transaction<'_>,
    draft: &BuilderDraft,
    analysis: &BuilderAnalysisRun,
    decision_id: &str,
    consumption: &BuilderAnalysisDecisionConsumption,
) -> Result<(), StoreError> {
    consumption.validate_integrity()?;
    let updated = transaction.execute(
        "UPDATE bmad_builder_analysis_decisions
         SET disposition = 'consumed', consumed_analysis_id = ?1,
             consumption_id = ?2, consumption_hash = ?3,
             consumed_at = ?4
         WHERE decision_id = ?5 AND draft_id = ?6 AND revision_id = ?7
           AND revision_hash = ?8 AND invocation_id = ?9 AND decision_hash = ?10
           AND disposition = 'pending'
           AND consumed_analysis_id IS NULL AND consumption_id IS NULL
           AND consumption_hash IS NULL AND consumed_at IS NULL",
        params![
            analysis.analysis_id.as_str(),
            consumption.consumption_id.as_str(),
            consumption.consumption_hash.to_string(),
            consumption.consumed_at.as_str(),
            decision_id,
            draft.record().draft_id.as_str(),
            analysis.revision_id.as_str(),
            analysis.revision_hash.to_string(),
            consumption.invocation_id.as_str(),
            consumption.decision_hash.to_string(),
        ],
    )?;
    if updated == 1 {
        Ok(())
    } else {
        Err(StoreError::StateConflict)
    }
}

fn invalidate_analysis_decision(
    transaction: &rusqlite::Transaction<'_>,
    draft: &BuilderDraft,
    invalidation: &BuilderAnalysisDecisionInvalidation,
    occurred_at: &str,
) -> Result<(), StoreError> {
    invalidation.validate_integrity()?;
    let updated = transaction.execute(
        "UPDATE bmad_builder_analysis_decisions
         SET disposition = 'invalidated', invalidation_reason = ?1,
             invalidation_version = ?2, invalidation_hash = ?3, invalidated_at = ?4
         WHERE decision_id = ?5 AND draft_id = ?6 AND revision_id = ?7
           AND decision_hash = ?8 AND disposition = 'pending'
           AND consumed_analysis_id IS NULL AND consumption_id IS NULL
           AND consumption_hash IS NULL AND consumed_at IS NULL
           AND invalidation_reason IS NULL AND invalidation_version IS NULL
           AND invalidation_hash IS NULL AND invalidated_at IS NULL",
        params![
            invalidation_reason_name(invalidation.reason),
            invalidation.aggregate_version,
            invalidation.invalidation_hash.to_string(),
            occurred_at,
            invalidation.decision_id.as_str(),
            draft.record().draft_id.as_str(),
            invalidation.revision_id.as_str(),
            invalidation.decision_hash.to_string(),
        ],
    )?;
    if updated == 1 {
        Ok(())
    } else {
        Err(StoreError::StateConflict)
    }
}

impl LocalStore {
    fn verify_builder_history_for_draft(&self, draft: &BuilderDraft) -> Result<(), StoreError> {
        let draft_id = draft.record().draft_id.as_str();
        let (revision_rows, analysis_rows, decision_rows) = {
            let connection = self.connection.lock();
            let revisions = load_revision_integrity_rows(&connection)?
                .into_iter()
                .filter(|row| row.draft_id == draft_id)
                .collect();
            let analyses = load_analysis_integrity_rows(&connection)?
                .into_iter()
                .filter(|row| row.draft_id == draft_id)
                .collect();
            let decisions = load_decision_integrity_rows(&connection)?
                .into_iter()
                .filter(|row| row.draft_id == draft_id)
                .collect();
            (revisions, analyses, decisions)
        };
        let drafts = BTreeMap::from([(draft_id.to_owned(), draft.clone())]);
        self.verify_builder_revisions(revision_rows, &drafts)?;
        self.verify_builder_analyses(analysis_rows, &drafts)?;
        self.verify_builder_decisions(decision_rows, &drafts)
    }

    pub(crate) fn verify_builder_integrity(&self) -> Result<(), StoreError> {
        let (draft_rows, revision_rows, analysis_rows, decision_rows) = {
            let connection = self.connection.lock();
            (
                load_draft_integrity_rows(&connection)?,
                load_revision_integrity_rows(&connection)?,
                load_analysis_integrity_rows(&connection)?,
                load_decision_integrity_rows(&connection)?,
            )
        };
        let drafts = self.verify_builder_drafts(draft_rows)?;
        self.verify_builder_revisions(revision_rows, &drafts)?;
        self.verify_builder_analyses(analysis_rows, &drafts)?;
        self.verify_builder_decisions(decision_rows, &drafts)
    }

    fn verify_builder_drafts(
        &self,
        rows: Vec<BuilderDraftIntegrityRow>,
    ) -> Result<BTreeMap<String, BuilderDraft>, StoreError> {
        let mut drafts = BTreeMap::new();
        for row in rows {
            let bytes = self.get_payload(&row.payload)?;
            let source = std::str::from_utf8(&bytes).map_err(|_| StoreError::Inconsistent)?;
            let draft = BuilderDraft::from_persisted_json(source)?;
            let scope = draft.scope().ok_or(StoreError::Inconsistent)?;
            if draft.record().draft_id.as_str() != row.draft_id
                || scope.owner_scope_ref.as_str() != row.owner_scope_ref
                || scope.project_id.as_str() != row.project_id
                || scope.authoring_session_id.as_str() != row.authoring_session_id
                || scope.authority_ref.authority_id.as_str() != row.authority_id
                || draft.version() != row.version
                || builder_state_name(draft.state()) != row.state
                || drafts.insert(row.draft_id, draft).is_some()
            {
                return Err(StoreError::Inconsistent);
            }
        }
        Ok(drafts)
    }

    fn verify_builder_revisions(
        &self,
        rows: Vec<BuilderRevisionIntegrityRow>,
        drafts: &BTreeMap<String, BuilderDraft>,
    ) -> Result<(), StoreError> {
        let mut seen = BTreeSet::new();
        for row in rows {
            let bytes = self.get_payload(&row.payload)?;
            let revision: BuilderDraftRevision = serde_json::from_slice(&bytes)?;
            let draft = drafts.get(&row.draft_id).ok_or(StoreError::Inconsistent)?;
            if canonical_json_bytes(&revision).map_err(|_| StoreError::Inconsistent)? != bytes
                || revision.revision_id.as_str() != row.revision_id
                || revision.ordinal != row.ordinal
                || revision.revision_hash.to_string() != row.revision_hash
                || revision.inventory_hash.to_string() != row.source_inventory_hash
                || revision.host_inventory_hash()?.to_string() != row.host_inventory_hash
                || draft.revisions().get(
                    usize::try_from(row.ordinal.saturating_sub(1))
                        .map_err(|_| StoreError::Inconsistent)?,
                ) != Some(&revision)
                || !seen.insert((row.draft_id, row.ordinal))
            {
                return Err(StoreError::Inconsistent);
            }
        }
        if drafts
            .values()
            .map(|draft| draft.revisions().len())
            .sum::<usize>()
            != seen.len()
        {
            return Err(StoreError::Inconsistent);
        }
        Ok(())
    }

    fn verify_builder_analyses(
        &self,
        rows: Vec<BuilderAnalysisIntegrityRow>,
        drafts: &BTreeMap<String, BuilderDraft>,
    ) -> Result<(), StoreError> {
        let mut seen = BTreeSet::new();
        for row in rows {
            let bytes = self.get_payload(&row.payload)?;
            let analysis: BuilderAnalysisRun = serde_json::from_slice(&bytes)?;
            let draft = drafts.get(&row.draft_id).ok_or(StoreError::Inconsistent)?;
            let binding = analysis.model_binding();
            if canonical_json_bytes(&analysis).map_err(|_| StoreError::Inconsistent)? != bytes
                || analysis.analysis_id.as_str() != row.analysis_id
                || analysis.revision_id.as_str() != row.revision_id
                || analysis.revision_hash.to_string() != row.revision_hash
                || analysis_kind_name(analysis.analysis_kind) != row.analysis_kind
                || binding.map(|value| value.context_decision_id.as_str())
                    != row.context_decision_id.as_deref()
                || binding.map(|value| value.invocation_id.as_str()) != row.invocation_id.as_deref()
                || binding.map(|value| value.context_decision_consumption_hash.to_string())
                    != row.decision_consumption_hash
                || !draft.analyses().contains(&analysis)
                || !seen.insert((row.draft_id, row.analysis_id))
            {
                return Err(StoreError::Inconsistent);
            }
        }
        if drafts
            .values()
            .map(|draft| draft.analyses().len())
            .sum::<usize>()
            != seen.len()
        {
            return Err(StoreError::Inconsistent);
        }
        Ok(())
    }

    fn verify_builder_decisions(
        &self,
        rows: Vec<BuilderDecisionIntegrityRow>,
        drafts: &BTreeMap<String, BuilderDraft>,
    ) -> Result<(), StoreError> {
        let mut seen = BTreeSet::new();
        for row in rows {
            let bytes = self.get_payload(&row.payload)?;
            let decision: BuilderAnalysisContextDecision = serde_json::from_slice(&bytes)?;
            decision.validate_integrity()?;
            let draft = drafts.get(&row.draft_id).ok_or(StoreError::Inconsistent)?;
            let pending_matches = row.disposition == "pending"
                && row.consumed_analysis_id.is_none()
                && row.consumption_id.is_none()
                && row.consumption_hash.is_none()
                && row.consumed_at.is_none()
                && row.invalidation_reason.is_none()
                && row.invalidation_version.is_none()
                && row.invalidation_hash.is_none()
                && row.invalidated_at.is_none()
                && draft.pending_analysis_decision() == Some(&decision);
            let consumption = draft
                .analysis_consumptions()
                .iter()
                .find(|value| value.decision_id == decision.decision_id);
            let consumed_matches = consumption.is_some_and(|value| {
                row.disposition == "consumed"
                    && row.consumed_analysis_id.as_deref() == Some(value.analysis_id.as_str())
                    && row.consumption_id.as_deref() == Some(value.consumption_id.as_str())
                    && row.consumption_hash.as_deref()
                        == Some(value.consumption_hash.to_string().as_str())
                    && row.consumed_at.as_deref() == Some(value.consumed_at.as_str())
                    && row.invalidation_reason.is_none()
                    && row.invalidation_version.is_none()
                    && row.invalidation_hash.is_none()
                    && row.invalidated_at.is_none()
                    && value.decision_hash == decision.decision_hash
            });
            let invalidation = draft
                .analysis_decision_invalidations()
                .iter()
                .find(|value| value.decision_id == decision.decision_id);
            let invalidated_matches = invalidation.is_some_and(|value| {
                row.disposition == "invalidated"
                    && row.consumed_analysis_id.is_none()
                    && row.consumption_id.is_none()
                    && row.consumption_hash.is_none()
                    && row.consumed_at.is_none()
                    && row.invalidation_reason.as_deref()
                        == Some(invalidation_reason_name(value.reason))
                    && row.invalidation_version == Some(value.aggregate_version)
                    && row.invalidation_hash.as_deref()
                        == Some(value.invalidation_hash.to_string().as_str())
                    && row
                        .invalidated_at
                        .as_deref()
                        .is_some_and(|time| !time.is_empty())
                    && value.decision_hash == decision.decision_hash
            });
            let scope = draft.scope().ok_or(StoreError::Inconsistent)?;
            if canonical_json_bytes(&decision).map_err(|_| StoreError::Inconsistent)? != bytes
                || decision.decision_id.as_str() != row.decision_id
                || decision.revision_id.as_str() != row.revision_id
                || decision.revision_hash.to_string() != row.revision_hash
                || decision.scope_hash.to_string() != row.scope_hash
                || decision.invocation_id.as_str() != row.invocation_id
                || decision.decision_hash.to_string() != row.decision_hash
                || decision.scope_hash
                    != canonical_hash("bmad-builder-draft-scope", 1, &scope)
                        .map_err(|_| StoreError::Inconsistent)?
                || !(pending_matches || consumed_matches || invalidated_matches)
                || !seen.insert((row.draft_id, row.decision_id))
            {
                return Err(StoreError::Inconsistent);
            }
        }
        let expected = drafts
            .values()
            .map(|draft| {
                draft.analysis_consumptions().len()
                    + usize::from(draft.pending_analysis_decision().is_some())
                    + draft.analysis_decision_invalidations().len()
            })
            .sum::<usize>();
        if expected != seen.len() {
            return Err(StoreError::Inconsistent);
        }
        Ok(())
    }
}

fn load_draft_integrity_rows(
    connection: &rusqlite::Connection,
) -> Result<Vec<BuilderDraftIntegrityRow>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT draft_id, owner_scope_ref, project_id, authoring_session_id, authority_id,
                version, state, state_content_hash, state_kind, state_schema_version,
                state_byte_count, state_key_version
         FROM bmad_builder_drafts ORDER BY draft_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(BuilderDraftIntegrityRow {
                draft_id: row.get(0)?,
                owner_scope_ref: row.get(1)?,
                project_id: row.get(2)?,
                authoring_session_id: row.get(3)?,
                authority_id: row.get(4)?,
                version: row.get(5)?,
                state: row.get(6)?,
                payload: PayloadRef {
                    content_hash: row.get(7)?,
                    kind: row.get(8)?,
                    schema_version: row.get(9)?,
                    byte_count: row.get(10)?,
                    key_version: row.get(11)?,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_revision_integrity_rows(
    connection: &rusqlite::Connection,
) -> Result<Vec<BuilderRevisionIntegrityRow>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT r.draft_id, r.revision_id, r.ordinal, r.revision_hash,
                r.source_inventory_hash, r.host_inventory_hash, r.content_hash,
                r.content_kind, r.content_schema_version, p.byte_count, p.key_version
         FROM bmad_builder_revisions r JOIN payloads p
           ON p.content_hash = r.content_hash AND p.kind = r.content_kind
          AND p.schema_version = r.content_schema_version
         ORDER BY r.draft_id, r.ordinal",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(BuilderRevisionIntegrityRow {
                draft_id: row.get(0)?,
                revision_id: row.get(1)?,
                ordinal: row.get(2)?,
                revision_hash: row.get(3)?,
                source_inventory_hash: row.get(4)?,
                host_inventory_hash: row.get(5)?,
                payload: PayloadRef {
                    content_hash: row.get(6)?,
                    kind: row.get(7)?,
                    schema_version: row.get(8)?,
                    byte_count: row.get(9)?,
                    key_version: row.get(10)?,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_analysis_integrity_rows(
    connection: &rusqlite::Connection,
) -> Result<Vec<BuilderAnalysisIntegrityRow>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT a.draft_id, a.analysis_id, a.revision_id, a.revision_hash,
                a.analysis_kind, a.context_decision_id, a.invocation_id,
                a.decision_consumption_hash, a.content_hash, a.content_kind,
                a.content_schema_version, p.byte_count, p.key_version
         FROM bmad_builder_analyses a JOIN payloads p
           ON p.content_hash = a.content_hash AND p.kind = a.content_kind
          AND p.schema_version = a.content_schema_version
         ORDER BY a.draft_id, a.analysis_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(BuilderAnalysisIntegrityRow {
                draft_id: row.get(0)?,
                analysis_id: row.get(1)?,
                revision_id: row.get(2)?,
                revision_hash: row.get(3)?,
                analysis_kind: row.get(4)?,
                context_decision_id: row.get(5)?,
                invocation_id: row.get(6)?,
                decision_consumption_hash: row.get(7)?,
                payload: PayloadRef {
                    content_hash: row.get(8)?,
                    kind: row.get(9)?,
                    schema_version: row.get(10)?,
                    byte_count: row.get(11)?,
                    key_version: row.get(12)?,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_decision_integrity_rows(
    connection: &rusqlite::Connection,
) -> Result<Vec<BuilderDecisionIntegrityRow>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT d.draft_id, d.decision_id, d.revision_id, d.revision_hash,
                d.scope_hash, d.invocation_id, d.decision_hash, d.disposition,
                d.consumed_analysis_id, d.consumption_id, d.consumption_hash,
                d.consumed_at, d.invalidation_reason, d.invalidation_version,
                d.invalidation_hash, d.invalidated_at, d.content_hash, d.content_kind,
                d.content_schema_version, p.byte_count, p.key_version
         FROM bmad_builder_analysis_decisions d JOIN payloads p
           ON p.content_hash = d.content_hash AND p.kind = d.content_kind
          AND p.schema_version = d.content_schema_version
         ORDER BY d.draft_id, d.decision_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(BuilderDecisionIntegrityRow {
                draft_id: row.get(0)?,
                decision_id: row.get(1)?,
                revision_id: row.get(2)?,
                revision_hash: row.get(3)?,
                scope_hash: row.get(4)?,
                invocation_id: row.get(5)?,
                decision_hash: row.get(6)?,
                disposition: row.get(7)?,
                consumed_analysis_id: row.get(8)?,
                consumption_id: row.get(9)?,
                consumption_hash: row.get(10)?,
                consumed_at: row.get(11)?,
                invalidation_reason: row.get(12)?,
                invalidation_version: row.get(13)?,
                invalidation_hash: row.get(14)?,
                invalidated_at: row.get(15)?,
                payload: PayloadRef {
                    content_hash: row.get(16)?,
                    kind: row.get(17)?,
                    schema_version: row.get(18)?,
                    byte_count: row.get(19)?,
                    key_version: row.get(20)?,
                },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn builder_event(
    draft_id: &str,
    event_type: &str,
    payload: &PayloadRef,
    causation_id: Option<String>,
) -> Result<EvidenceAppend, StoreError> {
    Ok(EvidenceAppend {
        stream_id: format!("bmad-builder:{draft_id}"),
        event_type: event_type.to_owned(),
        payload_hash: payload.content_hash.clone(),
        payload_ref: Some(payload_uri(payload)?),
        correlation_id: draft_id.to_owned(),
        causation_id,
        redaction_level: "summary".to_owned(),
        retention_class: "authority".to_owned(),
    })
}

fn payload_uri(payload: &PayloadRef) -> Result<String, StoreError> {
    let digest = payload
        .content_hash
        .strip_prefix("sha256:")
        .ok_or(StoreError::Inconsistent)?;
    Ok(format!("cas://sha256/{digest}"))
}

fn builder_state_name(state: BuilderDraftState) -> &'static str {
    match state {
        BuilderDraftState::Drafting => "drafting",
        BuilderDraftState::DraftReady => "draft_ready",
        BuilderDraftState::Analyzed => "analyzed",
        BuilderDraftState::UserAccepted => "user_accepted",
        BuilderDraftState::Blocked => "blocked",
        BuilderDraftState::Abandoned => "abandoned",
        BuilderDraftState::Superseded => "superseded",
    }
}

fn analysis_kind_name(kind: BuilderAnalysisKind) -> &'static str {
    match kind {
        BuilderAnalysisKind::DeterministicStatic => "deterministic_static",
        BuilderAnalysisKind::ModelLens => "model_lens",
    }
}

fn invalidation_reason_name(reason: BuilderAnalysisDecisionInvalidationReason) -> &'static str {
    match reason {
        BuilderAnalysisDecisionInvalidationReason::RevisionChanged => "revision_changed",
        BuilderAnalysisDecisionInvalidationReason::RevisionSuperseded => "revision_superseded",
        BuilderAnalysisDecisionInvalidationReason::AcceptedForReview => "accepted_for_review",
        BuilderAnalysisDecisionInvalidationReason::DraftBlocked => "draft_blocked",
        BuilderAnalysisDecisionInvalidationReason::DraftAbandoned => "draft_abandoned",
    }
}

fn builder_event_matches_state(event: BuilderPersistenceEvent, state: BuilderDraftState) -> bool {
    matches!(
        (event, state),
        (
            BuilderPersistenceEvent::RevisionAppended,
            BuilderDraftState::DraftReady
        ) | (
            BuilderPersistenceEvent::AnalysisDecisionIssued,
            BuilderDraftState::DraftReady | BuilderDraftState::Analyzed
        ) | (
            BuilderPersistenceEvent::AnalysisRecorded,
            BuilderDraftState::Analyzed
        ) | (
            BuilderPersistenceEvent::RevisionSuperseded,
            BuilderDraftState::Superseded
        ) | (
            BuilderPersistenceEvent::AcceptedForReview,
            BuilderDraftState::UserAccepted
        ) | (BuilderPersistenceEvent::Blocked, BuilderDraftState::Blocked)
            | (
                BuilderPersistenceEvent::Abandoned,
                BuilderDraftState::Abandoned
            )
    )
}
