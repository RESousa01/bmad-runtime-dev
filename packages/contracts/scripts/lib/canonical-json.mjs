import { sha256Hex } from "./sha256.mjs";

export class CanonicalizationError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "CanonicalizationError";
    this.code = code;
  }
}

export function assertWellFormedUnicode(value, label = "string") {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);

    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) {
        throw new CanonicalizationError(
          "INVALID_UNICODE",
          `${label} contains an unpaired high surrogate at UTF-16 index ${index}.`,
        );
      }
      index += 1;
      continue;
    }

    if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      throw new CanonicalizationError(
        "INVALID_UNICODE",
        `${label} contains an unpaired low surrogate at UTF-16 index ${index}.`,
      );
    }
  }
}

function serialize(value, path) {
  if (value === null) {
    return "null";
  }

  switch (typeof value) {
    case "boolean":
      return value ? "true" : "false";
    case "number": {
      if (!Number.isFinite(value)) {
        throw new CanonicalizationError(
          "NON_FINITE_NUMBER",
          `${path} contains a non-finite number.`,
        );
      }
      if (Number.isInteger(value) && !Number.isSafeInteger(value)) {
        throw new CanonicalizationError(
          "INTEGER_OUT_OF_RANGE",
          `${path} exceeds the interoperable JSON integer range.`,
        );
      }
      return JSON.stringify(value);
    }
    case "string":
      assertWellFormedUnicode(value, path);
      return JSON.stringify(value);
    case "object": {
      if (Array.isArray(value)) {
        const items = [];
        for (let index = 0; index < value.length; index += 1) {
          if (!Object.hasOwn(value, index)) {
            throw new CanonicalizationError(
              "SPARSE_ARRAY",
              `${path}[${index}] is a sparse array hole.`,
            );
          }
          const item = value[index];
          if (item === undefined) {
            throw new CanonicalizationError(
              "UNDEFINED_VALUE",
              `${path}[${index}] is undefined.`,
            );
          }
          items.push(serialize(item, `${path}[${index}]`));
        }
        if (Object.keys(value).some((key) => !/^(0|[1-9][0-9]*)$/.test(key))) {
          throw new CanonicalizationError(
            "ARRAY_EXTRA_PROPERTY",
            `${path} carries a non-JSON array property.`,
          );
        }
        return `[${items.join(",")}]`;
      }

      const prototype = Object.getPrototypeOf(value);
      if (prototype !== Object.prototype && prototype !== null) {
        throw new CanonicalizationError(
          "NON_JSON_OBJECT",
          `${path} is not a plain JSON object.`,
        );
      }

      const ownKeys = Reflect.ownKeys(value);
      if (ownKeys.some((key) => typeof key !== "string")) {
        throw new CanonicalizationError(
          "NON_JSON_OBJECT",
          `${path} carries a symbol property.`,
        );
      }
      const keys = ownKeys.sort();
      const members = keys.map((key) => {
        const descriptor = Object.getOwnPropertyDescriptor(value, key);
        if (
          descriptor === undefined ||
          descriptor.enumerable !== true ||
          descriptor.get !== undefined ||
          descriptor.set !== undefined
        ) {
          throw new CanonicalizationError(
            "NON_JSON_OBJECT",
            `${path}.${key} is not an enumerable JSON data property.`,
          );
        }
        assertWellFormedUnicode(key, `${path} property name`);
        const member = descriptor.value;
        if (member === undefined) {
          throw new CanonicalizationError(
            "UNDEFINED_VALUE",
            `${path}.${key} is undefined.`,
          );
        }
        return `${JSON.stringify(key)}:${serialize(member, `${path}.${key}`)}`;
      });
      return `{${members.join(",")}}`;
    }
    default:
      throw new CanonicalizationError(
        "NON_JSON_VALUE",
        `${path} contains unsupported JSON type ${typeof value}.`,
      );
  }
}

export function canonicalize(value) {
  return serialize(value, "$");
}

export function withoutRootFields(value, excludedFields) {
  if (value === null || Array.isArray(value) || typeof value !== "object") {
    throw new CanonicalizationError(
      "EXCLUSION_REQUIRES_OBJECT",
      "Root-field exclusion requires a JSON object.",
    );
  }

  const excluded = new Set(excludedFields);
  return Object.fromEntries(
    Object.entries(value).filter(([key]) => !excluded.has(key)),
  );
}

export function canonicalHash({ purpose, schemaMajor, value, excludedFields = [] }) {
  if (!/^[a-z][a-z0-9-]{1,63}$/.test(purpose)) {
    throw new CanonicalizationError(
      "INVALID_HASH_PURPOSE",
      `Invalid Sapphirus hash purpose: ${purpose}`,
    );
  }
  if (!/^v[1-9][0-9]*$/.test(schemaMajor)) {
    throw new CanonicalizationError(
      "INVALID_SCHEMA_MAJOR",
      `Invalid schema major: ${schemaMajor}`,
    );
  }

  const hashValue =
    excludedFields.length === 0
      ? value
      : withoutRootFields(value, excludedFields);
  const preimage = `sapphirus:${purpose}:${schemaMajor}\n${canonicalize(hashValue)}`;
  const digest = sha256Hex(preimage);

  return {
    canonicalJson: canonicalize(hashValue),
    preimage,
    serializedHash: `sha256:${digest}`,
  };
}
