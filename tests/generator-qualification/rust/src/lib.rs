#![forbid(unsafe_code)]

//! Rust-side qualification for the repository's generated JSON contracts.

use std::{cell::Cell, collections::HashSet, fmt};

use jsonschema::{
    error::{ValidationError, ValidationErrorKind},
    Draft, Resource, Retrieve, Uri, Validator,
};
use serde::{
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
    Deserialize,
};
use serde_json::{Map, Number, Value};

const MAX_INTEROPERABLE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_INTEROPERABLE_INTEGER_F64: f64 = 9_007_199_254_740_991.0;

/// Limits enforced before a source document reaches JSON Schema validation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParserLimits {
    pub max_bytes: usize,
    pub max_container_depth: usize,
}

/// Repository-owned, cross-language validation reasons.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReasonCategory {
    DuplicateMember,
    MaxDepthExceeded,
    MaxBytesExceeded,
    InvalidUnicode,
    IntegerOutOfRange,
    UnknownDiscriminator,
    UnknownProperty,
    RequiredPropertyMissing,
    TypeMismatch,
    NumberTooSmall,
    NumberTooLarge,
    PatternMismatch,
    OneOfMismatch,
    SchemaInvalid,
}

impl ReasonCategory {
    const fn as_str(self) -> &'static str {
        match self {
            Self::DuplicateMember => "DUPLICATE_MEMBER",
            Self::MaxDepthExceeded => "MAX_DEPTH_EXCEEDED",
            Self::MaxBytesExceeded => "MAX_BYTES_EXCEEDED",
            Self::InvalidUnicode => "INVALID_UNICODE",
            Self::IntegerOutOfRange => "INTEGER_OUT_OF_RANGE",
            Self::UnknownDiscriminator => "UNKNOWN_DISCRIMINATOR",
            Self::UnknownProperty => "UNKNOWN_PROPERTY",
            Self::RequiredPropertyMissing => "REQUIRED_PROPERTY_MISSING",
            Self::TypeMismatch => "TYPE_MISMATCH",
            Self::NumberTooSmall => "NUMBER_TOO_SMALL",
            Self::NumberTooLarge => "NUMBER_TOO_LARGE",
            Self::PatternMismatch => "PATTERN_MISMATCH",
            Self::OneOfMismatch => "ONE_OF_MISMATCH",
            Self::SchemaInvalid => "SCHEMA_INVALID",
        }
    }
}

impl fmt::Display for ReasonCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The authority that rejected a qualification document.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RejectionStage {
    None,
    StrictParser,
    StructuralValidator,
}

/// Stable result returned by the Rust qualification lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualificationResult {
    pub accepted: bool,
    pub reason_category: Option<ReasonCategory>,
    pub rejection_stage: RejectionStage,
    pub validator_invoked: bool,
}

/// Failure to construct the pinned structural validator.
#[derive(Debug)]
pub struct QualificationBuildError(String);

impl fmt::Display for QualificationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for QualificationBuildError {}

/// Strict-parser and `jsonschema` 0.44.1 qualification adapter.
pub struct QualificationValidator {
    validator: Validator,
    parser_limits: ParserLimits,
    reason_priority: Vec<ReasonCategory>,
}

#[derive(Clone, Copy, Debug)]
struct ExplicitResourcesOnly;

impl Retrieve for ExplicitResourcesOnly {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("unregistered JSON Schema resource: {uri}"),
        )
        .into())
    }
}

