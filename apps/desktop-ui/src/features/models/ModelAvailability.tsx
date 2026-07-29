/**
 * FND-050 Phase A: how availability reads out loud.
 *
 * A profile's `availability` is a closed two-value vocabulary
 * (`ModelSelectableAvailability`) once it is selectable at all -- "degraded"
 * carries a `statusDetail` the operator wrote, which is the only accurate
 * sentence to show, never a generic guess. An unavailable profile carries a
 * different closed vocabulary (`ModelBlockedAvailability`) plus a
 * `ModelRecoveryAction`; both are rendered here as a paired reason/next-step,
 * because a reason alone leaves the user guessing what to do about it.
 */

import type {
  ModelBlockedAvailability,
  ModelRecoveryAction,
  ModelSelectableAvailability,
} from "../../lib/hostClient/modelProtocol";

export function selectableAvailabilityLabel(
  availability: ModelSelectableAvailability,
): string {
  switch (availability) {
    case "available":
      return "Available";
    case "degraded":
      return "Degraded";
  }
}

function blockedAvailabilityLabel(availability: ModelBlockedAvailability): string {
  switch (availability) {
    case "policy_blocked":
      return "Blocked by organization policy";
    case "not_entitled":
      return "Not entitled";
    case "unqualified":
      return "Not yet qualified";
    case "deployment_missing":
      return "Deployment missing";
    case "catalog_stale":
      return "Catalog is stale";
  }
}

function recoveryActionLabel(action: ModelRecoveryAction): string {
  switch (action) {
    case "refresh_catalog":
      return "Refresh the catalog to check again.";
    case "request_entitlement":
      return "Request entitlement from your administrator.";
    case "awaiting_qualification":
      return "Awaiting qualification -- no action to take yet.";
    case "contact_administrator":
      return "Contact your administrator.";
    case "select_another_profile":
      return "Choose a different model.";
  }
}

export interface ModelAvailabilityBadgeProps {
  readonly availability: ModelSelectableAvailability;
  readonly statusDetail: string | null;
}

/// A short badge plus, when the profile is degraded, the operator's own
/// sentence explaining why -- never a placeholder invented here.
export function ModelAvailabilityBadge({
  availability,
  statusDetail,
}: ModelAvailabilityBadgeProps) {
  return (
    <span
      className={`model-picker__availability model-picker__availability--${availability}`}
    >
      <span className="model-picker__availability-label">
        {selectableAvailabilityLabel(availability)}
      </span>
      {availability === "degraded" && statusDetail ? (
        <span className="model-picker__availability-detail">{statusDetail}</span>
      ) : null}
    </span>
  );
}

export interface ModelUnavailableReasonProps {
  readonly displayName: string;
  readonly availability: ModelBlockedAvailability;
  readonly recoveryAction: ModelRecoveryAction;
}

/// One row in the "not offered to you" list: what it is, why, and the one
/// thing that would change it.
export function ModelUnavailableReason({
  displayName,
  availability,
  recoveryAction,
}: ModelUnavailableReasonProps) {
  return (
    <li className="model-picker__unavailable-row">
      <span className="model-picker__unavailable-name">{displayName}</span>
      <span className="model-picker__unavailable-reason">
        {blockedAvailabilityLabel(availability)}
      </span>
      <span className="model-picker__unavailable-recovery">
        {recoveryActionLabel(recoveryAction)}
      </span>
    </li>
  );
}
