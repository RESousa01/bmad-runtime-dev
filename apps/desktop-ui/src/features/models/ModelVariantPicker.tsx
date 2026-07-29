/**
 * FND-050 Phase A: the variant row within one profile.
 *
 * A profile's variants are named operating points (reasoning effort, output
 * ceiling), not separate models -- so this renders as a single-select group
 * scoped to the profile, using the same `aria-pressed` row pattern
 * `AgentSelector` already uses for its own rows rather than inventing a
 * second selection idiom in the same popover.
 */

import type { ModelVariantProjection } from "../../lib/hostClient/modelProtocol";

function reasoningEffortLabel(effort: ModelVariantProjection["reasoningEffort"]): string {
  switch (effort) {
    case "none": return "No reasoning";
    case "low": return "Low reasoning";
    case "medium": return "Medium reasoning";
    case "high": return "High reasoning";
  }
}

export interface ModelVariantPickerProps {
  readonly profileId: string;
  readonly variants: readonly ModelVariantProjection[];
  readonly defaultVariantId: string;
  readonly selectedVariantId: string | null;
  readonly onSelectVariant: (variantId: string) => void;
}

export function ModelVariantPicker({
  profileId,
  variants,
  defaultVariantId,
  selectedVariantId,
  onSelectVariant,
}: ModelVariantPickerProps) {
  if (variants.length <= 1) return null;
  return (
    <div
      className="model-picker__variants"
      role="group"
      aria-label={`Variants for ${profileId}`}
    >
      {variants.map((variant) => {
        const isSelected = selectedVariantId
          ? selectedVariantId === variant.variantId
          : variant.variantId === defaultVariantId;
        return (
          <button
            aria-pressed={isSelected}
            className="model-picker__variant"
            key={variant.variantId}
            onClick={(event) => {
              event.stopPropagation();
              onSelectVariant(variant.variantId);
            }}
            title={`${reasoningEffortLabel(variant.reasoningEffort)} · ${variant.maxOutputTokens.toLocaleString("en-US")} max output tokens`}
            type="button"
          >
            {variant.displayName}
            {variant.variantId === defaultVariantId ? (
              <span className="model-picker__variant-default" aria-hidden="true">·default</span>
            ) : null}
          </button>
        );
      })}
    </div>
  );
}
