//! FND-041: what survives a restart is a record of a choice, never a catalog.
//!
//! Two invariants are under test.
//!
//! The **snapshot** is metadata and nothing else. It has no profile row, no
//! display text, and no cache validator, so a restarted desktop can say what it
//! last saw and render it as stale, but it can neither reconstruct a catalog
//! from local state nor authorize anything with it. A catalog epoch also never
//! moves backwards, so a replayed older catalog cannot become durable truth.
//!
//! The **selections** are the user's own history, scoped per workspace and
//! bounded. When a selected profile disappears the row is demoted, not deleted:
//! the desktop must be able to say what the user had, which is what makes
//! "require an explicit choice" possible instead of a silent switch.

#![allow(
    clippy::expect_used,
    reason = "a store operation that does not hold is a broken test, and it must stop the run"
)]

use desktop_store::{
    KeyProtector, LocalStore, StoreError, StoredCatalogSnapshot, StoredModelSelection,
    MAX_RECENT_MODEL_SELECTIONS,
};

#[derive(Debug)]
struct TestProtector;

impl KeyProtector for TestProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(plaintext.to_vec())
    }

    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, StoreError> {
        Ok(protected.to_vec())
    }
}

const WORKSPACE: &str = "workspace_01J77777777777777777777777";
const OTHER_WORKSPACE: &str = "workspace_01J88888888888888888888888";
const SUBJECT: &str = "sha256:aaaa1111bbbb2222cccc3333dddd4444eeee5555ffff6666aaaa7777bbbb8888";
const OTHER_SUBJECT: &str =
    "sha256:9999888877776666555544443333222211110000aaaabbbbccccddddeeeeffff";
const GENERATED_AT: u64 = 1_785_196_800_000;

fn catalog_hash(nibble: char) -> String {
    format!("sha256:{}", String::from(nibble).repeat(64))
}

fn snapshot(
    workspace_id: &str,
    subject_hash: &str,
    epoch: u32,
    nibble: char,
) -> StoredCatalogSnapshot {
    StoredCatalogSnapshot {
        workspace_id: workspace_id.to_owned(),
        subject_hash: subject_hash.to_owned(),
        catalog_version: format!("v{epoch}.00112233445566aa"),
        catalog_hash: catalog_hash(nibble),
        catalog_state: "fresh".to_owned(),
        profile_count: 2,
        default_profile_id: Some("fixture-mid".to_owned()),
        generated_at_ms: GENERATED_AT,
        expires_at_ms: GENERATED_AT + 300_000,
        verified_at_ms: GENERATED_AT + 1_000,
    }
}

fn selection(profile_id: &str, variant_id: &str, selected_at_ms: u64) -> StoredModelSelection {
    StoredModelSelection {
        profile_id: profile_id.to_owned(),
        variant_id: variant_id.to_owned(),
        catalog_version: "v7.00112233445566aa".to_owned(),
        catalog_hash: catalog_hash('1'),
        selected_at_ms,
        is_current: true,
    }
}

#[test]
fn the_v19_migration_adds_the_model_catalog_snapshot_and_selection_tables(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    assert_eq!(store.schema_version()?, 20);

    let catalog = store.schema_catalog()?;
    let names = catalog
        .iter()
        .map(|(kind, name, _, _)| (kind.as_str(), name.as_str()))
        .collect::<Vec<_>>();
    for expected in [
        ("table", "model_catalog_snapshots"),
        ("table", "model_profile_selections"),
        ("index", "model_catalog_snapshots_by_workspace"),
        ("index", "model_profile_selections_by_workspace"),
        ("index", "model_profile_selections_one_current"),
    ] {
        assert!(names.contains(&expected), "{expected:?} missing: {names:?}");
    }
    Ok(())
}

