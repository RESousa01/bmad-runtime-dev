#![allow(clippy::expect_used)]

use desktop_runtime::{
    sha256_bytes, BmadPackageRoot, BmadResolvedResource, BmadResourceEdgeDisposition,
    BmadResourceEdgeDispositionKind, BmadResourceInput, BmadResourceRequirement,
    BmadResourceResolver, BmadResourceResolverError,
};

fn package_root() -> BmadPackageRoot {
    BmadPackageRoot::new("method", "bmad-method@6.10.0").expect("pinned package root")
}

fn resource(path: &str, body: &str) -> BmadResourceInput {
    BmadResourceInput::file(path, body.as_bytes()).expect("safe in-package fixture path")
}

fn disposition(
    path: &str,
    body: &str,
    literal: &str,
    kind: BmadResourceEdgeDispositionKind,
) -> BmadResourceEdgeDisposition {
    BmadResourceEdgeDisposition::new(
        "method",
        "bmad-method@6.10.0",
        path,
        sha256_bytes(body.as_bytes()),
        literal,
        kind,
    )
    .expect("exact edge disposition")
}

#[test]
fn bmad_resource_resolution_closes_declared_markdown_frontmatter_toml_yaml_csv_script_and_template_resources(
) {
    // Break caught: removing any supported reference parser or resolving a declaration outside
    // its source directory leaves this mandatory closure incomplete.
    let resolver = BmadResourceResolver::new(
        package_root(),
        vec![
            resource(
                "skills/brief/SKILL.md",
                "---\ntemplate: ./templates/brief.md\nasset: ./assets/runtime-copy.json\n---\n[step](./steps/step.toml)\n",
            ),
            resource(
                "skills/brief/steps/step.toml",
                "include = \"../templates/brief.md\"\nscript = \"../scripts/render.mjs\"\nconfig = \"../config.yaml\"\n",
            ),
            resource("skills/brief/config.yaml", "template: ./templates/brief.md\n"),
            resource(
                "skills/brief/templates/brief.md",
                "{{ include \"../data/fields.csv\" }}\n",
            ),
            resource("skills/brief/data/fields.csv", "name,script\nbrief,../scripts/render.mjs\n"),
            resource(
                "skills/brief/scripts/render.mjs",
                "import \"../assets/runtime.json\";\n",
            ),
            resource("skills/brief/assets/runtime.json", "{\"runtime\":true}\n"),
            resource("skills/brief/assets/runtime-copy.json", "{\"runtime\":true}\n"),
        ],
    )
    .expect("pinned package inventory is valid");

    let closure = resolver
        .resolve_skill("skills/brief/SKILL.md")
        .expect("complete mandatory closure activates");

    assert!(closure.activation_allowed());
    assert!(closure.missing().is_empty());
    assert!(closure.cycles().is_empty());
    assert_eq!(
        closure
            .resources()
            .iter()
            .map(BmadResolvedResource::path)
            .collect::<Vec<_>>(),
        [
            "skills/brief/SKILL.md",
            "skills/brief/assets/runtime-copy.json",
            "skills/brief/assets/runtime.json",
            "skills/brief/config.yaml",
            "skills/brief/data/fields.csv",
            "skills/brief/scripts/render.mjs",
            "skills/brief/steps/step.toml",
            "skills/brief/templates/brief.md",
        ]
    );
    let root = closure.resources().first().expect("skill root");
    assert_eq!(root.source_id(), "method");
    assert_eq!(root.package_identity(), "bmad-method@6.10.0");
    assert_eq!(root.media_type(), "text/markdown");
    assert_eq!(
        root.provenance().requirement(),
        BmadResourceRequirement::Mandatory
    );
    assert_eq!(resolver.cached_content_count(), 7);
}

