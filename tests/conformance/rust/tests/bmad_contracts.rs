use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

use desktop_runtime::{canonical_hash_without_field, canonical_json_bytes};
use jsonschema::{error::ValidationErrorKind, Draft, Resource, Validator};
use sapphirus_contracts_conformance::{
    validate_bmad_semantics, validate_method_advance_result_semantics,
    validate_method_help_proposal_semantics, validate_method_help_recommendation_semantics,
};
use sapphirus_generator_qualification::{
    ParserLimits, QualificationValidator, ReasonCategory, RejectionStage,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

#[allow(dead_code, clippy::all, clippy::pedantic, clippy::unwrap_used)]
#[path = "../../../../packages/contracts/generated/rust/contracts.rs"]
mod generated_contracts;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureEntry {
    file: String,
    schema: Option<String>,
    valid: bool,
    reason_code: Option<String>,
    #[serde(default)]
    reason_codes: Vec<String>,
    context_file: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenFile {
    vectors: Vec<GoldenVector>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenVector {
    name: String,
    purpose: String,
    schema_major: String,
    excluded_fields: Vec<String>,
    value: Value,
    canonical_json: String,
    expected_hash: String,
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn contract_root() -> PathBuf {
    repository_root().join("packages/contracts")
}

fn load_schemas() -> Result<HashMap<String, Value>, Box<dyn std::error::Error>> {
    let mut schemas = HashMap::new();
    for entry in fs::read_dir(contract_root().join("schemas"))? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.ends_with(".schema.json") {
            schemas.insert(name.to_owned(), serde_json::from_slice(&fs::read(path)?)?);
        }
    }
    Ok(schemas)
}

fn qualification_validator(
    schema_name: &str,
    schemas: &HashMap<String, Value>,
) -> Result<QualificationValidator, Box<dyn std::error::Error>> {
    let root = schemas
        .get(schema_name)
        .ok_or_else(|| format!("missing root schema {schema_name}"))?;
    let resources = schemas
        .iter()
        .filter(|(name, _)| name.as_str() != schema_name)
        .map(|(_, schema)| schema.clone())
        .collect();
    Ok(QualificationValidator::new(
        root,
        resources,
        ParserLimits {
            max_bytes: 2_097_152,
            max_container_depth: 16,
        },
        vec![
            ReasonCategory::UnknownProperty,
            ReasonCategory::RequiredPropertyMissing,
            ReasonCategory::PatternMismatch,
            ReasonCategory::TypeMismatch,
            ReasonCategory::OneOfMismatch,
            ReasonCategory::SchemaInvalid,
        ],
    )?)
}

fn schema_validator(
    schema_name: &str,
    schemas: &HashMap<String, Value>,
) -> Result<Validator, Box<dyn std::error::Error>> {
    let root = schemas
        .get(schema_name)
        .ok_or_else(|| format!("missing root schema {schema_name}"))?;
    let mut options = jsonschema::options().with_draft(Draft::Draft202012);
    for (name, schema) in schemas {
        if name == schema_name {
            continue;
        }
        let identifier = schema
            .get("$id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("schema {name} has no $id"))?;
        options = options.with_resource(
            identifier.to_owned(),
            Resource::from_contents(schema.clone()),
        );
    }
    Ok(options.build(root)?)
}

fn normalized_schema_reason(validator: &Validator, value: &Value) -> Option<&'static str> {
    let reasons: HashSet<_> = validator
        .iter_errors(value)
        .map(|error| match error.kind() {
            ValidationErrorKind::Constant { .. } => "CONST_MISMATCH",
            ValidationErrorKind::Enum { .. } => "ENUM_MISMATCH",
            ValidationErrorKind::AdditionalProperties { .. }
            | ValidationErrorKind::UnevaluatedProperties { .. } => "UNKNOWN_PROPERTY",
            ValidationErrorKind::Required { .. } => "REQUIRED_PROPERTY_MISSING",
            ValidationErrorKind::Type { .. } => "TYPE_MISMATCH",
            ValidationErrorKind::Pattern { .. } => "PATTERN_MISMATCH",
            ValidationErrorKind::MinItems { .. } => "ARRAY_TOO_SHORT",
            ValidationErrorKind::MaxItems { .. } | ValidationErrorKind::AdditionalItems { .. } => {
                "ARRAY_TOO_LONG"
            }
            ValidationErrorKind::OneOfMultipleValid { .. }
            | ValidationErrorKind::OneOfNotValid { .. } => "ONE_OF_MISMATCH",
            _ => "SCHEMA_INVALID",
        })
        .collect();
    [
        "ONE_OF_MISMATCH",
        "ARRAY_TOO_SHORT",
        "ARRAY_TOO_LONG",
        "UNKNOWN_PROPERTY",
        "REQUIRED_PROPERTY_MISSING",
        "CONST_MISMATCH",
        "ENUM_MISMATCH",
        "PATTERN_MISMATCH",
        "TYPE_MISMATCH",
        "SCHEMA_INVALID",
    ]
    .into_iter()
    .find(|reason| reasons.contains(reason))
}

fn assert_generated_round_trip<T>(
    source: &[u8],
    expected: &Value,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: DeserializeOwned + Serialize,
{
    let typed: T = serde_json::from_slice(source)?;
    let round_trip = serde_json::to_value(typed)?;
    assert_eq!(&round_trip, expected);
    Ok(())
}

#[test]
fn every_bmad_fixture_has_the_same_rust_reason_category() -> Result<(), Box<dyn std::error::Error>>
{
    let schemas = load_schemas()?;
    let fixture_root = contract_root().join("fixtures");
    let catalog: Vec<FixtureEntry> =
        serde_json::from_slice(&fs::read(fixture_root.join("catalog.json"))?)?;
    let entries: Vec<_> = catalog
        .into_iter()
        .filter(|entry| entry.file.contains("/bmad/"))
        .collect();
    assert_eq!(entries.len(), 103);

    for entry in entries {
        let source = fs::read(fixture_root.join(&entry.file))?;
        if entry.reason_code.as_deref() == Some("DUPLICATE_MEMBER") {
            let arbitrary_root = "bmad-package-descriptor.schema.json";
            let result =
                qualification_validator(arbitrary_root, &schemas)?.validate_source(&source);
            assert_eq!(
                result.reason_category,
                Some(ReasonCategory::DuplicateMember)
            );
            assert_eq!(entry.reason_codes, ["DUPLICATE_MEMBER"]);
            assert_eq!(result.rejection_stage, RejectionStage::StrictParser);
            assert!(!result.validator_invoked);
            continue;
        }

        let schema_name = entry
            .schema
            .as_deref()
            .ok_or("fixture schema is required")?;
        let qualification =
            qualification_validator(schema_name, &schemas)?.validate_source(&source);
        let value: Value = serde_json::from_slice(&source)?;
        if !qualification.accepted {
            assert!(!entry.valid, "{} was structurally rejected", entry.file);
            let reason =
                normalized_schema_reason(&schema_validator(schema_name, &schemas)?, &value);
            assert_eq!(reason, entry.reason_code.as_deref(), "{}", entry.file);
            assert_eq!(
                reason.into_iter().collect::<Vec<_>>(),
                entry.reason_codes,
                "{}",
                entry.file
            );
            continue;
        }

        let descriptor = if let Some(path) = entry.context_file.as_ref() {
            Some(serde_json::from_slice(&fs::read(fixture_root.join(path))?)?)
        } else {
            None
        };
        let semantic = validate_bmad_semantics(&value, descriptor.as_ref());
        if entry.valid {
            assert!(semantic.is_empty(), "{}: {semantic:?}", entry.file);
        } else {
            assert_eq!(semantic, entry.reason_codes, "{}", entry.file);
        }
    }
    Ok(())
}

#[test]
fn rust_matches_all_eight_bmad_golden_hash_vectors() -> Result<(), Box<dyn std::error::Error>> {
    let golden: GoldenFile = serde_json::from_slice(&fs::read(
        contract_root().join("fixtures/golden/bmad/hash-vectors.json"),
    )?)?;
    assert_eq!(golden.vectors.len(), 8);
    for vector in golden.vectors {
        let schema_major = vector
            .schema_major
            .strip_prefix('v')
            .ok_or("invalid schema major")?
            .parse::<u32>()?;
        let field = vector
            .excluded_fields
            .first()
            .ok_or("missing excluded field")?;
        let hash =
            canonical_hash_without_field(&vector.purpose, schema_major, &vector.value, field)?;
        assert_eq!(hash.to_string(), vector.expected_hash, "{}", vector.name);
        let mut without_self = vector.value.clone();
        without_self
            .as_object_mut()
            .ok_or("golden value must be an object")?
            .remove(field);
        assert_eq!(
            String::from_utf8(canonical_json_bytes(&without_self)?)?,
            vector.canonical_json,
            "{}",
            vector.name
        );

        let mut excluded_mutation = vector.value.clone();
        excluded_mutation[field] = Value::String(format!("sha256:{}", "f".repeat(64)));
        assert_eq!(
            canonical_hash_without_field(
                &vector.purpose,
                schema_major,
                &excluded_mutation,
                field,
            )?
            .to_string(),
            vector.expected_hash,
            "{} excluded mutation",
            vector.name,
        );
        let mut included_mutation = vector.value.clone();
        included_mutation["schemaVersion"] = Value::String("transplanted.v1".to_owned());
        assert_ne!(
            canonical_hash_without_field(
                &vector.purpose,
                schema_major,
                &included_mutation,
                field,
            )?
            .to_string(),
            vector.expected_hash,
            "{} included mutation",
            vector.name,
        );
    }
    Ok(())
}

#[test]
fn every_valid_bmad_root_round_trips_through_generated_rust_types(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture_root = contract_root().join("fixtures");
    let entries: Vec<FixtureEntry> =
        serde_json::from_slice(&fs::read(fixture_root.join("catalog.json"))?)?;
    for entry in entries
        .into_iter()
        .filter(|entry| entry.valid && entry.file.contains("/bmad/"))
    {
        let source = fs::read(fixture_root.join(&entry.file))?;
        let value: Value = serde_json::from_slice(&source)?;
        let result = match entry.schema.as_deref().ok_or("missing schema")? {
            "bmad-package-descriptor.schema.json" => assert_generated_round_trip::<
                generated_contracts::BmadPackageDescriptor,
            >(&source, &value),
            "bmad-capability-catalog.schema.json" => assert_generated_round_trip::<
                generated_contracts::BmadCapabilityCatalog,
            >(&source, &value),
            "bmad-method-advance-result.schema.json" => assert_generated_round_trip::<
                generated_contracts::MethodAdvanceResult,
            >(&source, &value),
            "bmad-method-help-proposal.schema.json" => assert_generated_round_trip::<
                generated_contracts::MethodHelpProposal,
            >(&source, &value),
            "bmad-method-help-recommendation.schema.json" => assert_generated_round_trip::<
                generated_contracts::MethodHelpRecommendation,
            >(&source, &value),
            "bmad-method-session.schema.json" => {
                assert_generated_round_trip::<generated_contracts::MethodSession>(&source, &value)
            }
            "bmad-builder-authoring.schema.json" => assert_generated_round_trip::<
                generated_contracts::BuilderAuthoringObject,
            >(&source, &value),
            "bmad-validation-report.schema.json" => assert_generated_round_trip::<
                generated_contracts::BmadValidationReport,
            >(&source, &value),
            schema => return Err(format!("unsupported BMAD schema {schema}").into()),
        };
        result.map_err(|error| format!("{}: {error}", entry.file))?;
    }
    Ok(())
}

#[test]
fn rust_sealed_help_semantics_reject_unsafe_text_hash_drift_and_invalid_instants(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture_root = contract_root().join("fixtures/valid/bmad");
    let proposal: Value =
        serde_json::from_slice(&fs::read(fixture_root.join("method-help-proposal.json"))?)?;
    let recommendation: Value = serde_json::from_slice(&fs::read(
        fixture_root.join("method-help-recommendation.json"),
    )?)?;
    let advance_result: Value =
        serde_json::from_slice(&fs::read(fixture_root.join("method-advance-result.json"))?)?;
    assert!(validate_method_help_proposal_semantics(&proposal).is_empty());
    assert!(validate_method_help_recommendation_semantics(&recommendation).is_empty());
    assert!(validate_method_advance_result_semantics(&advance_result).is_empty());

    let mut unsafe_proposal = proposal;
    unsafe_proposal["rationaleSummary"] = Value::String("unsafe\u{202e}text".to_owned());
    assert_eq!(
        validate_method_help_proposal_semantics(&unsafe_proposal),
        ["BMAD_UNSAFE_TEXT"]
    );

    let mut invalid_recommendation = recommendation;
    invalid_recommendation["createdAt"] = Value::String("2026-02-31T10:00:00.000Z".to_owned());
    assert_eq!(
        validate_method_help_recommendation_semantics(&invalid_recommendation),
        ["HASH_MISMATCH", "INVALID_UTC_INSTANT"]
    );
    invalid_recommendation["createdAt"] = Value::String("0000-02-29T10:00:00.000Z".to_owned());
    assert!(
        !validate_method_help_recommendation_semantics(&invalid_recommendation)
            .iter()
            .any(|code| code == "INVALID_UTC_INSTANT")
    );

    let mut invalid_advance = advance_result;
    invalid_advance["resultKind"] = Value::String("refusal".to_owned());
    invalid_advance["safeMessage"] = Value::String("unsafe\u{2069}text".to_owned());
    assert_eq!(
        validate_method_advance_result_semantics(&invalid_advance),
        ["BMAD_UNSAFE_TEXT", "HASH_MISMATCH"]
    );
    Ok(())
}

#[test]
fn bmad_strict_parser_enforces_exact_byte_and_depth_limits(
) -> Result<(), Box<dyn std::error::Error>> {
    let schemas = load_schemas()?;
    let validator = qualification_validator("bmad-package-descriptor.schema.json", &schemas)?;

    let exact_limit = format!("\"{}\"", "a".repeat(2_097_150));
    assert_eq!(exact_limit.len(), 2_097_152);
    assert!(
        validator
            .validate_source(exact_limit.as_bytes())
            .validator_invoked
    );

    let multibyte_over_limit = format!("\"{}\"", "é".repeat(1_048_576));
    let bytes_result = validator.validate_source(multibyte_over_limit.as_bytes());
    assert_eq!(
        bytes_result.reason_category,
        Some(ReasonCategory::MaxBytesExceeded)
    );
    assert!(!bytes_result.validator_invoked);

    let depth_sixteen = format!("{}null{}", "[".repeat(16), "]".repeat(16));
    assert!(
        validator
            .validate_source(depth_sixteen.as_bytes())
            .validator_invoked
    );
    let depth_seventeen = format!("{}null{}", "[".repeat(17), "]".repeat(17));
    let depth_result = validator.validate_source(depth_seventeen.as_bytes());
    assert_eq!(
        depth_result.reason_category,
        Some(ReasonCategory::MaxDepthExceeded)
    );
    assert!(!depth_result.validator_invoked);
    Ok(())
}
