/**
 * FND-050 Phase A: capabilities and limits, rendered without inventing a
 * number the catalog did not publish.
 *
 * `ModelLimitsProjection`'s four fields are each independently nullable, and
 * `null` is load-bearing: it means the organization published no qualified
 * value, not zero and not unlimited. This module is the one place that
 * distinction gets turned into text, so every other caller can trust
 * "Unknown" was written deliberately rather than as a fallback for a missing
 * case.
 */

import type {
  ModelCapabilitiesProjection,
  ModelLimitsProjection,
} from "../../lib/hostClient/modelProtocol";

const CAPABILITY_LABELS: ReadonlyArray<{
  readonly key: keyof ModelCapabilitiesProjection;
  readonly label: string;
}> = [
  { key: "textInput", label: "Text" },
  { key: "imageInput", label: "Images" },
  { key: "pdfInput", label: "PDFs" },
  { key: "attachments", label: "Attachments" },
  { key: "toolCalling", label: "Tool calling" },
  { key: "structuredOutput", label: "Structured output" },
  { key: "reasoning", label: "Reasoning" },
];

export interface ModelCapabilityTagsProps {
  readonly capabilities: ModelCapabilitiesProjection;
}

/// Only the capabilities the profile actually has, as a tag list -- omitting
/// what it lacks reads cleaner than a full grid of checks and crosses, and
/// nothing here is a claim about anything the profile does not do.
export function ModelCapabilityTags({ capabilities }: ModelCapabilityTagsProps) {
  const present = CAPABILITY_LABELS.filter(({ key }) => capabilities[key]);
  if (present.length === 0) {
    return <span className="model-picker__capabilities model-picker__capabilities--none">No published capabilities</span>;
  }
  return (
    <ul className="model-picker__capabilities" aria-label="Capabilities">
      {present.map(({ key, label }) => (
        <li className="model-picker__capability-tag" key={key}>{label}</li>
      ))}
    </ul>
  );
}

function formatTokenCount(value: number | null): string {
  if (value === null) return "Unknown";
  return `${value.toLocaleString("en-US")} tokens`;
}

function formatByteCount(value: number | null): string {
  if (value === null) return "Unknown";
  if (value >= 1_048_576) return `${(value / 1_048_576).toFixed(1)} MB`;
  if (value >= 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${value} bytes`;
}

export interface ModelLimitsListProps {
  readonly limits: ModelLimitsProjection;
}

/// Every row renders even when the value is `null` -- silently dropping a
/// row for an unknown limit would look identical to "this profile has no
/// such limit at all", which is a different, unverified claim.
export function ModelLimitsList({ limits }: ModelLimitsListProps) {
  return (
    <dl className="model-picker__limits" aria-label="Limits">
      <div className="model-picker__limit-row">
        <dt>Context window</dt>
        <dd>{formatTokenCount(limits.contextTokens)}</dd>
      </div>
      <div className="model-picker__limit-row">
        <dt>Max input</dt>
        <dd>{formatTokenCount(limits.maxInputTokens)}</dd>
      </div>
      <div className="model-picker__limit-row">
        <dt>Max output</dt>
        <dd>{formatTokenCount(limits.maxOutputTokens)}</dd>
      </div>
      <div className="model-picker__limit-row">
        <dt>Max attachment size</dt>
        <dd>{formatByteCount(limits.maxAttachmentBytes)}</dd>
      </div>
    </dl>
  );
}