#[test]
fn bmad_resource_resolution_blocks_activation_for_missing_mandatory_cycle_escape_and_unpinned_remote_edges(
) {
    // Break caught: treating missing or unsafe references as optional lets an incomplete skill
    // activate, bypassing the package-root boundary.
    let resolver = BmadResourceResolver::new(
        package_root(),
        vec![
            resource(
                "skills/unsafe/SKILL.md",
                "[missing](./missing.md)\n[cycle](./steps/a.md)\n[escape](../../../outside.md)\n[remote](https://example.invalid/template.md)\n",
            ),
            resource("skills/unsafe/steps/a.md", "[root](../SKILL.md)\n"),
        ],
    )
    .expect("pinned package inventory is valid");

    let error = resolver
        .resolve_skill("skills/unsafe/SKILL.md")
        .expect_err("incomplete mandatory closure blocks activation");

    let closure = error.closure().expect("closure diagnostics are retained");
    assert!(!closure.activation_allowed());
    assert_eq!(closure.missing(), ["skills/unsafe/missing.md"]);
    assert_eq!(
        closure.cycles(),
        &[vec!["skills/unsafe/SKILL.md", "skills/unsafe/steps/a.md"]]
    );
    assert_eq!(
        closure.invalid_references(),
        ["../../../outside.md", "https://example.invalid/template.md",]
    );
    assert!(matches!(
        error,
        BmadResourceResolverError::IncompleteMandatoryClosure { .. }
    ));
}

#[test]
fn bmad_resource_resolution_retains_optional_edges_without_making_them_activation_blockers() {
    // Break caught: a missing explicitly optional asset either disappears from evidence or blocks
    // an otherwise complete mandatory resource closure.
    let resolver = BmadResourceResolver::new(
        package_root(),
        vec![
            resource(
                "skills/optional/SKILL.md",
                "optional: ./assets/logo.svg\n[required](./steps/ready.md)\n",
            ),
            resource("skills/optional/steps/ready.md", "ready\n"),
        ],
    )
    .expect("pinned package inventory is valid");

    let closure = resolver
        .resolve_skill("skills/optional/SKILL.md")
        .expect("optional edge does not block activation");

    assert!(closure.activation_allowed());
    assert!(closure.missing().is_empty());
    assert_eq!(
        closure.optional_missing(),
        ["skills/optional/assets/logo.svg"]
    );
}

#[test]
fn bmad_resource_resolution_traverses_nested_and_cross_skill_edges_without_seeding_owned_members() {
    // Break caught: pre-seeding a skill with every file hides a broken nested edge and cannot
    // prove the external skill dependency or its declaration provenance.
    let resolver = BmadResourceResolver::new(
        package_root(),
        vec![
            resource("skills/alpha/SKILL.md", "[nested](./steps/one.md)\n"),
            resource(
                "skills/alpha/steps/one.md",
                "[shared](../../beta/templates/brief.md)\n",
            ),
            resource("skills/beta/templates/brief.md", "brief\n"),
            resource("skills/alpha/unreachable.md", "not declared\n"),
        ],
    )
    .expect("inventory");

    let closure = resolver
        .resolve_skill("skills/alpha/SKILL.md")
        .expect("declared graph closes");

    assert_eq!(
        closure
            .resources()
            .iter()
            .map(BmadResolvedResource::path)
            .collect::<Vec<_>>(),
        [
            "skills/alpha/SKILL.md",
            "skills/alpha/steps/one.md",
            "skills/beta/templates/brief.md",
        ]
    );
    let cross_skill = closure.resources().last().expect("cross skill resource");
    assert_eq!(
        cross_skill.provenance().declared_by(),
        Some("skills/alpha/steps/one.md")
    );
}

#[test]
fn bmad_resource_resolution_rejects_drive_device_and_remote_edges_and_prose_optionality() {
    // Break caught: accepting a Windows device/drive/UNC target, an unpinned remote, or treating
    // ordinary prose that contains "optional" as a policy declaration bypasses fail-closed closure.
    let resolver = BmadResourceResolver::new(
        package_root(),
        vec![resource(
            "skills/strict/SKILL.md",
            "[optional reading](./missing.md)\n[drive](C:/escape.md)\n[backslash drive](C:\\escape.md)\n[device](\\\\?\\C:\\escape.md)\n[remote](https://example.invalid/asset.md)\n",
        )],
    )
    .expect("inventory");

    let error = resolver
        .resolve_skill("skills/strict/SKILL.md")
        .expect_err("unsafe or incomplete closure blocks activation");
    let closure = error.closure().expect("diagnostics");
    assert_eq!(closure.missing(), ["skills/strict/missing.md"]);
    assert!(closure.closure_percent() < 100.0);
    assert_eq!(closure.required_mandatory_edges(), 6);
    assert_eq!(closure.closed_mandatory_edges(), 1);
    assert_eq!(
        closure.invalid_references(),
        [
            "C:/escape.md",
            "C:\\escape.md",
            "\\\\?\\C:\\escape.md",
            "https://example.invalid/asset.md"
        ]
    );
}