#[test]
fn every_model_catalog_table_is_strict_and_refuses_an_unsafe_millisecond_or_state(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let connection = rusqlite::Connection::open(store.database_path())?;

    // Above the JS-safe-integer ceiling: a millisecond that cannot survive the
    // renderer boundary is not allowed to become durable.
    assert!(connection
        .execute(
            "INSERT INTO model_catalog_snapshots
             (workspace_id, subject_hash, catalog_version, catalog_hash, catalog_state,
              profile_count, default_profile_id, generated_at_ms, expires_at_ms, verified_at_ms)
             VALUES ('w', 's', 'v1.00112233445566aa', ?1, 'fresh', 1, NULL, 1, 2, 9007199254740992)",
            rusqlite::params![catalog_hash('1')],
        )
        .is_err());

    // A catalog state outside the contract's vocabulary.
    assert!(connection
        .execute(
            "INSERT INTO model_catalog_snapshots
             (workspace_id, subject_hash, catalog_version, catalog_hash, catalog_state,
              profile_count, default_profile_id, generated_at_ms, expires_at_ms, verified_at_ms)
             VALUES ('w', 's', 'v1.00112233445566aa', ?1, 'unavailable', 1, NULL, 1, 2, 3)",
            rusqlite::params![catalog_hash('1')],
        )
        .is_err());

    // A hash that is not a sha256 digest.
    assert!(connection
        .execute(
            "INSERT INTO model_catalog_snapshots
             (workspace_id, subject_hash, catalog_version, catalog_hash, catalog_state,
              profile_count, default_profile_id, generated_at_ms, expires_at_ms, verified_at_ms)
             VALUES ('w', 's', 'v1.00112233445566aa', 'md5:short', 'fresh', 1, NULL, 1, 2, 3)",
            [],
        )
        .is_err());

    // An expiry that does not follow generation.
    assert!(connection
        .execute(
            "INSERT INTO model_catalog_snapshots
             (workspace_id, subject_hash, catalog_version, catalog_hash, catalog_state,
              profile_count, default_profile_id, generated_at_ms, expires_at_ms, verified_at_ms)
             VALUES ('w', 's', 'v1.00112233445566aa', ?1, 'fresh', 1, NULL, 9, 9, 9)",
            rusqlite::params![catalog_hash('1')],
        )
        .is_err());

    // A selection flag outside the two-valued domain.
    assert!(connection
        .execute(
            "INSERT INTO model_profile_selections
             (workspace_id, profile_id, variant_id, catalog_version, catalog_hash,
              selected_at_ms, recorded_at_ms, is_current)
             VALUES ('w', 'fixture-mid', 'balanced', 'v1.00112233445566aa', ?1, 1, 1, 2)",
            rusqlite::params![catalog_hash('1')],
        )
        .is_err());
    Ok(())
}

/// The snapshot columns are the whole durable footprint of a catalog. There is
/// no column that could hold Azure authority, so a future write cannot
/// introduce one without a migration and a review.
#[test]
fn no_model_catalog_table_has_a_column_that_could_hold_azure_authority(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let connection = rusqlite::Connection::open(store.database_path())?;

    for table in ["model_catalog_snapshots", "model_profile_selections"] {
        let mut statement =
            connection.prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        assert!(!columns.is_empty(), "{table} has no columns");
        for column in &columns {
            let column = column.to_lowercase();
            for forbidden in [
                "deployment",
                "endpoint",
                "subscription",
                "tenant",
                "resource",
                "credential",
                "api_key",
                "etag",
                "display",
            ] {
                assert!(!column.contains(forbidden), "{table}.{column}");
            }
        }
    }
    Ok(())
}

#[test]
fn a_verified_model_catalog_snapshot_survives_a_restart_as_metadata_only(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    {
        let store = LocalStore::open(directory.path(), &TestProtector)?;
        store.record_model_catalog_snapshot(&snapshot(WORKSPACE, SUBJECT, 7, '1'))?;
        store.record_model_selection(WORKSPACE, &selection("fixture-mid", "balanced", 100), 101)?;
    }

    let reopened = LocalStore::open(directory.path(), &TestProtector)?;
    let restored = reopened
        .model_catalog_snapshot(WORKSPACE, SUBJECT)?
        .expect("the snapshot survives a restart");
    assert_eq!(restored, snapshot(WORKSPACE, SUBJECT, 7, '1'));
    assert_eq!(
        reopened
            .current_model_selection(WORKSPACE)?
            .expect("the selection survives a restart")
            .profile_id,
        "fixture-mid"
    );
    Ok(())
}

