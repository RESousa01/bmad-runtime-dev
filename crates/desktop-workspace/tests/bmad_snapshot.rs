#![allow(clippy::expect_used)]

use desktop_runtime::BmadLocationClass;
use desktop_workspace::{read_bmad_source_snapshot, BmadSnapshotError};

#[test]
fn reads_observed_method_composite_from_all_three_host_roots() {
    let root = tempfile::tempdir().expect("workspace");
    let files = [
        ("_bmad/config.toml", b"[core]\nname='test'\n".as_slice()),
        (
            "_bmad/manifest.yaml",
            b"staging: evidence-only\n".as_slice(),
        ),
        (".agents/skills/alpha/SKILL.md", b"# Alpha\n".as_slice()),
        (".claude/skills/beta/SKILL.md", b"# Beta\n".as_slice()),
    ];
    for (relative, bytes) in files {
        let path = root
            .path()
            .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        std::fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        std::fs::write(path, bytes).expect("source bytes");
    }

    let snapshot = read_bmad_source_snapshot(root.path()).expect("BMAD snapshot");
    assert_eq!(snapshot.entries().len(), 4);
    assert!(snapshot.entries().iter().any(|entry| {
        entry.path() == "_bmad/config.toml" && entry.location() == BmadLocationClass::BmadControl
    }));
    assert!(snapshot.entries().iter().any(|entry| {
        entry.path() == ".agents/skills/alpha/SKILL.md"
            && entry.location() == BmadLocationClass::HostNativeAgents
    }));
    assert!(snapshot.entries().iter().any(|entry| {
        entry.path() == ".claude/skills/beta/SKILL.md"
            && entry.location() == BmadLocationClass::HostNativeClaude
    }));
}

#[test]
fn final_inventory_is_derived_from_observed_bytes_not_staging_manifest_claims() {
    let root = tempfile::tempdir().expect("workspace");
    let skill = root.path().join(".agents/skills/alpha/SKILL.md");
    std::fs::create_dir_all(skill.parent().expect("parent")).expect("directory");
    std::fs::write(&skill, b"first observed bytes").expect("first bytes");
    let first = read_bmad_source_snapshot(root.path()).expect("first snapshot");

    std::fs::write(&skill, b"second observed bytes").expect("second bytes");
    let second = read_bmad_source_snapshot(root.path()).expect("second snapshot");
    assert_ne!(
        first.observed_inventory_hash(),
        second.observed_inventory_hash()
    );
}

#[test]
fn skips_sensitive_entries_and_rejects_source_limit_overflow() {
    let root = tempfile::tempdir().expect("workspace");
    let secret = root.path().join("_bmad/.env");
    std::fs::create_dir_all(secret.parent().expect("parent")).expect("directory");
    std::fs::write(secret, b"TOKEN=secret").expect("secret fixture");
    let visible = root.path().join("_bmad/config.toml");
    std::fs::write(visible, b"safe=true").expect("visible fixture");

    let snapshot = read_bmad_source_snapshot(root.path()).expect("filtered snapshot");
    assert_eq!(snapshot.entries().len(), 1);
    assert_eq!(snapshot.entries()[0].path(), "_bmad/config.toml");

    let overflow = root.path().join(".agents/skills/huge/SKILL.md");
    std::fs::create_dir_all(overflow.parent().expect("parent")).expect("directory");
    std::fs::write(overflow, vec![b'x'; 1_048_577]).expect("oversized fixture");
    assert!(matches!(
        read_bmad_source_snapshot(root.path()),
        Err(BmadSnapshotError::LimitExceeded)
    ));
}

#[test]
fn context_library_and_build_outputs_are_never_discovery_roots() {
    let root = tempfile::tempdir().expect("workspace");
    for relative in [
        "bmad-runtime-lib/_source_review/skill/SKILL.md",
        "target/debug/skill/SKILL.md",
        "node_modules/package/SKILL.md",
    ] {
        let path = root
            .path()
            .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        std::fs::create_dir_all(path.parent().expect("parent")).expect("directory");
        std::fs::write(path, b"must not be discovered").expect("fixture");
    }
    let snapshot = read_bmad_source_snapshot(root.path()).expect("empty snapshot");
    assert!(snapshot.entries().is_empty());
}
