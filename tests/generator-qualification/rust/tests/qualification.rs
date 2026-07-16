use std::{fs, path::PathBuf};

use desktop_runtime::{canonical_hash, canonical_json_bytes};
use sapphirus_generator_qualification::{
    ParserLimits, QualificationValidator, ReasonCategory, RejectionStage,
};
use serde::Deserialize;
use serde_json::Value;

#[allow(dead_code, clippy::all, clippy::pedantic, clippy::unwrap_used)]
#[path = "../../../../packages/contracts/generated/rust/contracts.rs"]
mod generated_production;

#[allow(dead_code, clippy::all, clippy::pedantic, clippy::unwrap_used)]
#[path = "../../generated/rust/qualification.rs"]
mod generated_qualification;

const PURPOSE: &str = "contract-object";
const SCHEMA_MAJOR: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    root_schema: String,
    resources: Vec<String>,
    parser_limits: ParserLimits,
    reason_priority: Vec<ReasonCategory>,
    fixtures: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    id: String,
    file: String,
    expected: Expected,
    reason_category: Option<ReasonCategory>,
    rejection_stage: RejectionStage,
    validator_invoked: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Accept,
    Reject,
}

#[derive(Debug, Deserialize)]
struct HashVectors {
    required: Vec<HashVector>,
    supplemental: Vec<HashVector>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HashVector {
    purpose: String,
    schema_major: String,
    value: Value,
    canonical_json: String,
    expected_hash: String,
}

fn qualification_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn repository_root() -> PathBuf {
    qualification_root().join("../..")
}

fn read_json(relative_path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read(
        qualification_root().join(relative_path),
    )?)?)
}

fn load_catalog() -> Result<Catalog, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read(
        qualification_root().join("catalog.json"),
    )?)?)
}

fn load_validator(catalog: &Catalog) -> Result<QualificationValidator, Box<dyn std::error::Error>> {
    let root_schema = read_json(&catalog.root_schema)?;
    let resources = catalog
        .resources
        .iter()
        .map(|resource| read_json(resource))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(QualificationValidator::new(
        &root_schema,
        resources,
        catalog.parser_limits,
        catalog.reason_priority.clone(),
    )?)
}

#[test]
fn unregistered_external_schema_resources_fail_closed() -> Result<(), Box<dyn std::error::Error>> {
    let external_schema_path = std::env::temp_dir().join(format!(
        "sapphirus-unregistered-schema-{}.json",
        std::process::id()
    ));
    fs::write(&external_schema_path, br#"{"type":"object"}"#)?;

    let external_uri = format!(
        "file:///{}",
        external_schema_path.to_string_lossy().replace('\\', "/")
    );
    let root_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": external_uri,
    });
    let result = QualificationValidator::new(
        &root_schema,
        Vec::new(),
        ParserLimits {
            max_bytes: 1_024,
            max_container_depth: 8,
        },
        vec![ReasonCategory::SchemaInvalid],
    );
    fs::remove_file(external_schema_path)?;

    assert!(
        result.is_err(),
        "the qualification validator must not retrieve an unregistered file URI"
    );
    Ok(())
}

#[test]
fn strict_parser_failures_never_invoke_jsonschema() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = load_catalog()?;
    let validator = load_validator(&catalog)?;

    for (source, expected_reason) in [
        (
            br#"{"value":1,"value":2}"#.as_slice(),
            ReasonCategory::DuplicateMember,
        ),
        (
            br"[[[[[[[[[null]]]]]]]]]".as_slice(),
            ReasonCategory::MaxDepthExceeded,
        ),
        (br#""\ud800""#.as_slice(), ReasonCategory::InvalidUnicode),
        (
            br"9007199254740992".as_slice(),
            ReasonCategory::IntegerOutOfRange,
        ),
        (
            br"9007199254740991.1".as_slice(),
            ReasonCategory::IntegerOutOfRange,
        ),
        (br"1e309".as_slice(), ReasonCategory::SchemaInvalid),
        (&[0xff][..], ReasonCategory::InvalidUnicode),
    ] {
        let result = validator.validate_source(source);
        assert!(!result.accepted);
        assert_eq!(result.reason_category, Some(expected_reason));
        assert_eq!(result.rejection_stage, RejectionStage::StrictParser);
        assert!(!result.validator_invoked);
    }

    Ok(())
}

#[test]
fn every_catalog_fixture_matches_the_rust_lane() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = load_catalog()?;
    let validator = load_validator(&catalog)?;
    assert_eq!(catalog.fixtures.len(), 25);

    for fixture in &catalog.fixtures {
        let source = fs::read(qualification_root().join(&fixture.file))?;
        let result = validator.validate_source(&source);
        assert_eq!(
            result.accepted,
            fixture.expected == Expected::Accept,
            "{}",
            fixture.id
        );
        assert_eq!(
            result.reason_category, fixture.reason_category,
            "{}",
            fixture.id
        );
        assert_eq!(
            result.rejection_stage, fixture.rejection_stage,
            "{}",
            fixture.id
        );
        assert_eq!(
            result.validator_invoked, fixture.validator_invoked,
            "{}",
            fixture.id
        );

        if fixture.expected == Expected::Accept {
            let parsed: Value = serde_json::from_slice(&source)?;
            let canonical_before = canonical_json_bytes(&parsed)?;
            let generated: generated_qualification::GeneratorQualification =
                serde_json::from_slice(&source)?;
            let generated_json = serde_json::to_vec(&generated)?;
            let reparsed: Value = serde_json::from_slice(&generated_json)?;
            assert_eq!(
                canonical_json_bytes(&reparsed)?,
                canonical_before,
                "{}",
                fixture.id
            );
            assert_eq!(
                canonical_hash(PURPOSE, SCHEMA_MAJOR, &reparsed)?,
                canonical_hash(PURPOSE, SCHEMA_MAJOR, &parsed)?,
                "{}",
                fixture.id,
            );
            let generated_result = validator.validate_source(&generated_json);
            assert!(
                generated_result.accepted,
                "{} generated round trip",
                fixture.id
            );

            if fixture.id == "text-null-empty" {
                assert!(reparsed.get("optionalValue").is_none());
            }
            if fixture.id == "count-optional-null" {
                assert_eq!(reparsed.get("optionalValue"), Some(&Value::Null));
            }
        }
    }

    Ok(())
}

#[test]
fn production_typify_tree_deserializes_a_real_contract() -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read(
        repository_root().join("packages/contracts/fixtures/valid/spec-consumption.json"),
    )?;
    let expected: Value = serde_json::from_slice(&source)?;
    let generated: generated_production::SpecConsumptionRecord = serde_json::from_slice(&source)?;
    let serialized: Value = serde_json::to_value(generated)?;

    assert_eq!(
        canonical_json_bytes(&serialized)?,
        canonical_json_bytes(&expected)?
    );
    Ok(())
}

#[test]
fn shared_canonical_hash_vectors_match_desktop_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let path = repository_root().join("packages/contracts/fixtures/golden/hash-vectors.json");
    let vectors: HashVectors = serde_json::from_slice(&fs::read(path)?)?;

    for vector in vectors.required.into_iter().chain(vectors.supplemental) {
        let schema_major = vector
            .schema_major
            .strip_prefix('v')
            .ok_or("schema major must use vN form")?
            .parse::<u32>()?;
        assert_eq!(
            String::from_utf8(canonical_json_bytes(&vector.value)?)?,
            vector.canonical_json,
        );
        assert_eq!(
            canonical_hash(&vector.purpose, schema_major, &vector.value)?.to_string(),
            vector.expected_hash,
        );
    }

    Ok(())
}