#[test]
fn bmad_resource_resolution_rejects_case_colliding_inventory_paths() {
    // Break caught: allowing a case-folded collision makes the same logical dependency resolve
    // differently on Windows and case-sensitive hosts.
    let result = BmadResourceResolver::new(
        package_root(),
        vec![
            resource("skills/case/SKILL.md", "ready\n"),
            resource("skills/case/skill.md", "shadow\n"),
        ],
    );
    assert!(matches!(
        result,
        Err(BmadResourceResolverError::CaseCollision)
    ));
}

#[test]
fn bmad_resource_resolution_caches_identical_bytes_without_reusing_the_wrong_parser_kind() {
    // Break caught: caching parsed references only by bytes lets a Markdown member's parser
    // suppress the TOML dependency carried by an identically-byte-for-byte member.
    let resolver = BmadResourceResolver::new(
        package_root(),
        vec![
            resource(
                "skills/cache/SKILL.md",
                "[markdown](./a.md)\n[toml](./b.toml)\n",
            ),
            resource("skills/cache/a.md", "template = \"./nested.md\"\n"),
            resource("skills/cache/b.toml", "template = \"./nested.md\"\n"),
            resource("skills/cache/nested.md", "nested\n"),
        ],
    )
    .expect("inventory");

    let closure = resolver
        .resolve_skill("skills/cache/SKILL.md")
        .expect("the TOML parser reaches the nested resource");
    assert_eq!(resolver.cached_content_count(), 3);
    assert_eq!(resolver.cached_parser_entry_count(), 4);
    assert!(closure
        .resources()
        .iter()
        .any(|resource| resource.path() == "skills/cache/nested.md"));
}

#[test]
fn bmad_resource_resolution_separates_html_and_hbs_parser_cache_semantics() {
    // Break caught: grouping all template extensions lets one parser's cached result suppress a
    // sibling extension's declared edge when their bytes are identical.
    let resolver = BmadResourceResolver::new(
        package_root(),
        vec![
            resource("skills/web/SKILL.md", "[html](./a.html)\n[hbs](./b.hbs)\n"),
            resource("skills/web/a.html", "---\ntemplate: ./nested.md\n---\n"),
            resource("skills/web/b.hbs", "---\ntemplate: ./nested.md\n---\n"),
            resource("skills/web/nested.md", "nested\n"),
        ],
    )
    .expect("inventory");

    let closure = resolver
        .resolve_skill("skills/web/SKILL.md")
        .expect("frontmatter-capable HTML parser reaches nested resource");
    assert_eq!(resolver.cached_content_count(), 3);
    assert_eq!(resolver.cached_parser_entry_count(), 4);
    assert!(closure
        .resources()
        .iter()
        .any(|resource| resource.path() == "skills/web/nested.md"));
}

#[test]
fn bmad_resource_resolution_preserves_exact_generated_and_rejected_dispositions_without_denominator_fabrication(
) {
    // Break caught: counting a generated output or rejected remote authority as a closed local
    // package edge can fabricate 100% closure and incorrectly make the skill invokable.
    let body = "[local](./local.md)\n[output](./report.md)\n[authority](https://example.invalid/design.md)\n";
    let resolver = BmadResourceResolver::with_edge_dispositions(
        package_root(),
        vec![
            resource("skills/mixed/SKILL.md", body),
            resource("skills/mixed/local.md", "local\n"),
        ],
        vec![
            disposition(
                "skills/mixed/SKILL.md",
                body,
                "./report.md",
                BmadResourceEdgeDispositionKind::GeneratedOutput {
                    output_identity: "mixed:report.md".to_owned(),
                },
            ),
            disposition(
                "skills/mixed/SKILL.md",
                body,
                "https://example.invalid/design.md",
                BmadResourceEdgeDispositionKind::ExternalAuthorityRejected {
                    authority_identity: "example:design.md".to_owned(),
                    governance_owner: "MTH-10".to_owned(),
                },
            ),
        ],
    )
    .expect("exact dispositions match the pinned member");

    let error = resolver
        .resolve_skill("skills/mixed/SKILL.md")
        .expect_err("rejected authority remains activation-blocking");
    let closure = error.closure().expect("typed blocker evidence");
    assert_eq!(closure.required_mandatory_edges(), 2);
    assert_eq!(closure.closed_mandatory_edges(), 2);
    assert!((closure.closure_percent() - 100.0).abs() < f64::EPSILON);
    assert!(!closure.activation_allowed());
    assert!(closure.missing().is_empty());
    assert_eq!(closure.activation_blockers().len(), 1);
    assert!(matches!(
        closure.activation_blockers()[0].kind(),
        BmadResourceEdgeDispositionKind::ExternalAuthorityRejected {
            governance_owner,
            ..
        } if governance_owner == "MTH-10"
    ));
    assert!(closure.edge_dispositions().iter().any(|edge| {
        matches!(
            edge.kind(),
            BmadResourceEdgeDispositionKind::GeneratedOutput { output_identity }
                if output_identity == "mixed:report.md"
        )
    }));
}

