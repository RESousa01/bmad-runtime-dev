/**
 * FND-050 Phase A: one selectable profile row.
 *
 * Deliberately not a card -- the repo's minimal visual language uses flat
 * rows with `aria-pressed`, same as every other selector in this popover.
 * A row here always shows availability, capabilities, and limits: hiding any
 * of the three because they seemed unremarkable would be the same mistake
 * as rendering a `null` limit as zero -- a silent claim nobody verified.
 */

import type { ModelProfileProjection } from "../../lib/hostClient/modelProtocol";
import { ModelAvailabilityBadge } from "./ModelAvailability";
import { ModelCapabilityTags, ModelLimitsList } from "./ModelCapabilityDetails";
import { ModelVariantPicker } from "./ModelVariantPicker";

export interface ModelProfileRowProps {
  readonly profile: ModelProfileProjection;
  readonly isSelected: boolean;
  readonly isDefault: boolean;
  readonly isRecent: boolean;
  readonly selectedVariantId: string | null;
  readonly onSelectProfile: (profileId: string, variantId: string) => void;
}

export function ModelProfileRow({
  profile,
  isSelected,
  isDefault,
  isRecent,
  selectedVariantId,
  onSelectProfile,
}: ModelProfileRowProps) {
  const variantId = isSelected && selectedVariantId
    ? selectedVariantId
    : profile.defaultVariantId;
  return (
    <li className="model-picker__profile">
      <button
        aria-pressed={isSelected}
        className="model-picker__profile-row"
        onClick={() => onSelectProfile(profile.profileId, variantId)}
        type="button"
      >
        <span className="model-picker__profile-heading">
          <span className="model-picker__profile-name">{profile.displayName}</span>
          {isDefault ? (
            <span className="model-picker__profile-tag" aria-hidden="true">Default</span>
          ) : null}
          {isRecent && !isDefault ? (
            <span className="model-picker__profile-tag" aria-hidden="true">Recent</span>
          ) : null}
          <ModelAvailabilityBadge
            availability={profile.availability}
            statusDetail={profile.statusDetail}
          />
        </span>
        {profile.description ? (
          <span className="model-picker__profile-description">{profile.description}</span>
        ) : null}
        <ModelCapabilityTags capabilities={profile.capabilities} />
        <ModelLimitsList limits={profile.limits} />
      </button>
      <ModelVariantPicker
        profileId={profile.profileId}
        variants={profile.variants}
        defaultVariantId={profile.defaultVariantId}
        selectedVariantId={isSelected ? selectedVariantId : null}
        onSelectVariant={(nextVariantId) => onSelectProfile(profile.profileId, nextVariantId)}
      />
    </li>
  );
}
