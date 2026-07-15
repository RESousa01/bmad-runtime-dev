// @vitest-environment jsdom
import "../test/setup";
import { render, screen, within } from "@testing-library/react";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import type {
  BmadLibraryProjection,
  BmadLibraryUiState,
} from "../lib/bmadProjection";
import { BmadLibraryPanel } from "./BmadLibraryPanel";

const baseProjection: BmadLibraryProjection = {
  schemaVersion: "bmad-library-snapshot.v1",
  scope: "installed_method",
  source: {
    sourceKind: "sealed_foundation",
    packageName: "bmad-method",
    packageVersion: "6.10.0",
  },
  installedSkills: [
    {
      moduleCode: "bmm",
      skillName: "bmad-architecture",
      displayName: "Create Architecture",
      description: "Create a bounded architecture spine.",
      actions: ["create"],
      entrypointKind: "step_jit",
      distributionProfile: "sapphirus_package",
      installProfile: "SapphirusManagedV1",
      validationProfile: "MethodStepWorkflowV6",
      availability: "capability_disabled",
      blockerCodes: ["bmad_capability_disabled"],
      hiddenFromHelp: false,
    },
    {
      moduleCode: "core",
      skillName: "bmad-help",
      displayName: "BMad Help",
      description: "Provide source-grounded guidance.",
      actions: [],
      entrypointKind: "direct",
      distributionProfile: "sapphirus_package",
      installProfile: "SapphirusManagedV1",
      validationProfile: "MethodOfficialSkillV6",
      availability: "capability_disabled",
      blockerCodes: ["bmad_capability_disabled"],
      hiddenFromHelp: false,
    },
  ],
  helpActions: [
    {
      moduleCode: "bmm",
      skillName: "bmad-architecture",
      action: "create",
      displayName: "Architecture",
      menuCode: "DP",
      description: "Prepare the architecture decision record.",
      requiredGuidance: true,
      expectedArtifacts: ["architecture"],
      availability: "capability_disabled",
      blockerCodes: ["bmad_capability_disabled"],
    },
    {
      moduleCode: "core",
      skillName: "bmad-help",
      action: null,
      displayName: "BMad Help",
      menuCode: "DP",
      description: "Review Method guidance.",
      requiredGuidance: false,
      expectedArtifacts: [],
      availability: "capability_disabled",
      blockerCodes: ["bmad_capability_disabled"],
    },
  ],
  methodAgents: [
    {
      moduleCode: "bmm",
      agentCode: "bmad-agent-architect",
      name: "Winston",
      title: "System Architect",
      icon: "🏛️",
      team: "BMAD Method",
      description: "Measured, with explicit trade-offs.",
      availability: "capability_disabled",
      blockerCodes: ["bmad_capability_disabled"],
      menus: [
        {
          code: "CA",
          description: "Create the architecture spine.",
          targetKind: "skill_target",
          displayLabel: "Create Architecture",
          availability: "capability_disabled",
          availabilityReason: "bmad_capability_disabled",
        },
      ],
    },
    {
      moduleCode: "bmm",
      agentCode: "bmad-agent-tech-writer",
      name: "Paige",
      title: "Technical Writer",
      icon: "📚",
      team: "BMAD Method",
      description: "Patient documentation guidance.",
      availability: "source_prompt_unavailable",
      blockerCodes: ["bmad_source_prompt_unavailable"],
      menus: ["WD", "MG", "VD", "EC"].map((code) => ({
        code,
        description: `Descriptive prompt reference ${code}.`,
        targetKind: "prompt_reference" as const,
        displayLabel: "Source prompt reference",
        availability: "source_prompt_unavailable" as const,
        availabilityReason: "bmad_source_prompt_unavailable",
      })),
    },
  ],
  nextCursor: null,
};

function readyState(
  overrides: Partial<BmadLibraryProjection> = {},
): BmadLibraryUiState {
  return {
    kind: "ready",
    projection: { ...baseProjection, ...overrides },
  };
}

