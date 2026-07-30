import { describe, expect, it } from "vitest";

import {
  featureCommands,
  isAvailable,
  resolveCapabilities,
  type CapabilityInput,
  type HostCommandName,
} from "../../../app/capabilityMatrix";
import { capabilityFidelity } from "../../../lib/bmadFidelity";
import {
  initialBmadRequestState,
  transitionBmadRequest,
} from "../../../lib/bmadModelProjection";
import type { BmadHelpRunCreatedProjection } from "../../../lib/bmadProjection";
import { parseBmadLibrarySnapshot } from "../../../lib/hostClient/bmadProtocol";

const run: BmadHelpRunCreatedProjection = {
  schemaVersion: "bmad-help-run.v1",
  runKind: "bmad_help",
  lifecycle: "created_unbound",
  workspaceId: "workspace_historical",
  runId: "run_historical",
  sessionId: "session_historical",
  currentIntent: "Which reviewed Method skill fits?",
  runnable: false,
  completionClaimed: false,
  recommendation: {
    schemaVersion: "bmad-help-recommendation.v1",
    displayName: "BMad Help",
    moduleCode: "core",
    skillName: "bmad-help",
    action: null,
    confidence: "heuristic",
    source: {
      sourceKind: "sealed_foundation",
      packageName: "bmad-method",
      packageVersion: "6.10.0",
    },
    reason: "The sealed catalog contains a related capability.",
    requiredGuidance: true,
    expectedArtifacts: ["architecture"],
    availability: "capability_disabled",
    blockerCodes: ["bmad_capability_disabled"],
    completionClaimed: false,
  },
};

const builderSnapshot = {
  schemaVersion: "bmad-library-snapshot.v2",
  scope: "installed_method",
  source: {
    sourceKind: "sealed_foundation",
    packageName: "bmad-method",
    packageVersion: "6.10.0",
  },
  installedSkills: [],
  helpActions: [],
  methodAgents: [],
  builderPackages: [
    {
      packageName: "bmad-builder",
      packageVersion: "2.1.0",
      packageKind: "agent",
      displayName: "Agent Builder",
      activationState: "installed_inactive",
      resourceCount: 3,
      descriptorDigest: "9ee163d13df2",
      blockerCodes: ["builder_engine_gated"],
    },
    {
      packageName: "bmad-builder",
      packageVersion: "2.1.0",
      packageKind: "workflow",
      displayName: "Workflow Builder",
      activationState: "installed_inactive",
      resourceCount: 2,
      descriptorDigest: "30cbe39a0600",
      blockerCodes: ["builder_engine_gated"],
    },
  ],
  coreSkills: [],
  nextCursor: null,
};

const allCommands: readonly HostCommandName[] = [
  ...featureCommands.chat,
  ...featureCommands.history,
  ...featureCommands.help,
  ...featureCommands.run,
  ...featureCommands.library,
  ...featureCommands.model,
];

function capabilityInput(overrides: Partial<CapabilityInput> = {}): CapabilityInput {
  return {
    browserPreview: false,
    hostReady: true,
    libraryLoaded: true,
    modelReady: true,
    supportedCommands: allCommands,
    workspaceGrantEpoch: 1,
    ...overrides,
  };
}

describe("SRC-05 current renderer characterization", () => {
  it("SRC-05-UI-PROJECTION-001 retains Builder packages as bounded inactive projections", () => {
    const projection = parseBmadLibrarySnapshot(builderSnapshot);
    expect(projection.builderPackages).toEqual([
      expect.objectContaining({
        packageKind: "agent",
        activationState: "installed_inactive",
        blockerCodes: ["builder_engine_gated"],
      }),
      expect.objectContaining({
        packageKind: "workflow",
        activationState: "installed_inactive",
        blockerCodes: ["builder_engine_gated"],
      }),
    ]);
    expect(JSON.stringify(projection)).not.toMatch(/fileContent|activate|installCommand/u);
  });

  it("SRC-05-UI-STATE-001 freezes cancellation, interruption, and restart recovery projections", () => {
    const interrupted = transitionBmadRequest(initialBmadRequestState, {
      type: "interrupted",
      result: run,
    });
    expect(interrupted).toEqual({ kind: "interrupted", result: run });

    const cancelled = transitionBmadRequest(interrupted, {
      type: "terminal",
      reason: "cancelled",
    });
    expect(cancelled).toEqual({ kind: "terminal", reason: "cancelled" });
    expect(transitionBmadRequest(cancelled, {
      type: "recovered",
      run,
    })).toBe(cancelled);

    const recovering = transitionBmadRequest(initialBmadRequestState, {
      type: "recover_started",
    });
    expect(transitionBmadRequest(recovering, {
      type: "recovered",
      run,
    })).toEqual({ kind: "idle", run });
  });

  it("SRC-05-UI-UNSUPPORTED-001 rejects future active Builder projections", () => {
    const unsupported = structuredClone(builderSnapshot);
    unsupported.builderPackages[0]!.activationState = "active";
    expect(() => parseBmadLibrarySnapshot(unsupported)).toThrow();
  });

  it("SRC-05-UI-FIDELITY-001 labels all five legacy Builder capabilities as inert", () => {
    for (const capabilityId of [
      "builder:agent.analyze",
      "builder:agent.create_rebuild",
      "builder:agent.edit",
      "builder:workflow.analyze",
      "builder:workflow.build_edit",
    ]) {
      expect(capabilityFidelity(capabilityId)).toEqual(expect.objectContaining({
        level: "inert_draft",
        label: "Inert draft",
      }));
    }
    expect(capabilityFidelity("bmm:bmad-architecture").level).toBe("ordered_steps");
    expect(capabilityFidelity("bmm:bmad-product-brief").level).toBe("adapted_one_shot");
  });

  it("SRC-05-UI-COUPLING-001 keeps the repaired chat closure independent from BMAD", () => {
    // The planning audit carried the historical aggregate-gate defect forward,
    // but the frozen baseline already contains its production-code repair.
    const bmadCommands = new Set<HostCommandName>([
      ...featureCommands.help,
      ...featureCommands.run,
      ...featureCommands.library,
    ]);
    const withoutBmad = allCommands.filter((command) => !bmadCommands.has(command));
    const capabilities = resolveCapabilities(capabilityInput({
      libraryLoaded: false,
      supportedCommands: withoutBmad,
    }));
    expect(isAvailable(capabilities.chat)).toBe(true);
    expect(isAvailable(capabilities.help)).toBe(false);
    expect(isAvailable(capabilities.run)).toBe(false);
  });
});
