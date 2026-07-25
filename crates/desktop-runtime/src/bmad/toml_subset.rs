//! A bounded TOML-subset reader for untrusted method configuration.
//!
//! The method's config surface is `customize.toml` and the four `config.toml`
//! layers, all of which live in the *user's workspace* and are therefore
//! untrusted input. Rather than take a general-purpose TOML dependency, this
//! reads exactly the subset those files use and **fails closed on anything
//! else**: an unrecognised construct is an error, never a silent skip. A
//! parser that guesses at input it does not understand is the wrong shape for
//! a security boundary.
//!
//! Supported: comments, `[table]`, `[[array.of.tables]]`, basic and literal
//! strings (including `"""` multi-line), integers, floats, booleans, arrays
//! (multi-line, trailing comma allowed), and inline tables.
//!
//! Deliberately unsupported, and rejected: datetimes, hex/octal/binary
//! integers, dotted keys outside a header, and string escapes beyond the
//! common set. None appear in the reviewed configuration.

use serde_json::{Map, Number, Value};

/// Bounds the whole document, mirroring the config-layer ceiling.
const MAX_INPUT_BYTES: usize = 1_048_576;
const MAX_DEPTH: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TomlSubsetError {
    /// The document exceeds its reviewed size or nesting bound.
    BoundsViolation,
    /// A construct outside the supported subset, or malformed input.
    Unsupported,
    /// A table or key was defined twice.
    Duplicate,
}

/// Parses one bounded TOML document into a JSON object.
///
/// # Errors
///
/// Returns [`TomlSubsetError`] for anything outside the supported subset; the
/// caller records that as an invalid config layer rather than proceeding on a
/// partial parse.
pub fn parse_toml_subset(input: &str) -> Result<Value, TomlSubsetError> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(TomlSubsetError::BoundsViolation);
    }
    let mut root = Map::new();
    // Path of the table currently being filled by bare `key = value` lines.
    let mut current: Vec<String> = Vec::new();
    let mut cursor = Cursor::new(input);

    loop {
        cursor.skip_trivia();
        if cursor.done() {
            break;
        }
        if cursor.eat('[') {
            let array_of_tables = cursor.eat('[');
            let path = cursor.parse_key_path()?;
            if !cursor.eat(']') || (array_of_tables && !cursor.eat(']')) {
                return Err(TomlSubsetError::Unsupported);
            }
            cursor.skip_to_line_end()?;
            if array_of_tables {
                push_array_table(&mut root, &path)?;
            } else {
                ensure_table(&mut root, &path)?;
            }
            current = path;
            continue;
        }
        let key = cursor.parse_simple_key()?;
        cursor.skip_inline_space();
        if !cursor.eat('=') {
            return Err(TomlSubsetError::Unsupported);
        }
        cursor.skip_inline_space();
        let value = cursor.parse_value(0)?;
        cursor.skip_to_line_end()?;
        insert_value(&mut root, &current, &key, value)?;
    }
    Ok(Value::Object(root))
}

/// Resolves `path` to a table, creating tables as needed. Walks into the last
/// element of an array-of-tables so later `[a.b]` headers address the entry
/// most recently pushed, as TOML requires.
fn table_at<'a>(
    root: &'a mut Map<String, Value>,
    path: &[String],
) -> Result<&'a mut Map<String, Value>, TomlSubsetError> {
    if path.len() > MAX_DEPTH {
        return Err(TomlSubsetError::BoundsViolation);
    }
    let mut table = root;
    for segment in path {
        let entry = table
            .entry(segment.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        table = match entry {
            Value::Object(object) => object,
            Value::Array(items) => match items.last_mut() {
                Some(Value::Object(object)) => object,
                _ => return Err(TomlSubsetError::Unsupported),
            },
            _ => return Err(TomlSubsetError::Unsupported),
        };
    }
    Ok(table)
}