describe("BmadLibraryPanel", () => {
  it("keeps installed skills, available actions, and Method agents separate", () => {
    render(<BmadLibraryPanel state={readyState()} />);

    const skills = screen.getByRole("region", { name: "Installed skills" });
    const actions = screen.getByRole("region", { name: "Available actions" });
    const agents = screen.getByRole("region", { name: "Method agents" });

    expect(within(skills).getByText("Create Architecture")).toBeTruthy();
    expect(within(skills).queryByText("Architecture")).toBeNull();
    expect(within(actions).getByText("Architecture")).toBeTruthy();
    expect(within(actions).queryByText("Create Architecture")).toBeNull();
    expect(within(agents).getByText("Winston")).toBeTruthy();
    expect(within(agents).getByText("System Architect")).toBeTruthy();
  });

  it("scopes duplicate menu aliases by module, skill, and action identity", () => {
    render(<BmadLibraryPanel state={readyState()} />);

    const actions = screen.getByRole("region", { name: "Available actions" });
    expect(within(actions).getAllByText("Menu code DP")).toHaveLength(2);

    const identifiers = screen.getByText("Internal identifiers").closest("details");
    expect(identifiers).not.toBeNull();
    expect(identifiers).toHaveProperty(
      "textContent",
      expect.stringContaining("bmm / bmad-architecture / create / DP"),
    );
    expect(identifiers).toHaveProperty(
      "textContent",
      expect.stringContaining("core / bmad-help / no action / DP"),
    );
  });

  it("renders Paige prompt references as unavailable descriptive rows", () => {
    render(<BmadLibraryPanel state={readyState()} />);

    const paige = screen.getByRole("listitem", { name: "Paige, Technical Writer" });
    expect(within(paige).getAllByText("Source prompt reference — unavailable")).toHaveLength(4);
    expect(within(paige).getAllByText("Source prompt unavailable")).toHaveLength(5);
    expect(within(paige).queryByRole("button")).toBeNull();
    expect(within(paige).queryByRole("link")).toBeNull();
  });

  it("renders HTML-like projected strings as inert text", () => {
    const malicious = "<img src=x onerror=alert('unsafe')>";
    const projection: BmadLibraryProjection = {
      ...baseProjection,
      installedSkills: [{
        ...baseProjection.installedSkills[0]!,
        displayName: malicious,
        description: "<script>window.pwned = true</script>",
      }],
      helpActions: [],
      methodAgents: [],
    };
    const { container } = render(<BmadLibraryPanel state={readyState(projection)} />);

    expect(screen.getByText(malicious)).toBeTruthy();
    expect(screen.getByText("<script>window.pwned = true</script>")).toBeTruthy();
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("script")).toBeNull();
  });

  it("retains full max-bound labels and descriptions for wrapping and assistive text", () => {
    const longLabel = "L".repeat(256);
    const longDescription = "D".repeat(2_048);
    render(
      <BmadLibraryPanel
        state={readyState({
          installedSkills: [{
            ...baseProjection.installedSkills[0]!,
            displayName: longLabel,
            description: longDescription,
          }],
          helpActions: [],
          methodAgents: [],
        })}
      />,
    );

    expect(screen.getByText(longLabel)).toHaveProperty("textContent", longLabel);
    expect(screen.getByText(longDescription)).toHaveProperty("textContent", longDescription);
  });

  it("shows independent empty states for each catalog family", () => {
    render(
      <BmadLibraryPanel
        state={readyState({
          installedSkills: [],
          helpActions: [],
          methodAgents: [],
        })}
      />,
    );

    expect(screen.getByText("No installed skills available.")).toBeTruthy();
    expect(screen.getByText("No available actions.")).toBeTruthy();
    expect(screen.getByText("No Method agents available.")).toBeTruthy();
  });

  it("supports loading and unavailable projection states", () => {
    const { rerender } = render(<BmadLibraryPanel state={{ kind: "loading" }} />);
    expect(screen.getByRole("status")).toHaveProperty(
      "textContent",
      expect.stringContaining("Loading Method library"),
    );

    const onReload = vi.fn();
    rerender(
      <BmadLibraryPanel
        onReload={onReload}
        state={{
          kind: "unavailable",
          message: "The Method catalog is unavailable.",
          retryable: true,
        }}
      />,
    );
    expect(screen.getByRole("alert")).toHaveProperty(
      "textContent",
      expect.stringContaining("The Method catalog is unavailable."),
    );
    expect(screen.getByRole("button", { name: "Reload Method library" })).toBeTruthy();
  });

  it("shows textual availability without execution, installation, or activation affordances", () => {
    const { container } = render(<BmadLibraryPanel state={readyState()} />);

    expect(screen.getAllByText("Capability disabled").length).toBeGreaterThan(0);
    expect(container.querySelector("button, a, input, select, textarea")).toBeNull();
    expect(document.body.textContent).not.toMatch(
      /\b(?:Chat|Run|Execute|Install|Activate|Convert|Evaluate)\b|Approve & apply locally/i,
    );
  });

  it("has no automated accessibility violations in the ready state", async () => {
    const { container } = render(<BmadLibraryPanel state={readyState()} />);
    const results = await axe.run(container, {
      rules: { "color-contrast": { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });
});
