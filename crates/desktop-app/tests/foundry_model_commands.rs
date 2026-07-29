//! Host wiring for the Foundry model command lane.
//!
//! `HostState`, `HostCommandData` and `execute_command` are all crate-private
//! and reachable only through a running Tauri window, so this asserts over the
//! host sources the way `lib.rs` already does for the Tauri command lane. That
//! is a deliberate trade: what is being defended here is the *shape* of the
//! boundary — which commands exist, what they are allowed to carry, and which
//! of them may be replayed — and every one of those facts is visible in the
//! source and would decay silently without something reading it.
//!
//! The behavioural half of this lane is covered where it is reachable:
//! `desktop-ipc/tests/foundry_model_ipc.rs` for parsing and admission, and the
//! `model` unit tests for the projections.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

/// The commands this lane added, in reviewed catalog order.
const FOUNDRY_MODEL_COMMANDS: [&str; 5] = [
    "model.catalog.get",
    "model.catalog.refresh",
    "model.selection.get",
    "model.response.send",
    "model.response.stop",
];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    let path = crate_root().join(relative);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    // Normalized so the assertions below are about the code and not about
    // whichever line ending the checkout happens to have produced.
    source.replace("\r\n", "\n")
}

/// The exact string literals of one `[&str; N]` constant.
fn string_array_constant(source: &str, name: &str) -> Vec<String> {
    let declaration = format!("const {name}: [&str; ");
    let start = source
        .find(&declaration)
        .unwrap_or_else(|| panic!("{name} is declared"));
    let open = source[start..]
        .find('[')
        .map(|offset| start + offset)
        .expect("array literal opens");
    let body_start = source[open..]
        .find("= [")
        .map(|offset| open + offset + 3)
        .expect("array literal body");
    let body_end = source[body_start..]
        .find("];")
        .map(|offset| body_start + offset)
        .expect("array literal closes");
    source[body_start..body_end]
        .split(',')
        .filter_map(|entry| {
            let trimmed = entry.trim();
            let inner = trimmed.strip_prefix('"')?.strip_suffix('"')?;
            Some(inner.to_owned())
        })
        .collect()
}

/// The host projects exactly this catalog to a Ready renderer, and
/// `tools/check-boundaries.mjs` pins the same list byte-for-byte across four
/// other files. Order is part of the contract, not an accident of how it was
/// typed.
#[test]
fn ready_commands_carry_the_five_new_commands_in_reviewed_order() {
    let commands = string_array_constant(&read("src/commands.rs"), "READY_COMMANDS");
    assert_eq!(commands.len(), 51, "READY_COMMANDS changed length");

    let start = commands
        .iter()
        .position(|command| command == "model.catalog.get")
        .expect("model.catalog.get is advertised");
    assert_eq!(&commands[start..start + 5], &FOUNDRY_MODEL_COMMANDS);
    assert_eq!(commands[start - 1], "model.selection.set");
    assert_eq!(commands[start + 5], "chat.send");

    // No duplicates anywhere: a repeated entry would make the pinned catalogs
    // disagree on length while still listing the same names.
    let mut sorted = commands.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), commands.len(), "READY_COMMANDS has duplicates");
}

/// The catalog and the recovery catalog are different lists for a reason. None
/// of these commands may be reachable while the host is in read-only recovery:
/// there is no store to read a selection from and no authority to spend.
#[test]
fn no_new_command_is_reachable_from_read_only_recovery() {
    let recovery = string_array_constant(&read("src/commands.rs"), "RECOVERY_COMMANDS");
    for command in FOUNDRY_MODEL_COMMANDS {
        assert!(
            !recovery.contains(&command.to_owned()),
            "`{command}` is reachable in read-only recovery"
        );
    }
}

