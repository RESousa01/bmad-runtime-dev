#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::unwrap_used,
    clippy::panic
)]
//! Negative and positive tests for the BMAD script sandbox.
//!
//! Every negative test asserts on the specific `SandboxDenial` variant the
//! sandbox produced, not merely `is_err()`. Where feasible the case is
//! constructed so it would succeed without the sandbox's check, so the test
//! is proving denial rather than coincidence.

use std::collections::BTreeSet;
use std::fs;

use desktop_runtime::execution::sandbox::{
    check_network, check_quota, recover_stale_runs, resolve_declared_path, resolve_environment,
    stage_input, verify_approval_current, ControlState, ProcessIsolationPort, RunRoot,
    SandboxDenial, UnavailableProcessIsolation,
};
use desktop_runtime::execution::script_profiles::{
    ApprovalBinding, CleanupPolicy, CommandSchema, MountDeclaration, NetworkPolicy, ResourceQuotas,
    ScriptProfile,
};
use desktop_runtime::sha256_bytes;

fn quotas() -> ResourceQuotas {
    ResourceQuotas {
        timeout_ms: 5_000,
        cpu_time_ms: 2_000,
        memory_bytes: 64 * 1024 * 1024,
        max_processes: 1,
        max_open_files: 16,
        max_output_bytes: 4_096,
    }
}

fn base_profile() -> ScriptProfile {
    let script_bytes = b"print('hello')";
    ScriptProfile {
        profile_id: "profile-1".to_string(),
        script_hash: sha256_bytes(script_bytes),
        interpreter_hash: sha256_bytes(b"python3.11"),
        read_only_mounts: vec![MountDeclaration {
            mount_path: "in/spec.json".to_string(),
            expected_content_hash: Some(sha256_bytes(b"{}")),
        }],
        read_write_mounts: vec![MountDeclaration {
            mount_path: "out".to_string(),
            expected_content_hash: None,
        }],
        environment_allowlist: BTreeSet::from(["BMAD_RUN_ID".to_string()]),
        network_policy: NetworkPolicy::Denied,
        command_schema: CommandSchema {
            program: "python3".to_string(),
            argument_pattern: r"^--spec=in/spec\.json$".to_string(),
        },
        quotas: quotas(),
        expected_outputs: vec!["out/result.json".to_string()],
        cleanup_policy: CleanupPolicy::Always,
    }
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bmad-sandbox-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp base");
    dir
}

// Break caught: a renderer-supplied absolute path must never be treated as
// mount authority, even when it points at a real file that exists on disk.
// Without the traversal check this would resolve straight to the OS path
// and read it.
#[test]
fn bmad_script_sandbox_denies_absolute_path_as_declared_input() {
    let base = temp_dir("absolute-path");
    let run_root = RunRoot::create(&base, "run-a").expect("create run root");
    let profile = base_profile();

    // Construct a real file outside the sandbox that the absolute path
    // would successfully read if the check were absent.
    let outside = base.join("outside-secret.txt");
    fs::write(&outside, b"leaked").expect("write outside file");

    let result = resolve_declared_path(&profile, &run_root, outside.to_str().unwrap(), false);
    assert!(matches!(result, Err(SandboxDenial::PathTraversal { .. })));
}

// Break caught: `..` segments must be rejected even when the resulting
// textual path would still land inside the sandbox root after normalization
// by a naive joiner.
#[test]
fn bmad_script_sandbox_denies_dot_dot_traversal() {
    let base = temp_dir("dotdot");
    let run_root = RunRoot::create(&base, "run-b").expect("create run root");
    let profile = base_profile();

    let result = resolve_declared_path(&profile, &run_root, "in/../../../etc/passwd", false);
    assert!(matches!(result, Err(SandboxDenial::PathTraversal { .. })));
}

// Break caught: a junction/symlink planted inside a declared mount that
// points outside the sandbox root must be caught by canonicalization, not
// just by the textual path.
#[test]
fn bmad_script_sandbox_denies_symlink_escape_from_declared_mount() {
    let base = temp_dir("symlink-escape");
    let run_root = RunRoot::create(&base, "run-c").expect("create run root");
    let profile = base_profile();

    // Real target outside the sandbox that the reparse point points to.
    let outside_target = base.join("outside-target");
    fs::create_dir_all(&outside_target).expect("create outside target");
    fs::write(outside_target.join("secret.txt"), b"leaked").expect("write outside secret");

    let out_mount = run_root.path().join("out");
    fs::create_dir_all(run_root.path()).expect("create run root dir");

    // symlink_dir requires elevation on some Windows configurations; if it
    // cannot be created here, the check this test targets cannot be
    // exercised, and the test must say so rather than pass by accident.
    #[cfg(windows)]
    let created = std::os::windows::fs::symlink_dir(&outside_target, &out_mount).is_ok();
    #[cfg(not(windows))]
    let created = std::os::unix::fs::symlink(&outside_target, &out_mount).is_ok();

    if !created {
        eprintln!(
            "bmad_script_sandbox_denies_symlink_escape_from_declared_mount: skipped — this \
             process could not create a directory symlink/junction in this environment \
             (requires Developer Mode or elevation on Windows). This is an environment \
             limitation, not a passing assertion."
        );
        return;
    }

    let result = resolve_declared_path(&profile, &run_root, "out/secret.txt", true);
    assert!(matches!(result, Err(SandboxDenial::SymlinkEscape { .. })));
}