impl QualificationValidator {
    /// Builds a Draft 2020-12 validator from only the explicitly supplied resources.
    ///
    /// # Errors
    ///
    /// Returns an error when a resource lacks `$id`, parser limits are invalid, or
    /// the schema graph cannot be compiled by the pinned validator.
    pub fn new(
        root_schema: &Value,
        resources: Vec<Value>,
        parser_limits: ParserLimits,
        reason_priority: Vec<ReasonCategory>,
    ) -> Result<Self, QualificationBuildError> {
        if parser_limits.max_container_depth == 0 {
            return Err(QualificationBuildError(
                "maxContainerDepth must be at least one".to_owned(),
            ));
        }
        if reason_priority.is_empty() {
            return Err(QualificationBuildError(
                "reasonPriority must not be empty".to_owned(),
            ));
        }

        let mut options = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .with_retriever(ExplicitResourcesOnly);
        for resource in resources {
            let identifier = resource
                .get("$id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    QualificationBuildError(
                        "qualification resource must declare a string $id".to_owned(),
                    )
                })?
                .to_owned();
            options = options.with_resource(identifier, Resource::from_contents(resource));
        }
        let validator = options
            .build(root_schema)
            .map_err(|error| QualificationBuildError(error.to_string()))?;

        Ok(Self {
            validator,
            parser_limits,
            reason_priority,
        })
    }

    /// Strictly parses and then structurally validates one UTF-8 JSON document.
    #[must_use]
    pub fn validate_source(&self, source: &[u8]) -> QualificationResult {
        let value = match parse_strict_json(source, self.parser_limits) {
            Ok(value) => value,
            Err(reason) => {
                return QualificationResult {
                    accepted: false,
                    reason_category: Some(reason),
                    rejection_stage: RejectionStage::StrictParser,
                    validator_invoked: false,
                };
            }
        };

        let issues: Vec<_> = self.validator.iter_errors(&value).collect();
        if issues.is_empty() {
            return QualificationResult {
                accepted: true,
                reason_category: None,
                rejection_stage: RejectionStage::None,
                validator_invoked: true,
            };
        }

        let reason = discriminator_reason(&value).unwrap_or_else(|| {
            let mut candidates = Vec::new();
            for issue in &issues {
                collect_issue_candidates(issue, &mut candidates);
            }
            candidates
                .into_iter()
                .min_by(|left, right| {
                    self.priority(left.0)
                        .cmp(&self.priority(right.0))
                        .then_with(|| left.1.cmp(&right.1))
                })
                .map_or(ReasonCategory::SchemaInvalid, |issue| issue.0)
        });

        QualificationResult {
            accepted: false,
            reason_category: Some(reason),
            rejection_stage: RejectionStage::StructuralValidator,
            validator_invoked: true,
        }
    }

    fn priority(&self, reason: ReasonCategory) -> usize {
        self.reason_priority
            .iter()
            .position(|candidate| *candidate == reason)
            .unwrap_or(usize::MAX)
    }
}

fn collect_issue_candidates(
    issue: &ValidationError<'_>,
    candidates: &mut Vec<(ReasonCategory, String)>,
) {
    candidates.push((
        map_validation_error(issue.kind()),
        issue.instance_path().to_string(),
    ));
    match issue.kind() {
        ValidationErrorKind::AnyOf { context }
        | ValidationErrorKind::OneOfMultipleValid { context }
        | ValidationErrorKind::OneOfNotValid { context } => {
            for branch in context {
                for nested in branch {
                    collect_issue_candidates(nested, candidates);
                }
            }
        }
        ValidationErrorKind::PropertyNames { error } => {
            collect_issue_candidates(error, candidates);
        }
        _ => {}
    }
}

fn discriminator_reason(value: &Value) -> Option<ReasonCategory> {
    let variant = value.get("variant")?.as_object()?;
    let kind = variant.get("kind")?.as_str()?;
    if kind != "text" && kind != "count" {
        return Some(ReasonCategory::UnknownDiscriminator);
    }
    if (kind == "text" && variant.contains_key("count"))
        || (kind == "count" && variant.contains_key("text"))
    {
        return Some(ReasonCategory::OneOfMismatch);
    }
    None
}

fn map_validation_error(kind: &ValidationErrorKind) -> ReasonCategory {
    match kind {
        ValidationErrorKind::AdditionalProperties { .. }
        | ValidationErrorKind::UnevaluatedProperties { .. } => ReasonCategory::UnknownProperty,
        ValidationErrorKind::Required { .. } => ReasonCategory::RequiredPropertyMissing,
        ValidationErrorKind::Type { .. } => ReasonCategory::TypeMismatch,
        ValidationErrorKind::Minimum { .. } | ValidationErrorKind::ExclusiveMinimum { .. } => {
            ReasonCategory::NumberTooSmall
        }
        ValidationErrorKind::Maximum { .. } | ValidationErrorKind::ExclusiveMaximum { .. } => {
            ReasonCategory::NumberTooLarge
        }
        ValidationErrorKind::Pattern { .. } => ReasonCategory::PatternMismatch,
        ValidationErrorKind::OneOfMultipleValid { .. }
        | ValidationErrorKind::OneOfNotValid { .. } => ReasonCategory::OneOfMismatch,
        _ => ReasonCategory::SchemaInvalid,
    }
}

