// @generated from scripts/lib/strict-json.mjs; DO NOT EDIT.
import { assertWellFormedUnicode } from "./unicode.mjs";

const MAX_INTEROPERABLE_INTEGER = 9_007_199_254_740_991;

function normalizeDecimalLexeme(token) {
  const match = /^(-?)([0-9]+)(?:\.([0-9]+))?(?:[eE]([+-]?[0-9]+))?$/u.exec(token);
  if (match === null) return null;

  const fraction = match[3] ?? "";
  let digits = `${match[2]}${fraction}`.replace(/^0+/u, "");
  if (digits.length === 0) {
    return { negative: false, digits: "0", decimalExponent: 0 };
  }

  const exponentToken = match[4] ?? "0";
  const exponentMagnitude = exponentToken.replace(/^[+-]?0*/u, "");
  if (exponentMagnitude.length > 15) return null;
  const exponent = Number(exponentToken);
  if (!Number.isSafeInteger(exponent)) return null;

  let trailingZeros = 0;
  while (digits.endsWith("0")) {
    digits = digits.slice(0, -1);
    trailingZeros += 1;
  }
  const decimalExponent = exponent - fraction.length + trailingZeros;
  if (!Number.isSafeInteger(decimalExponent)) return null;

  return {
    negative: match[1] === "-",
    digits,
    decimalExponent,
  };
}

function isLosslesslyRepresentedNumber(token, value) {
  const source = normalizeDecimalLexeme(token);
  const represented = normalizeDecimalLexeme(String(value));
  return (
    source !== null &&
    represented !== null &&
    source.negative === represented.negative &&
    source.digits === represented.digits &&
    source.decimalExponent === represented.decimalExponent
  );
}

export class StrictJsonError extends SyntaxError {
  constructor(code, message, offset) {
    super(`${message} (offset ${offset})`);
    this.name = "StrictJsonError";
    this.code = code;
    this.offset = offset;
  }
}

class StrictJsonParser {
  constructor(text, { maxContainerDepth }) {
    this.text = text;
    this.offset = 0;
    this.maxContainerDepth = maxContainerDepth;
  }

  parse() {
    this.skipWhitespace();
    const value = this.parseValue(0);
    this.skipWhitespace();
    if (this.offset !== this.text.length) {
      this.fail("TRAILING_CONTENT", "Unexpected trailing JSON content");
    }
    return value;
  }

  parseValue(containerDepth) {
    const token = this.text[this.offset];
    if (token === "{") return this.parseObject(containerDepth + 1);
    if (token === "[") return this.parseArray(containerDepth + 1);
    if (token === '"') return this.parseString();
    if (token === "t") return this.parseLiteral("true", true);
    if (token === "f") return this.parseLiteral("false", false);
    if (token === "n") return this.parseLiteral("null", null);
    if (token === "-" || (token >= "0" && token <= "9")) {
      return this.parseNumber();
    }
    this.fail("UNEXPECTED_TOKEN", "Expected a JSON value");
  }

  parseObject(containerDepth) {
    this.assertContainerDepth(containerDepth);
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
      const value = this.parseValue(containerDepth);
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

  parseArray(containerDepth) {
    this.assertContainerDepth(containerDepth);
    const result = [];
    this.offset += 1;
    this.skipWhitespace();

    if (this.text[this.offset] === "]") {
      this.offset += 1;
      return result;
    }

    while (this.offset < this.text.length) {
      result.push(this.parseValue(containerDepth));
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
    if (
      (Number.isInteger(value) && Math.abs(value) > MAX_INTEROPERABLE_INTEGER) ||
      !isLosslesslyRepresentedNumber(match[0], value)
    ) {
      this.fail(
        "INTEGER_OUT_OF_RANGE",
        "JSON number cannot be represented losslessly in the interoperable range",
      );
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

  assertContainerDepth(containerDepth) {
    if (containerDepth > this.maxContainerDepth) {
      this.fail(
        "MAX_DEPTH_EXCEEDED",
        `JSON container depth exceeds the configured limit of ${this.maxContainerDepth}`,
      );
    }
  }

  fail(code, message) {
    throw new StrictJsonError(code, message, this.offset);
  }
}

function parseLimit(value, name, minimum) {
  if (value === undefined) return Number.POSITIVE_INFINITY;
  if (!Number.isSafeInteger(value) || value < minimum) {
    throw new RangeError(`${name} must be a safe integer greater than or equal to ${minimum}.`);
  }
  return value;
}

export function parseStrictJson(text, options = {}) {
  if (typeof text !== "string") {
    throw new TypeError("parseStrictJson expects a string.");
  }
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError("parseStrictJson options must be an object.");
  }

  const maxBytes = parseLimit(options.maxBytes, "maxBytes", 0);
  const maxContainerDepth = parseLimit(
    options.maxContainerDepth,
    "maxContainerDepth",
    1,
  );
  if (
    Number.isFinite(maxBytes) &&
    new TextEncoder().encode(text).byteLength > maxBytes
  ) {
    throw new StrictJsonError(
      "MAX_BYTES_EXCEEDED",
      `JSON input exceeds the configured limit of ${maxBytes} bytes`,
      0,
    );
  }

  return new StrictJsonParser(text, { maxContainerDepth }).parse();
}