// Break caught: NTFS alternate-data-stream syntax (`name:stream`) must be
// rejected outright rather than passed through to a file API that would
// silently open the hidden stream.
#[test]
fn bmad_script_sandbox_denies_alternate_data_stream_path() {
    let base = temp_dir("ads");
    let run_root = RunRoot::create(&base, "run-d").expect("create run root");
    let profile = base_profile();

    let result = resolve_declared_path(&profile, &run_root, "out/result.json:hidden", true);
    assert!(matches!(
        result,
        Err(SandboxDenial::AlternateDataStream { .. })
    ));
}

// Break caught: a path that is not one of the profile's declared mounts
// must be refused even when it is a plain relative path with no traversal
// tricks at all.
#[test]
fn bmad_script_sandbox_denies_undeclared_path() {
    let base = temp_dir("undeclared");
    let run_root = RunRoot::create(&base, "run-e").expect("create run root");
    let profile = base_profile();

    let result = resolve_declared_path(&profile, &run_root, "undeclared/file.txt", false);
    assert!(matches!(result, Err(SandboxDenial::PathNotDeclared { .. })));
}

// Break caught: writing to a mount declared read-only must be refused with
// a distinct reason from "not declared at all", proving the checker
// actually consulted mount permissions rather than just membership.
#[test]
fn bmad_script_sandbox_denies_write_to_read_only_mount() {
    let base = temp_dir("read-only-write");
    let run_root = RunRoot::create(&base, "run-f").expect("create run root");
    let profile = base_profile();

    let result = resolve_declared_path(&profile, &run_root, "in/spec.json", true);
    assert!(matches!(result, Err(SandboxDenial::MountReadOnly { .. })));
}

// Break caught: the sandbox must never pass through an environment variable
// that is not on the profile's allowlist, even when the caller requests
// only one variable and it looks harmless.
#[test]
fn bmad_script_sandbox_denies_undeclared_environment_variable() {
    let profile = base_profile();
    let requested = BTreeSet::from(["UNDECLARED_VAR".to_string()]);
    let result = resolve_environment(&profile, &requested);
    assert!(matches!(
        result,
        Err(SandboxDenial::EnvNotAllowlisted { .. })
    ));
}

// Break caught: a hard-denied secret-shaped variable name must be refused
// even if a (misconfigured) profile explicitly allowlisted it — the
// allowlist alone is not sufficient authority for secrets.
#[test]
fn bmad_script_sandbox_denies_environment_secret_even_when_allowlisted() {
    let mut profile = base_profile();
    profile
        .environment_allowlist
        .insert("AWS_SECRET_ACCESS_KEY".to_string());
    let requested = BTreeSet::from(["AWS_SECRET_ACCESS_KEY".to_string()]);
    let result = resolve_environment(&profile, &requested);
    assert!(matches!(result, Err(SandboxDenial::EnvHardDenied { .. })));
}

// Break caught: a `Denied` network policy must refuse every host, proven
// against a host that would otherwise be a completely ordinary, reachable
// address.
#[test]
fn bmad_script_sandbox_denies_network_access_when_policy_is_denied() {
    let profile = base_profile();
    let result = check_network(&profile, "example.com");
    assert!(matches!(result, Err(SandboxDenial::NetworkDenied { .. })));
}

// Break caught: the cloud instance-metadata endpoint must be refused
// unconditionally, even for a profile whose network policy explicitly
// allowlists that exact host — proving this is a hard block, not a policy
// choice a profile author could accidentally or deliberately open.
#[test]
fn bmad_script_sandbox_denies_cloud_metadata_endpoint_even_when_allowlisted() {
    let mut profile = base_profile();
    profile.network_policy =
        NetworkPolicy::AllowlistedHosts(BTreeSet::from(["169.254.169.254".to_string()]));
    let result = check_network(&profile, "169.254.169.254");
    assert!(matches!(
        result,
        Err(SandboxDenial::CloudMetadataEndpoint { .. })
    ));
}