/// Every added command routes somewhere. A `LocalCommand` variant with no
/// `execute_command` arm would not compile, but a variant routed to the wrong
/// handler would — so the pairing is named explicitly.
#[test]
fn every_added_command_routes_to_its_own_handler() {
    let dispatch = read("src/commands.rs");
    let source = read("src/commands/model.rs");
    for (variant, handler) in [
        ("LocalCommand::GetModelCatalog", "model_catalog_data"),
        ("LocalCommand::RefreshModelCatalog", "refresh_model_catalog"),
        ("LocalCommand::GetModelSelection", "model_selection_data"),
        ("LocalCommand::SendModelResponse", "send_model_response"),
        ("LocalCommand::StopModelResponse", "stop_model_response"),
    ] {
        assert!(
            dispatch.contains(variant),
            "{variant} has no execute_command arm"
        );
        assert!(
            source.contains(&format!("fn {handler}(")),
            "{handler} is not defined"
        );
    }
}

/// Neither half of the response lifecycle may be answered from the reply cache.
///
/// The gate already refuses to replay them, so a cached reply could only be
/// served to a request the gate had just rejected — but the two lists are
/// maintained separately, and this is what keeps them agreeing.
#[test]
fn the_response_lifecycle_is_never_served_from_the_reply_cache() {
    let source = read("src/commands.rs");
    let start = source
        .find("fn should_cache_reply(")
        .expect("should_cache_reply is defined");
    let body_end = source[start..]
        .find("\n}\n")
        .map(|offset| start + offset)
        .expect("should_cache_reply closes");
    let body = &source[start..body_end];
    for variant in [
        "LocalCommand::SendModelResponse",
        "LocalCommand::StopModelResponse",
    ] {
        assert!(
            body.contains(variant),
            "{variant} is cacheable; a replay could be answered from the cache"
        );
    }
    // The refresh deliberately *is* cacheable: it is fingerprinted, so a replay
    // is the same read of the same catalog rather than a second fetch.
    assert!(
        !body.contains("LocalCommand::RefreshModelCatalog"),
        "the catalog refresh was excluded from the reply cache"
    );
}

/// The renderer projection carries the organization's own published model
/// metadata, but no way to reach a provider.
///
/// `capabilities`, `limits`, `family`, `availability`, `statusDetail`,
/// `variants` and `qualifiedAt` are now read off the canonical catalog profile
/// -- FND-051 and FND-052 need them for the picker and the context-window
/// rows -- but `catalog_projection_input` still has no way to read a
/// deployment name, an endpoint, a subscription/resource identifier, or
/// either policy/deployment-snapshot hash, because the canonical profile type
/// itself carries none of them into this function's reach.
#[test]
fn the_catalog_projection_reads_published_metadata_but_no_connection_authority() {
    let source = read("src/commands/model.rs");
    let start = source
        .find("fn catalog_projection_input(")
        .expect("catalog_projection_input is defined");
    let end = source[start..]
        .find("\nfn map_selectable_availability(")
        .map(|offset| start + offset)
        .expect("catalog_projection_input closes");
    let body = &source[start..end];
    for expected in ["capabilities", "limits", "family", "availability", "variants"] {
        assert!(
            body.contains(expected),
            "catalog_projection_input no longer reads `{expected}` off the canonical profile"
        );
    }
    for forbidden in [
        "deployment_snapshot_hash",
        "policy_hash",
        "deployment_name",
        "endpoint",
        "subscription",
        "credential",
    ] {
        assert!(
            !body.contains(forbidden),
            "catalog_projection_input reads `{forbidden}` off the canonical profile"
        );
    }
}