#[test]
fn a_replayed_older_model_catalog_never_overwrites_a_newer_one(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    store.record_model_catalog_snapshot(&snapshot(WORKSPACE, SUBJECT, 9, '1'))?;

    // A strictly older epoch is a downgrade.
    assert!(matches!(
        store.record_model_catalog_snapshot(&snapshot(WORKSPACE, SUBJECT, 8, '2')),
        Err(StoreError::Inconsistent)
    ));
    // The same epoch carrying different content is a transplant.
    assert!(matches!(
        store.record_model_catalog_snapshot(&snapshot(WORKSPACE, SUBJECT, 9, '3')),
        Err(StoreError::Inconsistent)
    ));
    // A version that is not shaped like a catalog version is refused outright,
    // because an unparseable epoch cannot be compared against anything.
    let mut malformed = snapshot(WORKSPACE, SUBJECT, 9, '1');
    malformed.catalog_version = "not-a-version".to_owned();
    assert!(matches!(
        store.record_model_catalog_snapshot(&malformed),
        Err(StoreError::Inconsistent)
    ));

    // Nothing moved.
    let held = store
        .model_catalog_snapshot(WORKSPACE, SUBJECT)?
        .expect("the newer snapshot is still held");
    assert_eq!(held.catalog_version, "v9.00112233445566aa");
    assert_eq!(held.catalog_hash, catalog_hash('1'));

    // Re-recording the identical catalog is idempotent and only advances when
    // it was last verified.
    let mut reverified = snapshot(WORKSPACE, SUBJECT, 9, '1');
    reverified.verified_at_ms = GENERATED_AT + 60_000;
    store.record_model_catalog_snapshot(&reverified)?;
    assert_eq!(
        store
            .model_catalog_snapshot(WORKSPACE, SUBJECT)?
            .expect("snapshot")
            .verified_at_ms,
        GENERATED_AT + 60_000
    );

    // A newer epoch is accepted.
    store.record_model_catalog_snapshot(&snapshot(WORKSPACE, SUBJECT, 10, '4'))?;
    assert_eq!(
        store
            .model_catalog_snapshot(WORKSPACE, SUBJECT)?
            .expect("snapshot")
            .catalog_version,
        "v10.00112233445566aa"
    );
    Ok(())
}

#[test]
fn model_catalog_snapshots_and_selections_are_isolated_per_workspace_and_subject(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    store.record_model_catalog_snapshot(&snapshot(WORKSPACE, SUBJECT, 7, '1'))?;
    store.record_model_catalog_snapshot(&snapshot(OTHER_WORKSPACE, SUBJECT, 3, '2'))?;
    store.record_model_selection(WORKSPACE, &selection("fixture-mid", "balanced", 100), 100)?;
    store.record_model_selection(
        OTHER_WORKSPACE,
        &selection("fixture-alpha", "fast", 100),
        100,
    )?;

    // One workspace's downgrade guard does not constrain another's: an epoch of
    // 3 is fine in a workspace that never saw 7.
    assert_eq!(
        store
            .model_catalog_snapshot(OTHER_WORKSPACE, SUBJECT)?
            .expect("snapshot")
            .catalog_version,
        "v3.00112233445566aa"
    );
    // A different subject in the same workspace is a different row entirely.
    assert!(store
        .model_catalog_snapshot(WORKSPACE, OTHER_SUBJECT)?
        .is_none());

    assert_eq!(
        store
            .current_model_selection(WORKSPACE)?
            .expect("selection")
            .profile_id,
        "fixture-mid"
    );
    assert_eq!(
        store
            .current_model_selection(OTHER_WORKSPACE)?
            .expect("selection")
            .profile_id,
        "fixture-alpha"
    );
    assert_eq!(store.recent_model_selections(WORKSPACE)?.len(), 1);
    Ok(())
}

#[test]
fn exactly_one_model_catalog_selection_per_workspace_is_current_and_the_rest_are_history(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    store.record_model_selection(WORKSPACE, &selection("fixture-mid", "balanced", 100), 100)?;
    store.record_model_selection(WORKSPACE, &selection("fixture-alpha", "fast", 200), 200)?;
    store.record_model_selection(WORKSPACE, &selection("fixture-mid", "thorough", 300), 300)?;

    let history = store.recent_model_selections(WORKSPACE)?;
    assert_eq!(history.len(), 3);
    // Newest first.
    assert_eq!(history[0].profile_id, "fixture-mid");
    assert_eq!(history[0].variant_id, "thorough");
    assert!(history[0].is_current);
    assert_eq!(history.iter().filter(|entry| entry.is_current).count(), 1);

    // The database, not the writer, is what guarantees a single current row.
    let connection = rusqlite::Connection::open(store.database_path())?;
    assert!(connection
        .execute(
            "UPDATE model_profile_selections SET is_current = 1 WHERE workspace_id = ?1",
            rusqlite::params![WORKSPACE],
        )
        .is_err());
    Ok(())
}

#[test]
fn a_removed_model_catalog_profile_clears_the_current_choice_without_erasing_it(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    store.record_model_selection(WORKSPACE, &selection("fixture-alpha", "fast", 100), 100)?;
    store.record_model_selection(WORKSPACE, &selection("fixture-gone", "balanced", 200), 200)?;

    // The catalog no longer offers `fixture-gone`. The host clears current; it
    // does not pick a replacement and it does not forget the choice.
    store.clear_current_model_selection(WORKSPACE)?;

    assert!(store.current_model_selection(WORKSPACE)?.is_none());
    let history = store.recent_model_selections(WORKSPACE)?;
    assert_eq!(history.len(), 2);
    assert!(history.iter().all(|entry| !entry.is_current));
    assert_eq!(history[0].profile_id, "fixture-gone");
    assert_eq!(history[1].profile_id, "fixture-alpha");
    Ok(())
}