// Break caught: tampered script bytes must invalidate a previously granted
// approval. The approval is computed once against the original script hash;
// re-verifying against a profile whose script_hash now reflects different
// bytes must fail, and must name "script" as the drifted component.
#[test]
fn bmad_script_sandbox_denies_tampered_script_bytes_via_approval_invalidation() {
    let original = base_profile();
    let granted = ApprovalBinding::compute(&original).expect("compute approval");

    let mut tampered = original.clone();
    tampered.script_hash = sha256_bytes(b"print('malicious payload')");

    let result = verify_approval_current(&granted, &tampered);
    match result {
        Err(SandboxDenial::ApprovalInvalidated { detail }) => {
            assert!(detail.contains("script"), "detail was: {detail}");
        }
        other => panic!("expected ApprovalInvalidated naming script, got {other:?}"),
    }
}

// Break caught: changing only the interpreter (script and profile fields
// otherwise identical) must independently invalidate approval — proving
// the binding does not collapse interpreter identity into the script hash.
#[test]
fn bmad_script_sandbox_denies_changed_interpreter_via_approval_invalidation() {
    let original = base_profile();
    let granted = ApprovalBinding::compute(&original).expect("compute approval");

    let mut changed = original.clone();
    changed.interpreter_hash = sha256_bytes(b"python2.7");

    let result = verify_approval_current(&granted, &changed);
    match result {
        Err(SandboxDenial::ApprovalInvalidated { detail }) => {
            // interpreter_hash is itself a field of the hashed profile, so
            // changing it also changes the profile hash; both must be named.
            assert!(detail.contains("interpreter"), "detail was: {detail}");
        }
        other => panic!("expected ApprovalInvalidated naming interpreter, got {other:?}"),
    }
}

// Break caught: widening the declared profile (e.g. adding a mount) after
// approval must independently invalidate approval even though script and
// interpreter hashes are untouched — proving the profile's own content is
// load-bearing in the binding, not just an identifier.
#[test]
fn bmad_script_sandbox_denies_widened_profile_via_approval_invalidation() {
    let original = base_profile();
    let granted = ApprovalBinding::compute(&original).expect("compute approval");

    let mut widened = original.clone();
    widened.read_write_mounts.push(MountDeclaration {
        mount_path: "out/extra".to_string(),
        expected_content_hash: None,
    });

    let result = verify_approval_current(&granted, &widened);
    match result {
        Err(SandboxDenial::ApprovalInvalidated { detail }) => {
            assert_eq!(detail, "profile");
        }
        other => panic!("expected ApprovalInvalidated naming profile, got {other:?}"),
    }
}

// Positive complement: an approval re-verified against the exact same
// profile must still pass, proving the invalidation checks above are
// discriminating on the actual change and not simply always failing.
#[test]
fn bmad_script_sandbox_accepts_unmodified_approval() {
    let profile = base_profile();
    let granted = ApprovalBinding::compute(&profile).expect("compute approval");
    assert!(verify_approval_current(&granted, &profile).is_ok());
}

// Break caught: staged input bytes that do not match the profile's declared
// content hash must be refused with the specific expected/actual hashes, not
// merely rejected — and the mismatched file must not be left on disk.
#[test]
fn bmad_script_sandbox_denies_tampered_input_content_hash() {
    let base = temp_dir("content-hash-mismatch");
    let run_root = RunRoot::create(&base, "run-g").expect("create run root");
    let profile = base_profile();
    let mount = profile.read_only_mounts[0].clone();

    let result = stage_input(&profile, &run_root, &mount, b"{\"tampered\": true}");
    match result {
        Err(SandboxDenial::ContentHashMismatch {
            expected, actual, ..
        }) => {
            assert_ne!(expected, actual);
        }
        other => panic!("expected ContentHashMismatch, got {other:?}"),
    }
    assert!(
        !run_root.path().join("in/spec.json").exists(),
        "a content-mismatched input must not be left readable on disk"
    );
}

// Positive complement: bytes matching the declared hash stage successfully,
// proving the hash check above discriminates rather than always failing.
#[test]
fn bmad_script_sandbox_accepts_matching_input_content_hash() {
    let base = temp_dir("content-hash-match");
    let run_root = RunRoot::create(&base, "run-h").expect("create run root");
    let profile = base_profile();
    let mount = profile.read_only_mounts[0].clone();

    let result = stage_input(&profile, &run_root, &mount, b"{}");
    assert!(result.is_ok());
}

// Break caught: an output-flood-shaped write must be flagged by the
// declared output quota, proving the quota is checked against the profile's
// number rather than an unbounded default.
#[test]
fn bmad_script_sandbox_denies_output_flood_via_quota_check() {
    let profile = base_profile();
    let observed_bytes: u64 = profile.quotas.max_output_bytes + 1;
    let result = check_quota(
        "max_output_bytes",
        profile.quotas.max_output_bytes,
        observed_bytes,
    );
    assert!(matches!(
        result,
        Err(SandboxDenial::QuotaExceeded { quota, .. }) if quota == "max_output_bytes"
    ));
}

