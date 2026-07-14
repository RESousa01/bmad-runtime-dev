import Ajv2020 from "ajv/dist/2020.js";
import { StrictJsonError, parseStrictJson } from "./strict-json.mjs";

const STRICT_PARSER_REASONS = new Map([
  ["DUPLICATE_MEMBER", "DUPLICATE_MEMBER"],
  ["MAX_DEPTH_EXCEEDED", "MAX_DEPTH_EXCEEDED"],
  ["MAX_BYTES_EXCEEDED", "MAX_BYTES_EXCEEDED"],
  ["INVALID_UNICODE", "INVALID_UNICODE"],
  ["INTEGER_OUT_OF_RANGE", "INTEGER_OUT_OF_RANGE"],
]);

const AJV_KEYWORD_REASONS = new Map([
  ["additionalProperties", "UNKNOWN_PROPERTY"],
  ["required", "REQUIRED_PROPERTY_MISSING"],
  ["type", "TYPE_MISMATCH"],
  ["minimum", "NUMBER_TOO_SMALL"],
  ["maximum", "NUMBER_TOO_LARGE"],
  ["pattern", "PATTERN_MISMATCH"],
  ["oneOf", "ONE_OF_MISMATCH"],
]);

function discriminatorReason(value) {
  const variant = value?.variant;
  if (variant === null || typeof variant !== "object" || Array.isArray(variant)) {
    return null;
  }

  if (typeof variant.kind === "string" && !["text", "count"].includes(variant.kind)) {
    return "UNKNOWN_DISCRIMINATOR";
  }
  if (variant.kind === "text" && Object.hasOwn(variant, "count")) {
    return "ONE_OF_MISMATCH";
  }
  if (variant.kind === "count" && Object.hasOwn(variant, "text")) {
    return "ONE_OF_MISMATCH";
  }
  return null;
}

function compareOrdinal(left, right) {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}

function mapAjvIssues(value, issues, reasonPriority) {
  const discriminator = discriminatorReason(value);
  if (discriminator !== null) return discriminator;

  const priority = new Map(reasonPriority.map((reason, index) => [reason, index]));
  const mapped = issues.map((issue) => {
    const reason = AJV_KEYWORD_REASONS.get(issue.keyword) ?? "SCHEMA_INVALID";
    return {
      reason: priority.has(reason) ? reason : "SCHEMA_INVALID",
      instancePath: typeof issue.instancePath === "string" ? issue.instancePath : "",
    };
  });
  mapped.sort((left, right) => {
    const priorityDifference =
      (priority.get(left.reason) ?? Number.MAX_SAFE_INTEGER)
      - (priority.get(right.reason) ?? Number.MAX_SAFE_INTEGER);
    return priorityDifference || compareOrdinal(left.instancePath, right.instancePath);
  });
  return mapped[0]?.reason ?? "SCHEMA_INVALID";
}

function discriminatorOf(value) {
  const discriminator = value?.variant?.kind;
  return typeof discriminator === "string" ? discriminator : null;
}

function result({ accepted, reasonCategory, rejectionStage, validatorInvoked, value }) {
  return {
    accepted,
    reasonCategory,
    rejectionStage,
    validatorInvoked,
    discriminator: discriminatorOf(value),
  };
}

export function createQualificationValidator({
  rootSchema,
  resources,
  parserLimits,
  reasonPriority,
  onValidatorInvoke = () => {},
}) {
  if (!Array.isArray(resources)) {
    throw new TypeError("Qualification resources must be an array.");
  }
  if (!Array.isArray(reasonPriority)) {
    throw new TypeError("Qualification reasonPriority must be an array.");
  }
  if (typeof onValidatorInvoke !== "function") {
    throw new TypeError("Qualification onValidatorInvoke must be a function.");
  }

  const ajv = new Ajv2020({
    allErrors: true,
    allowUnionTypes: false,
    strict: true,
    validateFormats: false,
  });
  for (const resource of resources) ajv.addSchema(resource);
  const validate = ajv.compile(rootSchema);

  return Object.freeze({
    validateSource(source) {
      let value;
      try {
        value = parseStrictJson(source, parserLimits);
      } catch (error) {
        if (!(error instanceof StrictJsonError)) throw error;
        return result({
          accepted: false,
          reasonCategory: STRICT_PARSER_REASONS.get(error.code) ?? "SCHEMA_INVALID",
          rejectionStage: "strict_parser",
          validatorInvoked: false,
          value: null,
        });
      }

      onValidatorInvoke();
      if (validate(value)) {
        return result({
          accepted: true,
          reasonCategory: null,
          rejectionStage: "none",
          validatorInvoked: true,
          value,
        });
      }

      return result({
        accepted: false,
        reasonCategory: mapAjvIssues(value, validate.errors ?? [], reasonPriority),
        rejectionStage: "structural_validator",
        validatorInvoked: true,
        value,
      });
    },
  });
}