#[test]
fn bmad_resource_resolution_rejects_disposition_hash_and_literal_drift() {
    // Break caught: a global/path-pattern exception survives source updates and silently changes
    // the meaning of a new package member.
    let body = "[output](./report.md)\n";
    let drifted_hash = BmadResourceEdgeDisposition::new(
        "method",
        "bmad-method@6.10.0",
        "skills/drift/SKILL.md",
        sha256_bytes(b"different bytes"),
        "./report.md",
        BmadResourceEdgeDispositionKind::GeneratedOutput {
            output_identity: "drift:report.md".to_owned(),
        },
    )
    .expect("well-shaped but drifted seal");
    let hash_result = BmadResourceResolver::with_edge_dispositions(
        package_root(),
        vec![resource("skills/drift/SKILL.md", body)],
        vec![drifted_hash],
    );
    assert!(matches!(
        hash_result,
        Err(BmadResourceResolverError::InvalidEdgeDisposition)
    ));

    let literal_result = BmadResourceResolver::with_edge_dispositions(
        package_root(),
        vec![resource("skills/drift/SKILL.md", body)],
        vec![disposition(
            "skills/drift/SKILL.md",
            body,
            "./renamed.md",
            BmadResourceEdgeDispositionKind::GeneratedOutput {
                output_identity: "drift:report.md".to_owned(),
            },
        )],
    );
    assert!(matches!(
        literal_result,
        Err(BmadResourceResolverError::InvalidEdgeDisposition)
    ));
}

#[test]
fn bmad_resource_resolution_keeps_unknown_generated_output_in_mandatory_closure() {
    // Break caught: treating every missing Markdown target as generated output bypasses the exact
    // disposition tuple and removes a real missing dependency from the denominator.
    let resolver = BmadResourceResolver::new(
        package_root(),
        vec![resource(
            "skills/unknown/SKILL.md",
            "[unknown](./unknown.md)\n",
        )],
    )
    .expect("inventory");

    let error = resolver
        .resolve_skill("skills/unknown/SKILL.md")
        .expect_err("unsealed output remains missing");
    let closure = error.closure().expect("missing evidence");
    assert_eq!(closure.missing(), ["skills/unknown/unknown.md"]);
    assert_eq!(closure.required_mandatory_edges(), 2);
    assert_eq!(closure.closed_mandatory_edges(), 1);
    assert!(matches!(
        closure.edge_dispositions()[0].kind(),
        BmadResourceEdgeDispositionKind::PackageResourceMandatory
    ));
}

#[test]
fn bmad_resource_resolution_does_not_reuse_exact_remote_seal_for_nested_member() {
    // Break caught: matching only by literal lets one reviewed remote disposition exempt the same
    // URL wherever it appears in a package.
    let root_body = "[step](./step.md)\n[authority](https://example.invalid/design.md)\n";
    let resolver = BmadResourceResolver::with_edge_dispositions(
        package_root(),
        vec![
            resource("skills/no-bypass/SKILL.md", root_body),
            resource(
                "skills/no-bypass/step.md",
                "[missing](./missing.md)\n[authority](https://example.invalid/design.md)\n",
            ),
        ],
        vec![disposition(
            "skills/no-bypass/SKILL.md",
            root_body,
            "https://example.invalid/design.md",
            BmadResourceEdgeDispositionKind::ExternalAuthorityRejected {
                authority_identity: "example:design.md".to_owned(),
                governance_owner: "MTH-10".to_owned(),
            },
        )],
    )
    .expect("root seal");

    let error = resolver
        .resolve_skill("skills/no-bypass/SKILL.md")
        .expect_err("nested missing and unsealed remote remain blockers");
    let closure = error.closure().expect("nested diagnostics");
    assert_eq!(closure.activation_blockers().len(), 1);
    assert_eq!(closure.missing(), ["skills/no-bypass/missing.md"]);
    assert_eq!(
        closure.invalid_references(),
        ["https://example.invalid/design.md"]
    );
}