fn ensure_table(root: &mut Map<String, Value>, path: &[String]) -> Result<(), TomlSubsetError> {
    if path.is_empty() {
        return Err(TomlSubsetError::Unsupported);
    }
    table_at(root, path)?;
    Ok(())
}

fn push_array_table(root: &mut Map<String, Value>, path: &[String]) -> Result<(), TomlSubsetError> {
    let (last, parents) = path.split_last().ok_or(TomlSubsetError::Unsupported)?;
    let parent = table_at(root, parents)?;
    let entry = parent
        .entry(last.clone())
        .or_insert_with(|| Value::Array(Vec::new()));
    match entry {
        Value::Array(items) => {
            items.push(Value::Object(Map::new()));
            Ok(())
        }
        _ => Err(TomlSubsetError::Duplicate),
    }
}

fn insert_value(
    root: &mut Map<String, Value>,
    path: &[String],
    key: &str,
    value: Value,
) -> Result<(), TomiInsertAlias> {
    let table = table_at(root, path)?;
    if table.contains_key(key) {
        return Err(TomlSubsetError::Duplicate);
    }
    table.insert(key.to_owned(), value);
    Ok(())
}

/// Alias so the signature above stays readable; the error type is unchanged.
type TomiInsertAlias = TomlSubsetError;

