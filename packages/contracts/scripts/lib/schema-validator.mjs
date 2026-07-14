import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { canonicalize } from "./canonical-json.mjs";
import { parseStrictJson } from "./strict-json.mjs";

export function isDiscriminatorRefinement(node) {
  const keys = Object.keys(node).sort();
  if (keys.length !== 2 || keys[0] !== "properties" || keys[1] !== "type") return false;
  const entries = Object.entries(node.properties ?? {});
  if (entries.length < 1 || entries.length > 2) return false;
  const allowedProperties = new Set(["builderKind", "lens", "validationProfile"]);
  return entries.every(([name, constraint]) => {
    if (!allowedProperties.has(name) || constraint === null || typeof constraint !== "object") {
      return false;
    }
    const constraintKeys = Object.keys(constraint);
    return constraintKeys.length === 1 && (constraintKeys[0] === "const" || constraintKeys[0] === "enum");
  });
}

export async function loadSchemaRegistry(schemaDirectory) {
  const names = (await readdir(schemaDirectory))
    .filter((name) => name.endsWith(".schema.json"))
    .sort();
  const documents = new Map();
  const ids = new Map();

  for (const name of names) {
    const source = await readFile(path.join(schemaDirectory, name), "utf8");
    const document = parseStrictJson(source);
    documents.set(name, document);
    if (typeof document.$id === "string") {
      if (ids.has(document.$id)) {
        throw new Error(`Duplicate schema $id ${document.$id}.`);
      }
      ids.set(document.$id, name);
    }
  }

  return { documents, ids };
}

function decodePointerToken(token) {
  return token.replaceAll("~1", "/").replaceAll("~0", "~");
}

export function resolveReference(registry, reference, currentDocumentName) {
  const [documentPart, fragment = ""] = reference.split("#", 2);
  const documentName =
    documentPart.length === 0
      ? currentDocumentName
      : path.posix.basename(documentPart.replaceAll("\\", "/"));
  const document = registry.documents.get(documentName);
  if (document === undefined) {
    throw new Error(`Unresolved schema document ${documentName} from ${reference}.`);
  }

  if (fragment.length === 0) {
    return { documentName, schema: document };
  }
  if (!fragment.startsWith("/")) {
    throw new Error(`Unsupported non-pointer schema fragment #${fragment}.`);
  }

  let schema = document;
  for (const token of fragment.slice(1).split("/").map(decodePointerToken)) {
    if (schema === null || typeof schema !== "object" || !(token in schema)) {
      throw new Error(`Unresolved JSON pointer #${fragment} in ${documentName}.`);
    }
    schema = schema[token];
  }
  return { documentName, schema };
}

function valueType(value) {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  if (Number.isInteger(value)) return "integer";
  return typeof value;
}

function deepEqual(left, right) {
  try {
    return canonicalize(left) === canonicalize(right);
  } catch {
    return false;
  }
}