// Break caught: a process-count observation shaped like a fork-bomb (far
// beyond the declared `max_processes`) must be flagged by the same quota
// mechanism, proven against the profile's own declared limit.
#[test]
fn bmad_script_sandbox_denies_process_tree_explosion_via_quota_check() {
    let profile = base_profile();
    let observed_processes: u64 = u64::from(profile.quotas.max_processes) + 5_000;
    let result = check_quota(
        "max_processes",
        u64::from(profile.quotas.max_processes),
        observed_processes,
    );
    assert!(matches!(
        result,
        Err(SandboxDenial::QuotaExceeded { quota, .. }) if quota == "max_processes"
    ));
}

// Break caught: a CPU-time observation beyond the declared budget must be
// flagged, proven against the profile's own declared `cpu_time_ms`.
#[test]
fn bmad_script_sandbox_denies_cpu_exhaustion_via_quota_check() {
    let profile = base_profile();
    let observed_cpu_ms: u64 = profile.quotas.cpu_time_ms * 10;
    let result = check_quota("cpu_time_ms", profile.quotas.cpu_time_ms, observed_cpu_ms);
    assert!(matches!(
        result,
        Err(SandboxDenial::QuotaExceeded { quota, .. }) if quota == "cpu_time_ms"
    ));
}

// Break caught: a memory observation beyond the declared budget must be
// flagged, proven against the profile's own declared `memory_bytes`.
#[test]
fn bmad_script_sandbox_denies_memory_exhaustion_via_quota_check() {
    let profile = base_profile();
    let observed_bytes: u64 = profile.quotas.memory_bytes * 4;
    let result = check_quota("memory_bytes", profile.quotas.memory_bytes, observed_bytes);
    assert!(matches!(
        result,
        Err(SandboxDenial::QuotaExceeded { quota, .. }) if quota == "memory_bytes"
    ));
}

// Break caught (honesty test): a child process persisting after kill, and
// live descendant termination in general, require an OS process-isolation
// port this crate does not have. This test proves the port reports that
// truthfully — `Unavailable`, not a silent `Ok(())` that would misrepresent
// an unenforced control as a success.
#[test]
fn bmad_script_sandbox_reports_process_kill_as_unavailable_not_silently_ok() {
    let port = UnavailableProcessIsolation;
    assert_eq!(port.control_state(), ControlState::Unavailable);

    let profile = base_profile();
    let result = port.spawn_isolated(&profile);
    match result {
        Err(SandboxDenial::Unavailable { control, .. }) => {
            assert_eq!(control, "process_isolation");
        }
        other => panic!(
            "process isolation must report Unavailable explicitly, not {other:?} — a silent \
             Ok(()) here would misreport an unenforced control as enforced"
        ),
    }
}

// Restart-safety: a run directory left behind without the completion marker
// (simulating a crash mid-run) must be quarantined on the next sweep rather
// than treated as trusted leftover output, and a cleanly completed run must
// be left alone.
#[test]
fn bmad_script_sandbox_quarantines_stale_run_missing_completion_marker() {
    let base = temp_dir("restart-recovery");

    let crashed = RunRoot::create(&base, "crashed-run").expect("create crashed run root");
    fs::write(crashed.path().join("out.txt"), b"partial").expect("write partial output");
    // Deliberately never call mark_complete on this one.

    let clean = RunRoot::create(&base, "clean-run").expect("create clean run root");
    clean.mark_complete().expect("mark complete");

    let recovered = recover_stale_runs(&base).expect("recover stale runs");

    let crashed_entry = recovered
        .iter()
        .find(|entry| entry.run_id == "crashed-run")
        .expect("crashed run present in recovery report");
    assert!(crashed_entry.quarantined);
    assert!(!base.join("runs").join("crashed-run").exists());

    let clean_entry = recovered
        .iter()
        .find(|entry| entry.run_id == "clean-run")
        .expect("clean run present in recovery report");
    assert!(!clean_entry.quarantined);
    assert!(base.join("runs").join("clean-run").exists());
}

// Restart-safety is idempotent: sweeping twice must not error and must not
// re-quarantine an already-quarantined run under a new base scan.
#[test]
fn bmad_script_sandbox_restart_recovery_is_idempotent() {
    let base = temp_dir("restart-idempotent");
    let crashed = RunRoot::create(&base, "crashed-run").expect("create crashed run root");
    fs::write(crashed.path().join("out.txt"), b"partial").expect("write partial output");

    let first = recover_stale_runs(&base).expect("first sweep");
    assert_eq!(first.len(), 1);
    let second = recover_stale_runs(&base).expect("second sweep");
    // The runs/ directory is now empty (moved to quarantine), so a second
    // sweep finds nothing left to recover — proving no double-processing.
    assert!(second.is_empty());
}
