// @vitest-environment jsdom
import "../test/setup";
import { render, screen, within } from "@testing-library/react";
import axe from "axe-core";
import { describe, expect, it } from "vitest";
import type {
  BmadHelpConfidence,
  BmadHelpRecommendationProjection,
  BmadHelpUiState,
} from "../lib/bmadProjection";
import { BmadHelpCard } from "./BmadHelpCard";

const recommendation: BmadHelpRecommendationProjection = {
  displayName: "Architecture",
  moduleCode: "bmm",
  skillName: "bmad-architecture",
  action: "create",
  confidence: "heuristic",
  source: {
    sourceKind: "sealed_foundation",
    packageName: "bmad-method",
    packageVersion: "6.10.0",
  },
  reason: "A bounded artifact match suggests this Method step.",
  requiredGuidance: true,
  expectedArtifacts: ["architecture", "decision record"],
  availability: "dependency_unavailable",
  blockerCodes: ["bmad_dependency_unavailable"],
};

function readyState(
  overrides: Partial<BmadHelpRecommendationProjection> = {},
): BmadHelpUiState {
  return {
    kind: "ready",
    recommendation: { ...recommendation, ...overrides },
  };
}

describe("BmadHelpCard", () => {
  it.each<[BmadHelpConfidence, string]>([
    ["authoritative", "Authoritative"],
    ["user_asserted", "User asserted"],
    ["heuristic", "Heuristic"],
    ["contextual", "Contextual"],
    ["unknown", "Unknown"],
  ])("renders %s confidence as %s", (confidence, label) => {
    render(<BmadHelpCard state={readyState({ confidence })} />);

    expect(screen.getByRole("heading", { name: "Suggested next step" })).toBeTruthy();
    const confidenceRow = screen.getByText("Confidence").closest("div");
    expect(confidenceRow).not.toBeNull();
    expect(within(confidenceRow as HTMLElement).getByText(label)).toBeTruthy();
  });

  it("shows source, reason, expected artifacts, guidance, and blockers", () => {
    render(<BmadHelpCard state={readyState()} />);

    expect(screen.getByText("bmad-method 6.10.0")).toBeTruthy();
    expect(screen.getByText("bmm / bmad-architecture / create")).toBeTruthy();
    expect(screen.getByText(recommendation.reason)).toBeTruthy();
    expect(screen.getByText("architecture")).toBeTruthy();
    expect(screen.getByText("decision record")).toBeTruthy();
    expect(screen.getByText("Required by Method guidance")).toBeTruthy();
    expect(screen.getByText("This guidance does not grant platform permission.")).toBeTruthy();
    expect(screen.getByText("Dependency unavailable")).toBeTruthy();
    expect(screen.getByText("bmad_dependency_unavailable")).toBeTruthy();
  });

  it("renders an honest no-evidence state", () => {
    render(<BmadHelpCard state={{ kind: "no_evidence" }} />);

    expect(screen.getByRole("heading", { name: "Suggested next step" })).toBeTruthy();
    expect(screen.getByText("No recommendation yet")).toBeTruthy();
    expect(screen.getByText(
      "No active governed session is available to ground a next step.",
    )).toBeTruthy();
  });

  it("renders loading without inventing a recommendation", () => {
    render(<BmadHelpCard state={{ kind: "loading" }} />);

    expect(screen.getByRole("status")).toHaveProperty(
      "textContent",
      expect.stringContaining("Finding a source-grounded recommendation"),
    );
    expect(screen.queryByText("Architecture")).toBeNull();
  });

  it.each([
    "The Method catalog is unavailable.",
    "Method configuration is unavailable.",
    "The selected dependency is unavailable.",
    "The source prompt is unavailable.",
  ])("renders bounded unavailable state: %s", (message) => {
    render(<BmadHelpCard state={{ kind: "unavailable", message }} />);
    expect(screen.getByRole("alert")).toHaveProperty(
      "textContent",
      expect.stringContaining(message),
    );
  });

  it("renders empty artifacts and blockers explicitly", () => {
    render(
      <BmadHelpCard
        state={readyState({
          expectedArtifacts: [],
          availability: "available",
          blockerCodes: [],
          requiredGuidance: false,
        })}
      />,
    );

    expect(screen.getByText("No expected artifacts recorded.")).toBeTruthy();
    expect(screen.getByText("Optional Method guidance")).toBeTruthy();
    expect(screen.getByText("Available")).toBeTruthy();
    expect(screen.getByText("No blockers reported.")).toBeTruthy();
  });

  it("keeps projected HTML-like reason and source text inert", () => {
    const malicious = "<img src=x onerror=alert('unsafe')>";
    const { container } = render(
      <BmadHelpCard
        state={readyState({
          reason: malicious,
          source: { ...recommendation.source, packageName: "<script>unsafe</script>" },
        })}
      />,
    );

    expect(screen.getByText(malicious)).toBeTruthy();
    expect(screen.getByText("<script>unsafe</script> 6.10.0")).toBeTruthy();
    expect(container.querySelector("img, script")).toBeNull();
  });

  it("retains full max-bound source, reason, and artifact text for wrapping", () => {
    const longSource = "S".repeat(256);
    const longReason = "R".repeat(2_048);
    const longArtifact = "A".repeat(256);
    render(
      <BmadHelpCard
        state={readyState({
          reason: longReason,
          expectedArtifacts: [longArtifact],
          source: { ...recommendation.source, packageName: longSource },
        })}
      />,
    );

    expect(screen.getByText(`${longSource} 6.10.0`)).toHaveProperty(
      "textContent",
      `${longSource} 6.10.0`,
    );
    expect(screen.getByText(longReason)).toHaveProperty("textContent", longReason);
    expect(screen.getByText(longArtifact)).toHaveProperty("textContent", longArtifact);
  });

  it("never exposes completion or execution controls", () => {
    const { container } = render(<BmadHelpCard state={readyState()} />);

    expect(container.querySelector("button, a, input, select, textarea")).toBeNull();
    expect(document.body.textContent).not.toMatch(
      /\b(?:Chat|Start|Run|Execute|Complete|Completed|Install|Activate|Convert|Evaluate)\b|Approve & apply locally/i,
    );
  });

  it("has no automated accessibility violations in the ready state", async () => {
    const { container } = render(<BmadHelpCard state={readyState()} />);
    const results = await axe.run(container, {
      rules: { "color-contrast": { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });
});
