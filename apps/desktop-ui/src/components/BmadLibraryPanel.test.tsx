// @vitest-environment jsdom
import "../test/setup";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import type {
  BmadLibraryProjection,
  BmadLibraryUiState,
} from "../lib/bmadProjection";
import { BmadLibraryPanel } from "./BmadLibraryPanel";

const baseProjection: BmadLibraryProjection = {
  schemaVersion: "bmad-library-snapshot.v2",
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
          capabilityId: null,
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
        capabilityId: null,
        availability: "source_prompt_unavailable" as const,
        availabilityReason: "bmad_source_prompt_unavailable",
      })),
    },
  ],
  builderPackages: [],
  coreSkills: [],
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
  it("leads with reviewed agents and keeps raw source catalog rows collapsed", () => {
    render(<BmadLibraryPanel state={readyState()} />);

    const agents = screen.getByRole("region", { name: "Reviewed agents" });
    const sourceDetails = screen.getByText(/Source catalog details/u).closest("details");

    expect(screen.getByRole("heading", { level: 2, name: "Skills & agents" })).toBeTruthy();
    expect(sourceDetails).not.toBeNull();
    expect(sourceDetails).toHaveProperty("open", false);
    fireEvent.click(within(sourceDetails!).getByText(/Source catalog details/u));
    const skills = screen.getByRole("region", { name: "Installed skills" });
    const actions = screen.getByRole("region", { name: "Source actions" });

    expect(within(skills).getByText("Create Architecture")).toBeTruthy();
    expect(within(skills).queryByText("Architecture")).toBeNull();
    expect(within(actions).getByText("Architecture")).toBeTruthy();
    expect(within(actions).queryByText("Create Architecture")).toBeNull();
    expect(within(agents).getByText("Winston")).toBeTruthy();
    expect(within(agents).getByText("System Architect")).toBeTruthy();
  });

  it("scopes duplicate menu aliases by module, skill, and action identity", () => {
    render(<BmadLibraryPanel state={readyState()} />);

    const sourceDetails = screen.getByText(/Source catalog details/u).closest("details");
    fireEvent.click(within(sourceDetails!).getByText(/Source catalog details/u));
    const actions = screen.getByRole("region", { name: "Source actions" });
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
    expect(within(paige).getAllByText("Source-only prompt")).toHaveLength(4);
    // FE-25 changed what the four menu rows say. The projection's own word for
    // the state ("Source prompt unavailable") is now on the agent alone, and
    // each menu carries the verdict plus the sentence that explains it — which
    // is what the reader needed and what the state name never said.
    expect(within(paige).getAllByText("Source prompt unavailable")).toHaveLength(1);
    expect(within(paige).getAllByText("Cannot run here")).toHaveLength(4);
    expect(
      within(paige).getAllByText(/original source prompt has not been adopted/u).length,
    ).toBeGreaterThanOrEqual(4);
    expect(within(paige).queryByRole("button", { name: "Start reviewed run" })).toBeNull();
    expect(within(paige).queryByRole("link")).toBeNull();
  });

  it("starts only agent menus projected as available", () => {
    const onRunCapability = vi.fn();
    const architect = baseProjection.methodAgents[0]!;
    render(
      <BmadLibraryPanel
        onRunCapability={onRunCapability}
        state={readyState({
          methodAgents: [
            {
              ...architect,
              availability: "available",
              blockerCodes: [],
              menus: architect.menus.map((menu) => ({
                ...menu,
                capabilityId: "bmm:bmad-architecture",
                availability: "available",
                availabilityReason: null,
              })),
            },
            baseProjection.methodAgents[1]!,
          ],
        })}
      />,
    );

    const runs = screen.getAllByRole("button", { name: "Start reviewed run" });
    expect(runs).toHaveLength(1);
    const run = runs[0]!;
    fireEvent.click(run);
    expect(onRunCapability).toHaveBeenCalledWith(
      "bmm:bmad-architecture",
      "Create Architecture",
    );
    const paige = screen.getByRole("listitem", { name: "Paige, Technical Writer" });
    expect(within(paige).queryByRole("button", { name: "Start reviewed run" })).toBeNull();
  });

  it("labels a runnable capability as an adapted one-shot", () => {
    const architect = baseProjection.methodAgents[0]!;
    render(
      <BmadLibraryPanel
        onRunCapability={vi.fn()}
        state={readyState({
          methodAgents: [
            {
              ...architect,
              availability: "available",
              blockerCodes: [],
              menus: architect.menus.map((menu) => ({
                ...menu,
                capabilityId: "bmm:bmad-product-brief",
                availability: "available",
                availabilityReason: null,
              })),
            },
          ],
        })}
      />,
    );

    // The runnable capability is marked "Adapted" so it is not mistaken for the
    // full upstream workflow.
    expect(screen.getAllByText("Adapted").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Start reviewed run" })).toBeTruthy();
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
      builderPackages: [],
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
          builderPackages: [],
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
          builderPackages: [],
        })}
      />,
    );

    expect(screen.getByText("No installed skills available.")).toBeTruthy();
    expect(screen.getByText("No source actions recorded.")).toBeTruthy();
    expect(screen.getByText("No agents available.")).toBeTruthy();
  });

  it("uses skills and agents catalog language for loading, fallback errors, and reload", () => {
    const { rerender } = render(<BmadLibraryPanel state={{ kind: "idle" }} />);
    expect(
      screen.getByText("This workspace's skills and agents have not been read yet."),
    ).toBeTruthy();

    rerender(<BmadLibraryPanel state={{ kind: "loading" }} />);
    expect(screen.getByRole("status")).toHaveProperty(
      "textContent",
      expect.stringContaining("Loading skills and agents catalog"),
    );

    const onReload = vi.fn();
    rerender(
      <BmadLibraryPanel
        onReload={onReload}
        state={{
          kind: "unavailable",
          message: "",
          retryable: true,
        }}
      />,
    );
    expect(screen.getByRole("alert")).toHaveProperty(
      "textContent",
      expect.stringContaining("The skills and agents catalog is unavailable."),
    );
    expect(screen.getByRole("button", { name: "Reload skills and agents" })).toBeTruthy();
  });

  /// Regression guard: `idle` is reached on first paint and again after every
  /// host rebind or workspace revocation, and it rendered one sentence and no
  /// control — the catalog could never be asked for from this panel.
  it("offers a read control from the unrequested state", async () => {
    const onReload = vi.fn();
    const user = userEvent.setup();
    render(<BmadLibraryPanel onReload={onReload} state={{ kind: "idle" }} />);

    await user.click(screen.getByRole("button", { name: "Read skills and agents" }));
    expect(onReload).toHaveBeenCalledOnce();
  });

  /// Regression guard: the reload was gated on `retryable`, so every failure
  /// the host did not mark retryable left the panel with an alert and no way
  /// out. The transport reported *every* failure that way until recently.
  it("still offers a reload when the failure was not marked retryable", async () => {
    const onReload = vi.fn();
    const user = userEvent.setup();
    render(
      <BmadLibraryPanel
        onReload={onReload}
        state={{
          kind: "unavailable",
          message: "Connect a model provider in Settings.",
          retryable: false,
        }}
      />,
    );

    expect(screen.getByRole("alert").textContent).toContain("Connect a model provider");
    expect(
      screen.getByText(/did not report this as a temporary failure/u),
    ).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Reload skills and agents" }));
    expect(onReload).toHaveBeenCalledOnce();
  });

  /// The user's own question was "it says start reviewed run, whats that for".
  /// The label names a mechanism; nothing on screen said what was reviewed.
  it("explains what a reviewed run does and points every start control at it", () => {
    const architect = baseProjection.methodAgents[0]!;
    render(
      <BmadLibraryPanel
        onRunCapability={vi.fn()}
        state={readyState({
          methodAgents: [
            {
              ...architect,
              availability: "available",
              blockerCodes: [],
              menus: architect.menus.map((menu) => ({
                ...menu,
                capabilityId: "bmm:bmad-architecture",
                availability: "available" as const,
                availabilityReason: null,
              })),
            },
          ],
        })}
      />,
    );

    const help = screen.getByText(/does not send your workspace/u);
    expect(help.textContent).toContain("approve them");
    expect(help.textContent).toContain("approve in Changes before anything is written");

    const start = screen.getByRole("button", { name: "Start reviewed run" });
    // Guard against the both-null form of this assertion passing vacuously.
    expect(start.getAttribute("aria-describedby")).toBeTruthy();
    expect(start.getAttribute("aria-describedby")).toBe(help.getAttribute("id"));
  });

  it("keeps the reviewed-run explanation off a catalog with nothing startable", () => {
    render(<BmadLibraryPanel onRunCapability={vi.fn()} state={readyState()} />);

    expect(screen.queryByText(/does not send your workspace/u)).toBeNull();
  });

  /// Regression guard: an unrecognised blocker rendered as a bare identifier
  /// and nothing else — the least actionable thing this panel can show.
  it("explains every blocker, including one it has no wording for", () => {
    render(
      <BmadLibraryPanel
        state={readyState({
          installedSkills: [{
            ...baseProjection.installedSkills[0]!,
            blockerCodes: ["bmad_capability_disabled", "bmad_future_code"] as never,
          }],
          helpActions: [],
          methodAgents: [],
        })}
      />,
    );

    const sourceDetails = screen.getByText(/Source catalog details/u).closest("details");
    fireEvent.click(within(sourceDetails!).getByText(/Source catalog details/u));
    const blockers = screen.getByRole("list", { name: "Blockers" });

    expect(within(blockers).getByText(/not enabled in the sealed foundation/u)).toBeTruthy();
    expect(within(blockers).getByText(/gives no further reason/u)).toBeTruthy();
    // The identifier is kept as evidence, but it is no longer the whole answer.
    expect(within(blockers).getByText("bmad_future_code")).toBeTruthy();
  });

  /// Regression guard: the fidelity note lived only in a `title` attribute, so
  /// the honesty marker read as the bare word "Adapted" to keyboard and screen
  /// reader users.
  it("exposes the fidelity note to assistive technology, not only as a tooltip", () => {
    render(
      <BmadLibraryPanel
        onRunCapability={vi.fn()}
        state={readyState({
          coreSkills: [{
            capabilityId: "bmm:bmad-spec",
            skill: "bmad-spec",
            displayName: "Spec",
            menuCode: "SP",
          }],
          methodAgents: [],
        })}
      />,
    );

    const badge = screen.getByText("Adapted");
    expect(badge.textContent).toContain(
      "A single reviewed, one-shot model request — not the full upstream multi-step BMAD workflow.",
    );
  });

  it("gives an agent's reviewed and source-only action lists distinct names", () => {
    const architect = baseProjection.methodAgents[0]!;
    render(
      <BmadLibraryPanel
        onRunCapability={vi.fn()}
        state={readyState({
          methodAgents: [
            {
              ...architect,
              availability: "available",
              blockerCodes: [],
              menus: [
                {
                  ...architect.menus[0]!,
                  capabilityId: "bmm:bmad-architecture",
                  availability: "available" as const,
                  availabilityReason: null,
                },
                { ...architect.menus[0]!, code: "XX" },
              ],
            },
          ],
        })}
      />,
    );

    expect(screen.getByRole("list", { name: "Winston reviewed actions" })).toBeTruthy();
    // Renamed with FE-25: "source-only" asserted *why* a menu was not startable,
    // and it was wrong for every skill-target menu the build could not bind.
    fireEvent.click(screen.getByText(/1 action that cannot be started here/u));
    expect(
      screen.getByRole("list", { name: "Winston actions that cannot be started" }),
    ).toBeTruthy();
  });

  it("shows textual availability without execution, installation, or activation affordances", () => {
    const { container } = render(<BmadLibraryPanel state={readyState()} />);

    expect(screen.getAllByText("Capability disabled").length).toBeGreaterThan(0);
    expect(screen.getByText(/Source catalog details/u).closest("details")).toHaveProperty("open", false);
    expect(container.querySelector("button, a, input, select, textarea")).toBeNull();
    // The verdict label is removed before the word check rather than dropped
    // from it. "Cannot run here" is a refusal, not an affordance — but any
    // *other* occurrence of these verbs still fails, which is what this guard
    // was for.
    const withoutRefusals = (document.body.textContent ?? "").replaceAll("Cannot run here", "");
    expect(withoutRefusals).not.toMatch(
      /\b(?:Chat|Run|Execute|Install|Activate|Convert|Evaluate)\b|Approve & apply locally/i,
    );
  });

  it("starts the five reviewed Builder authoring operations while keeping outputs inactive", () => {
    const onRunBuilderCapability = vi.fn();
    render(
      <BmadLibraryPanel
        onRunBuilderCapability={onRunBuilderCapability}
        state={readyState({
          builderPackages: [
            {
              packageName: "bmad-builder",
              packageVersion: "2.1.0",
              packageKind: "agent",
              displayName: "Builder agent",
              activationState: "installed_inactive",
              resourceCount: 4,
              blockerCodes: ["builder_engine_gated"],
              descriptorDigest: "0123456789ab",
            },
            {
              packageName: "bmad-builder",
              packageVersion: "2.1.0",
              packageKind: "workflow",
              displayName: "Builder workflow",
              activationState: "installed_inactive",
              resourceCount: 2,
              blockerCodes: ["builder_engine_gated"],
              descriptorDigest: "abcdef012345",
            },
          ],
        })}
      />,
    );

    expect(screen.getAllByText("Inactive output")).toHaveLength(2);
    const create = screen.getByRole("button", { name: "Create or rebuild agent" });
    fireEvent.click(create);
    expect(onRunBuilderCapability).toHaveBeenCalledWith(
      "builder:agent.create_rebuild",
      "Create or rebuild agent",
    );
    expect(screen.getAllByRole("button", { name: /agent|workflow/i })).toHaveLength(5);
  });

  /// FE-25. The old panel split an agent's menus on `availability ===
  /// "available" && capabilityId !== null`, so a menu the catalog calls
  /// available while naming nothing to start was filed under "source-only
  /// actions" — a heading that asserts *why*, about a case where nothing knows
  /// why. A user cannot act on a row that says the wrong thing any more than on
  /// a row that is not there.
  it("says a menu with nothing to start is not known, rather than calling it source-only", () => {
    const architect = baseProjection.methodAgents[0]!;
    render(
      <BmadLibraryPanel
        onRunCapability={vi.fn()}
        state={readyState({
          methodAgents: [{
            ...architect,
            availability: "available",
            blockerCodes: [],
            menus: [{
              ...architect.menus[0]!,
              availability: "available" as const,
              availabilityReason: null,
              capabilityId: null,
            }],
          }],
        })}
      />,
    );

    const winston = screen.getByRole("listitem", { name: "Winston, System Architect" });
    // Listed, with its own identity, not absent and not renamed.
    expect(within(winston).getByText("CA")).toBeTruthy();
    expect(within(winston).getByText("Skill target · Create Architecture")).toBeTruthy();
    expect(within(winston).getByText("Not known")).toBeTruthy();
    expect(
      within(winston).getByText(/neither the catalog nor the recorded coverage matrix names a capability/u),
    ).toBeTruthy();
    // Not "Cannot run here": nothing here established that it cannot, only that
    // the two sources contradict each other.
    expect(within(winston).queryByText("Cannot run here")).toBeNull();
    expect(within(winston).queryByRole("button", { name: "Start reviewed run" })).toBeNull();
  });

  /// The other half of the same guard: a verdict that *is* a denial says so,
  /// with the sentence the host's own state name never carried — and keeps the
  /// blocker code underneath as evidence rather than as the answer.
  it("says a blocked menu cannot run, and why, where the menu is listed", () => {
    render(<BmadLibraryPanel onRunCapability={vi.fn()} state={readyState()} />);

    const winston = screen.getByRole("listitem", { name: "Winston, System Architect" });
    fireEvent.click(within(winston).getByText(/1 action that cannot be started here/u));
    const refused = within(winston).getByRole("list", {
      name: "Winston actions that cannot be started",
    });

    expect(within(refused).getByText("Cannot run here")).toBeTruthy();
    expect(
      within(refused).getByText(/not enabled in the sealed foundation, so it cannot be started here/u),
    ).toBeTruthy();
    expect(within(refused).getByText("bmad_capability_disabled")).toBeTruthy();
    expect(within(refused).queryByText("Not known")).toBeNull();
    expect(within(refused).queryByRole("button", { name: "Start reviewed run" })).toBeNull();
  });

  /// A menu carries no upstream skill name — only a code, a label and, when the
  /// build binds one, a capability — so the matrix is asked by capability here.
  /// Asked by skill it would answer "no row for this skill" about a capability
  /// it holds two rows for.
  it("names the upstream skills behind a startable menu's capability", () => {
    const architect = baseProjection.methodAgents[0]!;
    render(
      <BmadLibraryPanel
        onRunCapability={vi.fn()}
        state={readyState({
          methodAgents: [{
            ...architect,
            availability: "available",
            blockerCodes: [],
            menus: architect.menus.map((menu) => ({
              ...menu,
              capabilityId: "bmm:bmad-architecture",
              availability: "available" as const,
              availabilityReason: null,
            })),
          }],
        })}
      />,
    );

    const reviewed = screen.getByRole("list", { name: "Winston reviewed actions" });
    expect(
      within(reviewed).getByText(
        /Projected from the upstream skill bmad-architecture; the legacy name bmad-create-architecture forwards to it/u,
      ),
    ).toBeTruthy();
    expect(
      within(reviewed).getByText(/No compatibility decision has been recorded/u),
    ).toBeTruthy();
  });

  /// Two recorded sources disagreeing is not a tie broken by whichever is read
  /// first. The core-skill start used to be unconditional, so a contradiction
  /// like this one was discovered only after the user had chosen and approved a
  /// context for it.
  it("refuses a core skill the coverage matrix contradicts, without calling it broken", () => {
    const onRunCapability = vi.fn();
    render(
      <BmadLibraryPanel
        onRunCapability={onRunCapability}
        state={readyState({
          coreSkills: [
            { capabilityId: "bmm:bmad-spec", skill: "bmad-spec", displayName: "Spec", menuCode: "SP" },
            {
              capabilityId: "bmm:bmad-sprint-status",
              skill: "bmad-sprint-status",
              displayName: "Sprint status",
              menuCode: "SS",
            },
          ],
          methodAgents: [],
        })}
      />,
    );

    const core = screen.getByRole("region", { name: "Core skills" });
    expect(within(core).getByText("Sprint status")).toBeTruthy();
    expect(within(core).getByText("Not known")).toBeTruthy();
    expect(within(core).getByText(/One of the two is out of date/u)).toBeTruthy();
    // Exactly one start remains, and it is the skill both sources agree on.
    const starts = within(core).getAllByRole("button", { name: "Start reviewed run" });
    expect(starts).toHaveLength(1);
    fireEvent.click(starts[0]!);
    expect(onRunCapability).toHaveBeenCalledWith("bmm:bmad-spec", "Spec");
  });

  /// The matrix's denominator is upstream BMAD directories, so a Forge-native
  /// core skill having no row is silence, not a denial. Reading it as one would
  /// withdraw the start controls from `bmm:bmad-shard-doc` and
  /// `bmm:bmad-index-docs` — two skills this build has always been able to run.
  it("keeps a core skill the matrix never covered startable, and says the matrix is silent", () => {
    const onRunCapability = vi.fn();
    render(
      <BmadLibraryPanel
        onRunCapability={onRunCapability}
        state={readyState({
          coreSkills: [{
            capabilityId: "bmm:bmad-shard-doc",
            skill: "bmad-shard-doc",
            displayName: "Shard document",
            menuCode: "SD",
          }],
          methodAgents: [],
        })}
      />,
    );

    const core = screen.getByRole("region", { name: "Core skills" });
    expect(within(core).getByText("Runs here")).toBeTruthy();
    expect(within(core).getByText(/no row for this skill/u)).toBeTruthy();
    fireEvent.click(within(core).getByRole("button", { name: "Start reviewed run" }));
    expect(onRunCapability).toHaveBeenCalledWith("bmm:bmad-shard-doc", "Shard document");
  });

  /// The help text explains a control. A catalog whose every start the verdict
  /// refused has no such control, and instructions for a button that is not on
  /// the page read as a button the reader has failed to find.
  it("keeps the reviewed-run explanation off a catalog whose every start was refused", () => {
    render(
      <BmadLibraryPanel
        onRunCapability={vi.fn()}
        state={readyState({
          coreSkills: [{
            capabilityId: "bmm:bmad-sprint-status",
            skill: "bmad-sprint-status",
            displayName: "Sprint status",
            menuCode: "SS",
          }],
          methodAgents: [],
        })}
      />,
    );

    expect(screen.getByText("Sprint status")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Start reviewed run" })).toBeNull();
    expect(screen.queryByText(/does not send your workspace/u)).toBeNull();
  });

  /// BM-03 has recorded no compatibility decision for any of the 48 upstream
  /// skills, so "Runs here" on its own would be read as a claim nobody made.
  it("says the compatibility of a startable skill is unreviewed", () => {
    render(
      <BmadLibraryPanel
        onRunCapability={vi.fn()}
        state={readyState({
          coreSkills: [{
            capabilityId: "bmm:bmad-spec",
            skill: "bmad-spec",
            displayName: "Spec",
            menuCode: "SP",
          }],
          methodAgents: [],
        })}
      />,
    );

    const core = screen.getByRole("region", { name: "Core skills" });
    expect(within(core).getByText("Runs here")).toBeTruthy();
    expect(
      within(core).getByText(/No compatibility decision has been recorded/u),
    ).toBeTruthy();
  });

  /// A build with deterministic help activates `core/bmad-help`, and the matrix
  /// records it as normalized into a package that reaches no capability. The
  /// chip used to read "Available" off the projection alone.
  it("reads a catalog entry the matrix projects nothing for as not known", () => {
    render(
      <BmadLibraryPanel
        state={readyState({
          installedSkills: [{
            ...baseProjection.installedSkills[1]!,
            availability: "available",
            blockerCodes: [],
          }],
          helpActions: [],
          methodAgents: [],
        })}
      />,
    );

    const sourceDetails = screen.getByText(/Source catalog details/u).closest("details");
    fireEvent.click(within(sourceDetails!).getByText(/Source catalog details/u));
    const skills = screen.getByRole("region", { name: "Installed skills" });

    expect(within(skills).getByText("Not known")).toBeTruthy();
    expect(within(skills).queryByText("Runs here")).toBeNull();
    expect(within(skills).getByText(/One of the two is out of date/u)).toBeTruthy();
  });

  it("has no automated accessibility violations in the ready state", async () => {
    const { container } = render(<BmadLibraryPanel state={readyState()} />);
    const results = await axe.run(container, {
      rules: { "color-contrast": { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });
});

describe("BmadLibraryPanel core skills", () => {
  /// Regression guard: the nine adopted core skills were runnable at the host
  /// but had no agent menu path, so the agent-keyed panel never listed them and
  /// no user could start one.
  it("lists the Core module menu and starts a reviewed run", async () => {
    const onRunCapability = vi.fn();
    const user = userEvent.setup();
    render(
      <BmadLibraryPanel
        onRunCapability={onRunCapability}
        state={{
          kind: "ready",
          projection: {
            ...baseProjection,
            coreSkills: [
              {
                capabilityId: "bmm:bmad-spec",
                skill: "bmad-spec",
                displayName: "Spec",
                menuCode: "SP",
              },
            ],
          },
        }}
      />,
    );

    expect(screen.getByRole("heading", { name: "Core skills" })).toBeTruthy();
    expect(screen.getByText("Spec")).toBeTruthy();
    // Upstream's own menu identity, not one Forge invented.
    expect(screen.getByText("Menu code SP")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Start reviewed run" }));
    expect(onRunCapability).toHaveBeenCalledWith("bmm:bmad-spec", "Spec");
  });
});