fn parse_strict_json(source: &[u8], limits: ParserLimits) -> Result<Value, ReasonCategory> {
    if source.len() > limits.max_bytes {
        return Err(ReasonCategory::MaxBytesExceeded);
    }
    let text = std::str::from_utf8(source).map_err(|_| ReasonCategory::InvalidUnicode)?;
    if contains_unpaired_surrogate_escape(text.as_bytes()) {
        return Err(ReasonCategory::InvalidUnicode);
    }
    reject_non_interoperable_number_lexemes(text)?;

    let state = ParserState {
        reason: Cell::new(None),
        max_container_depth: limits.max_container_depth,
    };
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = StrictSeed {
        state: &state,
        depth: 0,
    }
    .deserialize(&mut deserializer)
    .map_err(|_| state.reason.get().unwrap_or(ReasonCategory::SchemaInvalid))?;
    deserializer
        .end()
        .map_err(|_| ReasonCategory::SchemaInvalid)?;
    Ok(value)
}

#[derive(Debug, Eq, PartialEq)]
struct NormalizedDecimal {
    negative: bool,
    digits: String,
    decimal_exponent: i64,
}

fn reject_non_interoperable_number_lexemes(text: &str) -> Result<(), ReasonCategory> {
    let source = text.as_bytes();
    let mut index = 0;
    let mut inside_string = false;

    while index < source.len() {
        if inside_string {
            match source[index] {
                b'\\' => index = index.saturating_add(2),
                b'"' => {
                    inside_string = false;
                    index += 1;
                }
                _ => index += 1,
            }
            continue;
        }

        match source[index] {
            b'"' => {
                inside_string = true;
                index += 1;
            }
            b'-' | b'0'..=b'9' => {
                let Some(end) = json_number_end(source, index) else {
                    index += 1;
                    continue;
                };
                if end == source.len() || is_json_value_delimiter(source[end]) {
                    validate_number_lexeme(&text[index..end])?;
                }
                index = end;
            }
            _ => index += 1,
        }
    }

    Ok(())
}

