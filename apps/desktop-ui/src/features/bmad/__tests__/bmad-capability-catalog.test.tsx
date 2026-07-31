import { describe, expect, it } from "vitest";

import {
  bmadCapabilities,
  builderActions,
  compatibilityAliases,
  uiAdvertisedBmadCapabilities,
} from "../../../generated/bmadCapabilities";

describe("canonical BMAD capability catalog", () => {
  it("projects all pinned source rows without the legacy five-action limit", () => {
    expect(bmadCapabilities.filter((row) => row.source.id === "method")).toHaveLength(48);
    expect(builderActions.filter((row) => row.visibility === "public")).toHaveLength(9);
    expect(builderActions.filter((row) => row.visibility === "internal")).toHaveLength(1);
  });

  it("advertises only public capabilities with runtime closure", () => {
    expect(
      uiAdvertisedBmadCapabilities.every(
        (row) => row.visibility === "public" && row.uiState === "runtime_available",
      ),
    ).toBe(true);
    expect(
      uiAdvertisedBmadCapabilities
        .filter((row) => row.source.id === "method")
        .map((row) => row.canonicalId),
    ).toEqual(["bmm:bmad-prd"]);
  });

  it("resolves every alias to one canonical implementation with migration policy", () => {
    const canonicalIds = new Set(bmadCapabilities.map((row) => row.canonicalId));
    const aliasIds = compatibilityAliases.map((alias) => alias.alias);

    expect(new Set(aliasIds).size).toBe(aliasIds.length);
    for (const alias of compatibilityAliases) {
      expect(canonicalIds.has(alias.target)).toBe(true);
      expect(alias.migrationPolicy.length).toBeGreaterThan(0);
    }
  });
});