struct Cursor<'a> {
    bytes: &'a [u8],
    input: &'a str,
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            input,
            position: 0,
        }
    }

    fn done(&self) -> bool {
        self.position >= self.bytes.len()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected as u8) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn skip_inline_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.position += 1;
        }
    }

    /// Skips whitespace, newlines, and comments — everything between tokens.
    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\r' | b'\n') => self.position += 1,
                Some(b'#') => {
                    while !matches!(self.peek(), None | Some(b'\n')) {
                        self.position += 1;
                    }
                }
                _ => return,
            }
        }
    }

    /// After a value, only spaces and an optional comment may precede the line
    /// break. Anything else is unsupported input, not something to skip past.
    fn skip_to_line_end(&mut self) -> Result<(), TomlSubsetError> {
        self.skip_inline_space();
        if self.peek() == Some(b'#') {
            while !matches!(self.peek(), None | Some(b'\n')) {
                self.position += 1;
            }
        }
        match self.peek() {
            None => Ok(()),
            Some(b'\n' | b'\r') => {
                self.position += 1;
                Ok(())
            }
            Some(_) => Err(TomlSubsetError::Unsupported),
        }
    }

    fn parse_simple_key(&mut self) -> Result<String, TomlSubsetError> {
        if self.peek() == Some(b'"') {
            return self.parse_basic_string();
        }
        let start = self.position;
        while matches!(
            self.peek(),
            Some(b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-')
        ) {
            self.position += 1;
        }
        if self.position == start {
            return Err(TomlSubsetError::Unsupported);
        }
        self.input
            .get(start..self.position)
            .map(str::to_owned)
            .ok_or(TomlSubsetError::Unsupported)
    }

    fn parse_key_path(&mut self) -> Result<Vec<String>, TomlSubsetError> {
        let mut path = Vec::new();
        loop {
            self.skip_inline_space();
            path.push(self.parse_simple_key()?);
            self.skip_inline_space();
            if !self.eat('.') {
                break;
            }
        }
        if path.len() > MAX_DEPTH {
            return Err(TomlSubsetError::BoundsViolation);
        }
        Ok(path)
    }

    fn parse_value(&mut self, depth: usize) -> Result<Value, TomlSubsetError> {
        if depth > MAX_DEPTH {
            return Err(TomlSubsetError::BoundsViolation);
        }
        match self.peek() {
            Some(b'"') => self.parse_string_value(),
            Some(b'\'') => self.parse_literal_string().map(Value::String),
            Some(b'[') => self.parse_array(depth),
            Some(b'{') => self.parse_inline_table(depth),
            Some(b't' | b'f') => self.parse_bool(),
            Some(b'0'..=b'9' | b'+' | b'-') => self.parse_number(),
            _ => Err(TomlSubsetError::Unsupported),
        }
    }

    fn parse_string_value(&mut self) -> Result<Value, TomlSubsetError> {
        if self
            .input
            .get(self.position..)
            .is_some_and(|rest| rest.starts_with(r#"""""#))
        {
            return self.parse_multiline_basic_string().map(Value::String);
        }
        self.parse_basic_string().map(Value::String)
    }

    fn parse_basic_string(&mut self) -> Result<String, TomlSubsetError> {
        if !self.eat('"') {
            return Err(TomlSubsetError::Unsupported);
        }
        let mut text = String::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => return Err(TomlSubsetError::Unsupported),
                Some(b'"') => {
                    self.position += 1;
                    return Ok(text);
                }
                Some(b'\\') => {
                    self.position += 1;
                    text.push(self.parse_escape()?);
                }
                Some(_) => self.push_char(&mut text)?,
            }
        }
    }

    fn parse_multiline_basic_string(&mut self) -> Result<String, TomlSubsetError> {
        self.position += 3;
        // A newline immediately after the opening delimiter is trimmed.
        if self.peek() == Some(b'\r') {
            self.position += 1;
        }
        if self.peek() == Some(b'\n') {
            self.position += 1;
        }
        let mut text = String::new();
        loop {
            let rest = self
                .input
                .get(self.position..)
                .ok_or(TomlSubsetError::Unsupported)?;
            if rest.starts_with(r#"""""#) {
                self.position += 3;
                return Ok(text);
            }
            match self.peek() {
                None => return Err(TomlSubsetError::Unsupported),
                Some(b'\\') => {
                    self.position += 1;
                    text.push(self.parse_escape()?);
                }
                Some(_) => self.push_char(&mut text)?,
            }
        }
    }

    fn parse_literal_string(&mut self) -> Result<String, TomlSubsetError> {
        if !self.eat('\'') {
            return Err(TomlSubsetError::Unsupported);
        }
        let mut text = String::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => return Err(TomlSubsetError::Unsupported),
                Some(b'\'') => {
                    self.position += 1;
                    return Ok(text);
                }
                Some(_) => self.push_char(&mut text)?,
            }
        }
    }

    /// Appends one whole UTF-8 character, so multi-byte input is never split.
    fn push_char(&mut self, text: &mut String) -> Result<(), TomlSubsetError> {
        let rest = self
            .input
            .get(self.position..)
            .ok_or(TomlSubsetError::Unsupported)?;
        let character = rest.chars().next().ok_or(TomlSubsetError::Unsupported)?;
        self.position += character.len_utf8();
        text.push(character);
        Ok(())
    }

    fn parse_escape(&mut self) -> Result<char, TomlSubsetError> {
        let escape = self.peek().ok_or(TomlSubsetError::Unsupported)?;
        self.position += 1;
        Ok(match escape {
            b'"' => '"',
            b'\\' => '\\',
            b'n' => '\n',
            b't' => '\t',
            b'r' => '\r',
            // Unicode escapes and the rest are outside the reviewed subset.
            _ => return Err(TomlSubsetError::Unsupported),
        })
    }

    fn parse_bool(&mut self) -> Result<Value, TomlSubsetError> {
        let rest = self
            .input
            .get(self.position..)
            .ok_or(TomlSubsetError::Unsupported)?;
        if rest.starts_with("true") {
            self.position += 4;
            return Ok(Value::Bool(true));
        }
        if rest.starts_with("false") {
            self.position += 5;
            return Ok(Value::Bool(false));
        }
        Err(TomlSubsetError::Unsupported)
    }

    fn parse_number(&mut self) -> Result<Value, TomlSubsetError> {
        let start = self.position;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.position += 1;
        }
        let mut float = false;
        while let Some(byte) = self.peek() {
            match byte {
                b'0'..=b'9' | b'_' => self.position += 1,
                b'.' | b'e' | b'E' => {
                    float = true;
                    self.position += 1;
                }
                _ => break,
            }
        }
        let raw = self
            .input
            .get(start..self.position)
            .ok_or(TomlSubsetError::Unsupported)?
            .replace('_', "");
        if raw.is_empty() {
            return Err(TomlSubsetError::Unsupported);
        }
        if float {
            let parsed = raw
                .parse::<f64>()
                .map_err(|_| TomlSubsetError::Unsupported)?;
            return Number::from_f64(parsed)
                .map(Value::Number)
                .ok_or(TomlSubsetError::Unsupported);
        }
        raw.parse::<i64>()
            .map(|value| Value::Number(value.into()))
            .map_err(|_| TomlSubsetError::Unsupported)
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, TomlSubsetError> {
        if !self.eat('[') {
            return Err(TomlSubsetError::Unsupported);
        }
        let mut items = Vec::new();
        loop {
            self.skip_trivia();
            if self.eat(']') {
                return Ok(Value::Array(items));
            }
            items.push(self.parse_value(depth + 1)?);
            self.skip_trivia();
            if self.eat(',') {
                continue;
            }
            self.skip_trivia();
            if self.eat(']') {
                return Ok(Value::Array(items));
            }
            return Err(TomlSubsetError::Unsupported);
        }
    }

    fn parse_inline_table(&mut self, depth: usize) -> Result<Value, TomlSubsetError> {
        if !self.eat('{') {
            return Err(TomlSubsetError::Unsupported);
        }
        let mut table = Map::new();
        loop {
            self.skip_trivia();
            if self.eat('}') {
                return Ok(Value::Object(table));
            }
            let key = self.parse_simple_key()?;
            self.skip_inline_space();
            if !self.eat('=') {
                return Err(TomlSubsetError::Unsupported);
            }
            self.skip_inline_space();
            let value = self.parse_value(depth + 1)?;
            if table.insert(key, value).is_some() {
                return Err(TomlSubsetError::Duplicate);
            }
            self.skip_trivia();
            if self.eat(',') {
                continue;
            }
            self.skip_trivia();
            if self.eat('}') {
                return Ok(Value::Object(table));
            }
            return Err(TomlSubsetError::Unsupported);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{parse_toml_subset, TomlSubsetError};

    /// The real sealed customization for the architecture workflow.
    const ARCHITECTURE_CUSTOMIZE: &str = include_str!(
        "../../../../packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/3-solutioning/bmad-architecture/customize.toml"
    );
    /// The real sealed agent customization, which uses `[[agent.menu]]`.
    const ARCHITECT_AGENT: &str = include_str!(
        "../../../../packages/bmad-foundation/source/method/6.10.0/src/bmm-skills/3-solutioning/bmad-agent-architect/customize.toml"
    );

    #[test]
    fn the_sealed_workflow_customization_parses() {
        let value = parse_toml_subset(ARCHITECTURE_CUSTOMIZE).expect("workflow customization");
        let workflow = value.get("workflow").expect("workflow table");

        assert_eq!(
            workflow
                .get("spine_template")
                .and_then(serde_json::Value::as_str),
            Some("assets/spine-template.md")
        );
        assert_eq!(
            workflow
                .get("activation_steps_prepend")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        let facts = workflow
            .get("persistent_facts")
            .and_then(serde_json::Value::as_array)
            .expect("persistent facts");
        assert_eq!(facts.len(), 1);
        assert!(facts[0]
            .as_str()
            .is_some_and(|fact| fact.starts_with("file:")));
    }

    #[test]
    fn the_sealed_agent_menu_parses_as_an_array_of_tables() {
        let value = parse_toml_subset(ARCHITECT_AGENT).expect("agent customization");
        let agent = value.get("agent").expect("agent table");

        assert_eq!(
            agent.get("name").and_then(serde_json::Value::as_str),
            Some("Winston")
        );
        assert_eq!(
            agent.get("title").and_then(serde_json::Value::as_str),
            Some("System Architect")
        );
        let menu = agent
            .get("menu")
            .and_then(serde_json::Value::as_array)
            .expect("menu");
        assert_eq!(menu.len(), 2);
        assert_eq!(
            menu[0].get("code").and_then(serde_json::Value::as_str),
            Some("CA")
        );
        assert_eq!(
            menu[0].get("skill").and_then(serde_json::Value::as_str),
            Some("bmad-architecture")
        );
        assert_eq!(
            menu[1].get("code").and_then(serde_json::Value::as_str),
            Some("IR")
        );
        // `principles` is a multi-line array of strings.
        assert_eq!(
            agent
                .get("principles")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(3)
        );
    }

    #[test]
    fn the_supported_scalar_and_collection_forms_round_trip() {
        let value = parse_toml_subset(
            "top = \"a\"\nflag = true\ncount = 12\nratio = 1.5\nlit = 'raw\\n'\n\
             [t]\nlist = [\n  \"x\",\n  \"y\",\n]\ninline = { a = 1, b = \"two\" }\n\
             multi = \"\"\"\nline one\nline two\"\"\"\n",
        )
        .expect("document");

        assert_eq!(
            value.get("top").and_then(serde_json::Value::as_str),
            Some("a")
        );
        assert_eq!(
            value.get("flag").and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            value.get("count").and_then(serde_json::Value::as_i64),
            Some(12)
        );
        assert!(value
            .get("ratio")
            .and_then(serde_json::Value::as_f64)
            .is_some());
        // A literal string keeps its backslash verbatim.
        assert_eq!(
            value.get("lit").and_then(serde_json::Value::as_str),
            Some("raw\\n")
        );
        let table = value.get("t").expect("table");
        assert_eq!(
            table
                .get("list")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            table
                .get("inline")
                .and_then(|v| v.get("b"))
                .and_then(serde_json::Value::as_str),
            Some("two")
        );
        assert_eq!(
            table.get("multi").and_then(serde_json::Value::as_str),
            Some("line one\nline two")
        );
    }

    #[test]
    fn unsupported_and_malformed_input_fails_closed() {
        for rejected in [
            // Datetimes are outside the reviewed subset.
            "when = 1979-05-27T07:32:00Z\n",
            // Unterminated strings and arrays.
            "key = \"open\n",
            "key = [1, 2\n",
            // A stray token after a value is not silently skipped.
            "key = 1 garbage\n",
            // Malformed headers.
            "[unclosed\n",
            "[[half]\n",
            // A value form we do not support.
            "key = 0xdeadbeef\n",
            "key = \n",
        ] {
            assert!(
                parse_toml_subset(rejected).is_err(),
                "accepted unsupported input: {rejected:?}"
            );
        }
    }

    #[test]
    fn duplicate_keys_fail_closed() {
        assert_eq!(
            parse_toml_subset("a = 1\na = 2\n"),
            Err(TomlSubsetError::Duplicate)
        );
        assert_eq!(
            parse_toml_subset("[t]\nk = 1\nk = 2\n"),
            Err(TomlSubsetError::Duplicate)
        );
        assert_eq!(
            parse_toml_subset("x = { a = 1, a = 2 }\n"),
            Err(TomlSubsetError::Duplicate)
        );
    }

    #[test]
    fn oversized_input_fails_closed() {
        let oversized = format!("k = \"{}\"\n", "x".repeat(1_048_600));
        assert_eq!(
            parse_toml_subset(&oversized),
            Err(TomlSubsetError::BoundsViolation)
        );
    }

    #[test]
    fn comments_and_blank_lines_are_ignored_everywhere() {
        let value =
            parse_toml_subset("# leading\n\n[t] # after header\n\n# between\nk = 1 # trailing\n\n")
                .expect("document");

        assert_eq!(
            value
                .get("t")
                .and_then(|t| t.get("k"))
                .and_then(serde_json::Value::as_i64),
            Some(1)
        );
    }
}