/// The guard against silently switching a model under a user mid-draft.
///
/// `RequiresExplicitChoice` has to survive the command layer as itself. The
/// failure this prevents is not a crash — it is a `_ =>` arm or an
/// `unwrap_or_default` that turns "your model is gone, choose again" into
/// "here is a different model", which nothing downstream could detect.
#[test]
fn requires_explicit_choice_is_never_collapsed_into_a_fallback() {
    let commands = read("src/commands/model.rs");
    let state = read("src/state/model.rs");

    assert!(
        commands.contains("RequiresExplicitChoice"),
        "the command layer does not mention RequiresExplicitChoice"
    );
    assert!(
        state.contains("RequiresExplicitChoice"),
        "the state layer does not mention RequiresExplicitChoice"
    );

    // `send` must route both non-resolving outcomes to the selection
    // projection, not to a default. If this arm ever became `Selected`-only
    // with a fallback, the projection call would disappear from the body.
    let start = commands
        .find("fn send_model_response(")
        .expect("send_model_response is defined");
    let end = commands[start..]
        .find("\n/// Stops one task-bound model response.")
        .map(|offset| start + offset)
        .expect("send_model_response closes");
    let body = &commands[start..end];
    assert!(
        body.contains("selection_data("),
        "send_model_response does not preserve the unresolved outcome"
    );
    for banned in ["unwrap_or_default", "unwrap_or_else", "CatalogDefault"] {
        assert!(
            !body.contains(banned),
            "send_model_response reaches for `{banned}` instead of asking the user"
        );
    }
}

/// The model selection stopped being a value you fall into.
#[test]
fn the_in_memory_selection_default_was_replaced_by_catalog_backed_state() {
    let state = read("src/state.rs") + &read("src/state/model.rs");
    assert!(
        !state.contains("impl Default for ModelSelectionState"),
        "ModelSelectionState still has a Default impl"
    );
    assert!(
        !state.contains("ModelSelectionState::default()"),
        "ModelSelectionState::default() is still constructed"
    );
    for expected in [
        "fn resolve_catalog_selection(",
        "fn install_model_catalog(",
        "fn record_catalog_selection(",
        "desktop_cloud::resolve_model_selection(",
    ] {
        assert!(
            state.contains(expected),
            "catalog-backed selection is missing `{expected}`"
        );
    }
}

/// The renderer is untrusted. These three primitives are the ones whose absence
/// the whole boundary rests on, and this lane must not have introduced one.
#[test]
fn the_lane_introduced_no_generic_effect_primitive() {
    for file in [
        "src/commands.rs",
        "src/commands/model.rs",
        "src/state.rs",
        "src/state/model.rs",
        "src/wire.rs",
    ] {
        let source = read(file);
        for forbidden in ["write_path", "apply_patch_text", "run_shell"] {
            assert!(
                !source.contains(forbidden),
                "{file} names the generic primitive `{forbidden}`"
            );
        }
    }
}

/// The evidence the IPC lane emits, checked from the host side too.
#[test]
fn the_recorded_ipc_evidence_matches_the_advertised_catalog() {
    let evidence_path = crate_root().join("../../artifacts/foundry/rust/ipc.json");
    let Ok(raw) = std::fs::read_to_string(&evidence_path) else {
        // Written by `desktop-ipc`'s own test run. Absent means that suite has
        // not run in this working tree yet, which is not this crate's failure.
        return;
    };
    let evidence: serde_json::Value = serde_json::from_str(&raw).expect("evidence JSON");
    assert_eq!(evidence["readyCommandCount"], 51);
    assert!(evidence["genericCapabilityFindings"]
        .as_array()
        .is_some_and(Vec::is_empty));

    let recorded: Vec<String> = evidence["readyCommands"]
        .as_array()
        .expect("readyCommands array")
        .iter()
        .map(|value| value.as_str().expect("command string").to_owned())
        .collect();
    let advertised = string_array_constant(&read("src/commands.rs"), "READY_COMMANDS");
    assert_eq!(
        recorded, advertised,
        "the recorded evidence and the host catalog disagree"
    );
}

/// A cheap guard on the helper above: a parser that silently returned an empty
/// list would make every assertion in this file vacuously true.
#[test]
fn the_constant_parser_actually_reads_the_source() {
    let commands = string_array_constant(&read("src/commands.rs"), "READY_COMMANDS");
    assert_eq!(commands.first().map(String::as_str), Some("app.get_boot_state"));
    assert_eq!(commands.last().map(String::as_str), Some("workspace.pick_folder"));
    assert!(Path::new(&crate_root().join("src/commands.rs")).is_file());
}