fn json_number_end(source: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    if source.get(index) == Some(&b'-') {
        index += 1;
    }

    match source.get(index)? {
        b'0' => index += 1,
        b'1'..=b'9' => {
            index += 1;
            while matches!(source.get(index), Some(b'0'..=b'9')) {
                index += 1;
            }
        }
        _ => return None,
    }

    if source.get(index) == Some(&b'.') {
        index += 1;
        if !matches!(source.get(index), Some(b'0'..=b'9')) {
            return None;
        }
        while matches!(source.get(index), Some(b'0'..=b'9')) {
            index += 1;
        }
    }

    if matches!(source.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(source.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        if !matches!(source.get(index), Some(b'0'..=b'9')) {
            return None;
        }
        while matches!(source.get(index), Some(b'0'..=b'9')) {
            index += 1;
        }
    }

    Some(index)
}

const fn is_json_value_delimiter(token: u8) -> bool {
    matches!(token, b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}')
}

fn validate_number_lexeme(token: &str) -> Result<(), ReasonCategory> {
    let value = token
        .parse::<f64>()
        .map_err(|_| ReasonCategory::SchemaInvalid)?;
    if !value.is_finite() {
        return Err(ReasonCategory::SchemaInvalid);
    }
    let source = normalize_decimal_lexeme(token);
    let represented = normalize_decimal_lexeme(&value.to_string());

    if (value.fract() == 0.0 && value.abs() > MAX_INTEROPERABLE_INTEGER_F64)
        || source.is_none()
        || source != represented
    {
        return Err(ReasonCategory::IntegerOutOfRange);
    }

    Ok(())
}

fn normalize_decimal_lexeme(token: &str) -> Option<NormalizedDecimal> {
    let (negative, unsigned) = token
        .strip_prefix('-')
        .map_or((false, token), |value| (true, value));
    let exponent_index = unsigned.find(['e', 'E']);
    let (mantissa, exponent_token) = exponent_index.map_or((unsigned, "0"), |index| {
        (&unsigned[..index], &unsigned[index + 1..])
    });
    let (whole, fraction) = mantissa
        .split_once('.')
        .map_or((mantissa, ""), |parts| parts);
    let combined = format!("{whole}{fraction}");
    let significant = combined.trim_start_matches('0');
    if significant.is_empty() {
        return Some(NormalizedDecimal {
            negative: false,
            digits: "0".to_owned(),
            decimal_exponent: 0,
        });
    }

    let exponent = parse_decimal_exponent(exponent_token)?;
    let fraction_length = i64::try_from(fraction.len()).ok()?;
    let trailing_length = significant.len() - significant.trim_end_matches('0').len();
    let trailing_zeros = i64::try_from(trailing_length).ok()?;
    let decimal_exponent = exponent
        .checked_sub(fraction_length)?
        .checked_add(trailing_zeros)?;
    let digits = significant[..significant.len() - trailing_length].to_owned();

    Some(NormalizedDecimal {
        negative,
        digits,
        decimal_exponent,
    })
}

fn parse_decimal_exponent(token: &str) -> Option<i64> {
    if let Some(magnitude) = token.strip_prefix('-') {
        magnitude.parse::<i64>().ok()?.checked_neg()
    } else {
        token.strip_prefix('+').unwrap_or(token).parse::<i64>().ok()
    }
}

fn contains_unpaired_surrogate_escape(source: &[u8]) -> bool {
    let mut index = 0;
    let mut in_string = false;
    while index < source.len() {
        match source[index] {
            b'"' => {
                in_string = !in_string;
                index += 1;
            }
            b'\\' if in_string => {
                if source.get(index + 1) != Some(&b'u') {
                    index = index.saturating_add(2);
                    continue;
                }
                let Some(code_unit) = decode_unicode_escape(source, index) else {
                    index += 1;
                    continue;
                };
                if (0xd800..=0xdbff).contains(&code_unit) {
                    let pair_index = index + 6;
                    let Some(pair) = decode_unicode_escape(source, pair_index) else {
                        return true;
                    };
                    if !(0xdc00..=0xdfff).contains(&pair) {
                        return true;
                    }
                    index = pair_index + 6;
                } else if (0xdc00..=0xdfff).contains(&code_unit) {
                    return true;
                } else {
                    index += 6;
                }
            }
            _ => index += 1,
        }
    }
    false
}

fn decode_unicode_escape(source: &[u8], index: usize) -> Option<u16> {
    if source.get(index..index + 2)? != b"\\u" {
        return None;
    }
    let digits = source.get(index + 2..index + 6)?;
    let mut value = 0_u16;
    for digit in digits {
        value = value.checked_mul(16)?;
        value = value.checked_add(u16::from(match digit {
            b'0'..=b'9' => digit - b'0',
            b'a'..=b'f' => digit - b'a' + 10,
            b'A'..=b'F' => digit - b'A' + 10,
            _ => return None,
        }))?;
    }
    Some(value)
}

struct ParserState {
    reason: Cell<Option<ReasonCategory>>,
    max_container_depth: usize,
}

struct StrictSeed<'a> {
    state: &'a ParserState,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for StrictSeed<'_> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictVisitor {
            state: self.state,
            depth: self.depth,
        })
    }
}

struct StrictVisitor<'a> {
    state: &'a ParserState,
    depth: usize,
}

impl StrictVisitor<'_> {
    fn fail<E>(&self, reason: ReasonCategory) -> E
    where
        E: de::Error,
    {
        self.state.reason.set(Some(reason));
        E::custom(reason.as_str())
    }

    fn container_depth<E>(&self) -> Result<usize, E>
    where
        E: de::Error,
    {
        let depth = self.depth + 1;
        if depth > self.state.max_container_depth {
            return Err(self.fail(ReasonCategory::MaxDepthExceeded));
        }
        Ok(depth)
    }

    fn child(&self, depth: usize) -> StrictSeed<'_> {
        StrictSeed {
            state: self.state,
            depth,
        }
    }
}

impl<'de> Visitor<'de> for StrictVisitor<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a strict JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.unsigned_abs() > MAX_INTEROPERABLE_INTEGER {
            return Err(self.fail(ReasonCategory::IntegerOutOfRange));
        }
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value > MAX_INTEROPERABLE_INTEGER {
            return Err(self.fail(ReasonCategory::IntegerOutOfRange));
        }
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.fract() == 0.0 && value.abs() > MAX_INTEROPERABLE_INTEGER_F64 {
            return Err(self.fail(ReasonCategory::IntegerOutOfRange));
        }
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| self.fail(ReasonCategory::SchemaInvalid))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let depth = self.container_depth()?;
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(self.child(depth))? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let depth = self.container_depth()?;
        let mut keys = HashSet::new();
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(self.fail(ReasonCategory::DuplicateMember));
            }
            let value = object.next_value_seed(self.child(depth))?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
