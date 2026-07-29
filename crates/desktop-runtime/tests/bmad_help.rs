#![allow(clippy::expect_used)]

//! Integration-level characterization for ENG-11 (canonical deterministic
//! BMAD Help).
//!
//! The deterministic query surface added for ENG-11 (`BmadHelpQuery` and its
//! supporting types in `crates/desktop-runtime/src/bmad/help.rs`) lives in a
//! private module and is intentionally not re-exported through
//! `crates/desktop-runtime/src/bmad/mod.rs` or `crates/desktop-runtime/src/lib.rs`
//! yet -- those two files are owned by a different lease and must not be
//! touched here. That means the bulk of the acceptance-criteria tests (blocked
//! vs invokable, alias resolution, support state, catalog counts) run as
//! `#[cfg(test)] mod tests` unit tests inside `help.rs` itself, where the
//! types are in scope. This file covers what *is* reachable from outside the
//! crate: the structural "no model/chat capability" guarantee, proven purely
//! by scanning the module source, and a black-box confirmation that the
//! already-exported catalog/advisor surface needs no model or chat value
//! either.

use std::{fs, path::Path};

use desktop_runtime::{BmadCatalogBuilder, BmadHelpCatalogSource, BmadLoadedPackage};

fn help_source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bmad/help.rs");
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

/// Strips `//`, `///`, and `//!` line comments before scanning, mirroring
/// `chat_bmad_capability_separation.rs`'s `code_only_lines`, so this module's
/// own prose describing why it excludes model/chat concepts cannot trip the
/// check itself.
fn code_only_lines(source: &str) -> Vec<&str> {
    source
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                ""
            } else {
                line
            }
        })
        .collect()
}

#[test]
// Break caught: a later edit wiring Help's availability, next-step, alias, or
// support-state logic to a model port, provider client, or chat type -- which
// would make Help gated on the very capability it exists to remain available
// without (acceptance 1). The scan fails the moment real code, not prose,
// names one.
fn bmad_help_module_source_has_no_model_or_chat_capability_reference() {
    let source = help_source();
    // Scan only the module proper: help.rs's own `#[cfg(test)] mod tests`
    // necessarily names these tokens as string literals in its assertions.
    let module_source = source
        .split_once("#[cfg(test)]")
        .map_or(source.as_str(), |(module, _)| module);
    for (index, line) in code_only_lines(module_source).into_iter().enumerate() {
        assert!(
            !line.contains("model_port") && !line.contains("ModelPort"),
            "help.rs line {} references a model port: {line:?}",
            index + 1,
        );
        assert!(
            !line.contains("crate::chat")
                && !line.contains("super::chat")
                && !line.contains("chat::")
                && !line.contains("ChatSession")
                && !line.contains("ChatTurn")
                && !line.contains("ChatReply"),
            "help.rs line {} references a chat type: {line:?}",
            index + 1,
        );
    }
}

#[test]
// Break caught: the *asymmetry* collapsing into equivalence -- Help's
// existing (already-exported) catalog and advisor surface quietly growing a
// required model/chat input to build or query a catalog, which would make
// even the pre-ENG-11 Help path unavailable offline.
fn bmad_help_existing_catalog_surface_builds_and_answers_without_a_model_value() {
    let package = BmadLoadedPackage {
        package_name: "bmad-method".to_owned(),
        package_version: "6.10.0".to_owned(),
        package_version_id: desktop_runtime::ContractId::new(
            "pkgver_8B33C55A4D67D0B258FEDBB75D1CB09DBC7F711BC9BDC794D8B052B31FCE6D86",
        )
        .expect("package version id"),
        descriptor_hash: desktop_runtime::sha256_bytes(b"descriptor"),
        observed_inventory_hash: desktop_runtime::sha256_bytes(b"inventory"),
        skills: Vec::new(),
    };
    let header = "module,skill,display-name,menu-code,description,action,args,phase,preceded-by,followed-by,required,output-location,outputs";
    let source = BmadHelpCatalogSource::new(
        "bmm",
        format!(
            "{header}\nBMad Method,bmad-product-brief,Product Brief,PB,Draft a brief.,,,1-analysis,,,true,planning_artifacts,brief\n"
        ),
    )
    .expect("catalog source");

    // No model or chat argument appears anywhere in this construction: the
    // catalog and its help actions are fully answerable from source text.
    let catalog = BmadCatalogBuilder::build(&package, &[source]).expect("catalog");
    assert_eq!(catalog.help_actions.len(), 1);
}
