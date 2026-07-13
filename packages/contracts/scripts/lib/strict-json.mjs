import { assertWellFormedUnicode } from "./canonical-json.mjs";

export class StrictJsonError extends SyntaxError {
  constructor(code, message, offset) {
    super(`${message} (offset ${offset})`);
    this.name = "StrictJsonError";
    this.code = code;
    this.offset = offset;
  }
}

class StrictJsonParser {
  constructor(text) {
    this.text = text;
    this.offset = 0;
  }

  parse() {
    this.skipWhitespace();
    const value = this.parseValue();
    this.skipWhitespace();
    if (this.offset !== this.text.length) {
      this.fail("TRAILING_CONTENT", "Unexpected trailing JSON content");
    }
    return value;
  }

  parseValue() {
    const token = this.text[this.offset];
    if (token === "{") return this.parseObject();
    if (token === "[") return this.parseArray();
    if (token === '"') return this.parseString();
    if (token === "t") return this.parseLiteral("true", true);
    if (token === "f") return this.parseLiteral("false", false);
    if (token === "n") return this.parseLiteral("null", null);
    if (token === "-" || (token >= "0" && token <= "9")) {
      return this.parseNumber();
    }
    this.fail("UNEXPECTED_TOKEN", "Expected a JSON value");
  }

  parseObject() {
    const result = Object.create(null);
    const keys = new Set();
    this.offset += 1;
    this.skipWhitespace();

    if (this.text[this.offset] === "}") {
      this.offset += 1;
      return result;
    }

    while (this.offset < this.text.length) {
      if (this.text[this.offset] !== '"') {
        this.fail("OBJECT_KEY_REQUIRED", "Expected a JSON object member name");
      }
      const keyOffset = this.offset;
      const key = this.parseString();
      if (keys.has(key)) {
        throw new StrictJsonError(
          "DUPLICATE_MEMBER",
          `Duplicate JSON member ${JSON.stringify(key)}`,
          keyOffset,
        );
      }
      keys.add(key);
      this.skipWhitespace();
      this.expect(":", "OBJECT_COLON_REQUIRED");
      this.skipWhitespace();
      const value = this.parseValue();
      Object.defineProperty(result, key, {
        configurable: true,
        enumerable: true,
        value,
        writable: true,
      });
      this.skipWhitespace();

      if (this.text[this.offset] === "}") {
        this.offset += 1;
        return result;
      }
      this.expect(",", "OBJECT_SEPARATOR_REQUIRED");
      this.skipWhitespace();
    }

    this.fail("UNTERMINATED_OBJECT", "Unterminated JSON object");
  }

  parseArray() {
    const result = [];
    this.offset += 1;
    this.skipWhitespace();

    if (this.text[this.offset] === "]") {
      this.offset += 1;
      return result;
    }

    while (this.offset < this.text.length) {
      result.push(this.parseValue());
      this.skipWhitespace();
      if (this.text[this.offset] === "]") {
        this.offset += 1;
        return result;
      }
      this.expect(",", "ARRAY_SEPARATOR_REQUIRED");
      this.skipWhitespace();
    }

    this.fail("UNTERMINATED_ARRAY", "Unterminated JSON array");
  }

  parseString() {
    const start = this.offset;
    this.offset += 1;
    let escaped = false;

    while (this.offset < this.text.length) {
      const codeUnit = this.text.charCodeAt(this.offset);
      const token = this.text[this.offset];

      if (codeUnit <= 0x1f) {
        this.fail("UNESCAPED_CONTROL", "Unescaped control character in JSON string");
      }
      if (escaped) {
        if (token === "u") {
          const hex = this.text.slice(this.offset + 1, this.offset + 5);
          if (!/^[0-9a-fA-F]{4}$/.test(hex)) {
            this.fail("INVALID_UNICODE_ESCAPE", "Invalid JSON Unicode escape");
          }
          this.offset += 5;
          escaped = false;
          continue;
        }
        if (!'"\\/bfnrt'.includes(token)) {
          this.fail("INVALID_ESCAPE", "Invalid JSON escape sequence");
        }
        escaped = false;
        this.offset += 1;
        continue;
      }
      if (token === "\\") {
        escaped = true;
        this.offset += 1;
        continue;
      }
      if (token === '"') {
        this.offset += 1;
        const decoded = JSON.parse(this.text.slice(start, this.offset));
        try {
          assertWellFormedUnicode(decoded, "JSON string");
        } catch (error) {
          throw new StrictJsonError(
            error.code ?? "INVALID_UNICODE",
            error.message,
            start,
          );
        }
        return decoded;
      }
      this.offset += 1;
    }

    this.fail("UNTERMINATED_STRING", "Unterminated JSON string");
  }

  parseNumber() {
    const rest = this.text.slice(this.offset);
    const match = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/.exec(rest);
    if (match === null) {
      this.fail("INVALID_NUMBER", "Invalid JSON number");
    }

    this.offset += match[0].length;
    const value = Number(match[0]);
    if (!Number.isFinite(value)) {
      this.fail("NON_FINITE_NUMBER", "JSON number cannot be represented finitely");
    }
    if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
      this.fail("INTEGER_OUT_OF_RANGE", "JSON integer exceeds the interoperable range");
    }
    return value;
  }

  parseLiteral(literal, value) {
    if (this.text.slice(this.offset, this.offset + literal.length) !== literal) {
      this.fail("INVALID_LITERAL", `Invalid JSON literal; expected ${literal}`);
    }
    this.offset += literal.length;
    return value;
  }

  skipWhitespace() {
    while (
      this.offset < this.text.length &&
      (this.text[this.offset] === " " ||
        this.text[this.offset] === "\t" ||
        this.text[this.offset] === "\r" ||
        this.text[this.offset] === "\n")
    ) {
      this.offset += 1;
    }
  }

  expect(token, code) {
    if (this.text[this.offset] !== token) {
      this.fail(code, `Expected ${JSON.stringify(token)}`);
    }
    this.offset += 1;
  }

  fail(code, message) {
    throw new StrictJsonError(code, message, this.offset);
  }
}

export function parseStrictJson(text) {
  if (typeof text !== "string") {
    throw new TypeError("parseStrictJson expects a string.");
  }
  return new StrictJsonParser(text).parse();
}
