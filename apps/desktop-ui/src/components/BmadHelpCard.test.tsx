// @vitest-environment jsdom
import "../test/setup";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { describe, expect, it, vi } from "vitest";
import type {
  BmadHelpConfidence,
  BmadHelpRecommendationProjection,
  BmadHelpRunCreatedProjection,
} from "../lib/bmadProjection";
import type { BmadRequestState } from "../lib/bmadModelProjection";
import { BmadHelpCard } from "./BmadHelpCard";

const recommendation: BmadHelpRecommendationProjection = {
  schemaVersion: "bmad-help-recommendation.v1",
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
  completionClaimed: false,
};

const run: BmadHelpRunCreatedProjection = {
  schemaVersion: "bmad-help-run.v1",
  runKind: "bmad_help",
  lifecycle: "created_unbound",
  workspaceId: "workspace-internal-id",
  runId: "run-internal-id",
  sessionId: "session-internal-id",
  currentIntent: "Review architecture readiness",
  runnable: false,
  completionClaimed: false,
  recommendation,
};

const reviewState: BmadRequestState = {
  kind: "review_required",
  run,
  runProjection: run,
  review: {
    workspaceId: run.workspaceId,
    workspaceGrantEpoch: 1,
    runId: run.runId,
    sessionId: run.sessionId,
    destinationLabel: "Deterministic local model",
    developmentOnly: true,
    consentDisclosure: "Only the exact reviewed context will be sent once.",
    manifestHash: `sha256:${"a".repeat(64)}`,
    purpose: "bmad_help",
    region: "localdev",
    retentionMode: "transient_no_store",
    expiresAt: Date.now() + 60_000,
    items: [{
      relativeLabel: "method/current-intent.txt",
      semanticRole: "current_intent",
      language: "text",
      outboundByteCount: run.currentIntent.length,
      tokenEstimate: 1,
      classification: "internal",
      redactions: [],
      outboundContent: run.currentIntent,
    }],
    exclusions: [],
    secretFindings: [],
    totalOutboundBytes: run.currentIntent.length,
    totalTokenEstimate: 1,
    redactionLimitation: "Redaction reduces risk but cannot prove every secret was detected.",
  },
  authority: {
    workspaceId: run.workspaceId,
    workspaceGrantEpoch: 1,
    runId: run.runId,
    sessionId: run.sessionId,
    authEpoch: 1,
    rendererGeneration: 1,
    manifestHash: `sha256:${"a".repeat(64)}`,
    expiresAt: Date.now() + 60_000,
  },
};

function readyState(
  overrides: Partial<BmadHelpRecommendationProjection> = {},
): BmadRequestState {
  return {
    kind: "idle",
    run: {
      ...run,
      recommendation: { ...recommendation, ...overrides },
    },
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

  it("labels the local run as created, unbound, and unable to execute", () => {
    render(<BmadHelpCard state={readyState()} />);

    expect(screen.getByText("Created")).toBeTruthy();
    expect(screen.getByText("Unbound")).toBeTruthy();
    expect(screen.getByText("No model request")).toBeTruthy();
    expect(screen.getByText("Execution unavailable")).toBeTruthy();
    expect(document.body.textContent).not.toContain(run.workspaceId);
    expect(document.body.textContent).not.toContain(run.runId);
    expect(document.body.textContent).not.toContain(run.sessionId);
  });

  it("renders an honest no-evidence state", () => {
    render(<BmadHelpCard state={{ kind: "idle", run: null }} />);

    expect(screen.getByRole("heading", { name: "Suggested next step" })).toBeTruthy();
    expect(screen.getByText("No recommendation yet")).toBeTruthy();
    expect(screen.getByText(
      "No active governed session is available to ground a next step.",
    )).toBeTruthy();
  });

  it("renders loading without inventing a recommendation", () => {
    render(<BmadHelpCard state={{ kind: "creating", activity: "creating" }} />);

    expect(screen.getByRole("status")).toHaveProperty(
      "textContent",
      expect.stringContaining("Preparing an exact Method request review"),
    );
    expect(screen.queryByText("Architecture")).toBeNull();
  });

  it("forwards explicit review gestures to the owning inspector", async () => {
    const user = userEvent.setup();
    const onApprove = vi.fn();
    const onCancel = vi.fn();
    render(
      <BmadHelpCard
        onApprove={onApprove}
        onCancel={onCancel}
        onSend={vi.fn()}
        state={reviewState}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Approve context" }));
    await user.click(screen.getByRole("button", { name: "Cancel review" }));

    expect(onApprove).toHaveBeenCalledOnce();
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it.each([
    "The Method catalog is unavailable.",
    "Method configuration is unavailable.",
    "The selected dependency is unavailable.",
    "The source prompt is unavailable.",
  ])("renders bounded unavailable state: %s", (message) => {
    render(<BmadHelpCard state={{ kind: "unavailable", message, run: null }} />);
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