function validateNode({ schema, value, registry, documentName, instancePath, errors }) {
  if (schema === false) {
    errors.push({
      code: "FALSE_SCHEMA",
      path: instancePath,
      message: "Value is forbidden by the schema.",
    });
    return;
  }
  if (schema === true) return;

  if (schema.$ref !== undefined) {
    const resolved = resolveReference(registry, schema.$ref, documentName);
    validateNode({
      schema: resolved.schema,
      value,
      registry,
      documentName: resolved.documentName,
      instancePath,
      errors,
    });
  }

  if (schema.allOf !== undefined) {
    for (const branch of schema.allOf) {
      validateNode({
        schema: branch,
        value,
        registry,
        documentName,
        instancePath,
        errors,
      });
    }
  }

  if (schema.oneOf !== undefined) {
    const outcomes = schema.oneOf.map((branch) => {
      const branchErrors = [];
      validateNode({
        schema: branch,
        value,
        registry,
        documentName,
        instancePath,
        errors: branchErrors,
      });
      return branchErrors;
    });
    const matches = outcomes.filter((branchErrors) => branchErrors.length === 0);
    if (matches.length !== 1) {
      errors.push({
        code: "ONE_OF_MISMATCH",
        path: instancePath,
        message: `Expected exactly one matching branch, received ${matches.length}.`,
      });
    }
  }

  if (schema.const !== undefined && !deepEqual(value, schema.const)) {
    errors.push({
      code: "CONST_MISMATCH",
      path: instancePath,
      message: `Expected constant ${JSON.stringify(schema.const)}.`,
    });
    return;
  }

  if (
    schema.enum !== undefined &&
    !schema.enum.some((member) => deepEqual(value, member))
  ) {
    errors.push({
      code: "ENUM_MISMATCH",
      path: instancePath,
      message: "Value is not a member of the closed enum.",
    });
    return;
  }

  if (schema.type !== undefined) {
    const actualType = valueType(value);
    const expected = Array.isArray(schema.type) ? schema.type : [schema.type];
    const typeMatches = expected.some(
      (type) => type === actualType || (type === "number" && actualType === "integer"),
    );
    if (!typeMatches) {
      errors.push({
        code: "TYPE_MISMATCH",
        path: instancePath,
        message: `Expected ${expected.join(" or ")}, received ${actualType}.`,
      });
      return;
    }
  }

  if (typeof value === "string") {
    const length = [...value].length;
    if (schema.minLength !== undefined && length < schema.minLength) {
      errors.push({ code: "STRING_TOO_SHORT", path: instancePath, message: "String is too short." });
    }
    if (schema.maxLength !== undefined && length > schema.maxLength) {
      errors.push({ code: "STRING_TOO_LONG", path: instancePath, message: "String is too long." });
    }
    if (schema.pattern !== undefined && !new RegExp(schema.pattern, "u").test(value)) {
      errors.push({ code: "PATTERN_MISMATCH", path: instancePath, message: "String does not match the required pattern." });
    }
  }

  if (typeof value === "number") {
    if (!Number.isFinite(value)) {
      errors.push({ code: "NON_FINITE_NUMBER", path: instancePath, message: "Number is not finite." });
    }
    if (schema.type === "integer" && !Number.isSafeInteger(value)) {
      errors.push({ code: "INTEGER_OUT_OF_RANGE", path: instancePath, message: "Integer is not safely interoperable." });
    }
    if (schema.minimum !== undefined && value < schema.minimum) {
      errors.push({ code: "NUMBER_TOO_SMALL", path: instancePath, message: "Number is below the minimum." });
    }
    if (schema.maximum !== undefined && value > schema.maximum) {
      errors.push({ code: "NUMBER_TOO_LARGE", path: instancePath, message: "Number exceeds the maximum." });
    }
  }

  if (Array.isArray(value)) {
    if (schema.minItems !== undefined && value.length < schema.minItems) {
      errors.push({ code: "ARRAY_TOO_SHORT", path: instancePath, message: "Array has too few items." });
    }
    if (schema.maxItems !== undefined && value.length > schema.maxItems) {
      errors.push({ code: "ARRAY_TOO_LONG", path: instancePath, message: "Array has too many items." });
    }
    if (schema.uniqueItems === true) {
      const canonicalItems = value.map((item) => canonicalize(item));
      if (new Set(canonicalItems).size !== canonicalItems.length) {
        errors.push({ code: "ARRAY_NOT_UNIQUE", path: instancePath, message: "Array contains duplicate items." });
      }
    }
    for (let index = 0; index < Math.min(value.length, schema.prefixItems?.length ?? 0); index += 1) {
      validateNode({
        schema: schema.prefixItems[index],
        value: value[index],
        registry,
        documentName,
        instancePath: `${instancePath}/${index}`,
        errors,
      });
    }
    if (schema.items !== undefined) {
      const start = schema.prefixItems?.length ?? 0;
      value.slice(start).forEach((item, offset) => {
        validateNode({
          schema: schema.items,
          value: item,
          registry,
          documentName,
          instancePath: `${instancePath}/${start + offset}`,
          errors,
        });
      });
    }
  }

  if (value !== null && !Array.isArray(value) && typeof value === "object") {
    const properties = schema.properties ?? {};
    for (const required of schema.required ?? []) {
      if (!Object.hasOwn(value, required)) {
        errors.push({
          code: "REQUIRED_PROPERTY_MISSING",
          path: instancePath,
          message: `Missing required property ${required}.`,
        });
      }
    }
    if (schema.additionalProperties === false) {
      for (const key of Object.keys(value)) {
        if (!Object.hasOwn(properties, key)) {
          errors.push({
            code: "UNKNOWN_PROPERTY",
            path: `${instancePath}/${key}`,
            message: `Unknown property ${key}.`,
          });
        }
      }
    }
    for (const [key, propertySchema] of Object.entries(properties)) {
      if (Object.hasOwn(value, key)) {
        validateNode({
          schema: propertySchema,
          value: value[key],
          registry,
          documentName,
          instancePath: `${instancePath}/${key}`,
          errors,
        });
      }
    }
  }
}

export function validateSchemaDocument(registry, documentName, value) {
  const schema = registry.documents.get(documentName);
  if (schema === undefined) {
    throw new Error(`Unknown schema document ${documentName}.`);
  }
  const errors = [];
  validateNode({
    schema,
    value,
    registry,
    documentName,
    instancePath: "$",
    errors,
  });
  return errors;
}