#[test]
fn a_selection_made_under_an_older_model_catalog_keeps_the_catalog_it_was_made_from(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    let mut stale = selection("fixture-mid", "balanced", 100);
    stale.catalog_version = "v3.00112233445566aa".to_owned();
    stale.catalog_hash = catalog_hash('9');
    store.record_model_selection(WORKSPACE, &stale, 100)?;
    store.record_model_catalog_snapshot(&snapshot(WORKSPACE, SUBJECT, 7, '1'))?;

    // The selection is not silently re-stamped with the newer catalog. Which
    // catalog a choice was made under is what tells the caller that context
    // estimates, consent, and any prepared request are stale.
    let current = store
        .current_model_selection(WORKSPACE)?
        .expect("the stale selection is still current");
    assert_eq!(current.catalog_version, "v3.00112233445566aa");
    assert_eq!(current.catalog_hash, catalog_hash('9'));
    assert_ne!(
        current.catalog_hash,
        store
            .model_catalog_snapshot(WORKSPACE, SUBJECT)?
            .expect("snapshot")
            .catalog_hash
    );

    // A selection whose catalog version is malformed never becomes durable.
    let mut malformed = selection("fixture-alpha", "fast", 200);
    malformed.catalog_version = "7".to_owned();
    assert!(matches!(
        store.record_model_selection(WORKSPACE, &malformed, 200),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}

#[test]
fn model_catalog_selection_history_is_bounded_and_never_prunes_the_current_choice(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;

    // The oldest choice, which every later write should eventually push out.
    store.record_model_selection(WORKSPACE, &selection("fixture-oldest", "balanced", 1), 1)?;
    let bound = u64::try_from(MAX_RECENT_MODEL_SELECTIONS).expect("a small bound");
    for index in 0..(bound + 4) {
        let profile = format!("fixture-p{index}");
        store.record_model_selection(
            WORKSPACE,
            &selection(&profile, "balanced", 100 + index),
            100 + index,
        )?;
    }

    let history = store.recent_model_selections(WORKSPACE)?;
    assert_eq!(history.len(), MAX_RECENT_MODEL_SELECTIONS);
    assert_eq!(history.iter().filter(|entry| entry.is_current).count(), 1);
    assert!(history[0].is_current);
    assert!(!history
        .iter()
        .any(|entry| entry.profile_id == "fixture-oldest"));

    // Recording an *older* selection as current must not let the bound evict
    // it: the current choice is never a pruning candidate.
    store.record_model_selection(WORKSPACE, &selection("fixture-ancient", "balanced", 2), 500)?;
    let history = store.recent_model_selections(WORKSPACE)?;
    assert_eq!(history.len(), MAX_RECENT_MODEL_SELECTIONS);
    let current = store
        .current_model_selection(WORKSPACE)?
        .expect("the ancient choice is current");
    assert_eq!(current.profile_id, "fixture-ancient");
    Ok(())
}

#[test]
fn a_corrupted_model_catalog_row_fails_closed_rather_than_being_read_as_data(
) -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let store = LocalStore::open(directory.path(), &TestProtector)?;
    store.record_model_selection(WORKSPACE, &selection("fixture-mid", "balanced", 100), 100)?;
    store.record_model_catalog_snapshot(&snapshot(WORKSPACE, SUBJECT, 7, '1'))?;

    // Simulates on-disk damage rather than a legal write: the CHECK constraints
    // refuse a negative millisecond, so the only way this state can arise is a
    // corrupted file. The reader must still refuse it rather than wrapping it
    // into an enormous instant.
    let connection = rusqlite::Connection::open(store.database_path())?;
    connection.execute_batch("PRAGMA ignore_check_constraints = ON;")?;
    connection.execute(
        "UPDATE model_profile_selections SET selected_at_ms = -1 WHERE workspace_id = ?1",
        rusqlite::params![WORKSPACE],
    )?;
    assert!(matches!(
        store.recent_model_selections(WORKSPACE),
        Err(StoreError::Inconsistent)
    ));

    connection.execute(
        "UPDATE model_catalog_snapshots SET generated_at_ms = -1 WHERE workspace_id = ?1",
        rusqlite::params![WORKSPACE],
    )?;
    assert!(matches!(
        store.model_catalog_snapshot(WORKSPACE, SUBJECT),
        Err(StoreError::Inconsistent)
    ));
    Ok(())
}
