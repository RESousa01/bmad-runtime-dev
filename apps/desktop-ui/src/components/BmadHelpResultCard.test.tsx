// @vitest-environment jsdom
import "../test/setup";
import { render, screen } from "@testing-library/react";
import axe from "axe-core";
import { describe, expect, it } from "vitest";
import type { BmadHelpRunCompletedProjection } from "../lib/bmadModelProjection";
import { BmadHelpResultCard } from "./BmadHelpResultCard";

const completed: BmadHelpRunCompletedProjection = {
  schemaVersion: "bmad-help-completed.v1",
  runKind: "bmad_help",
  lifecycle: "completed",
  workspaceId: "workspace_test",
  runId: "run_test",
  sessionId: "session_test",
  runnable: false,
  completionClaimed: true,
  recommendation: {
    recommendationKind: "recommended_capability",
    displayName: "Architecture",
    moduleCode: "bmm",
    skillName: "bmad-architecture",
    action: "create",
    evidenceClass: "user_asserted",
    guidanceRequired: true,
    rationaleSummary: "The reviewed intent contains an explicit architecture signal.",
    createdAt: Date.UTC(2026, 6, 16, 12, 1),
  },
  receipt: {
    schemaVersion: "bmad-model-receipt-summary.v1",
    receiptId: "receipt_safe_123",
    status: "succeeded",
    retentionMode: "transient_no_store",
    region: "localdev",
    inputBytes: 320,
    outputBytes: 144,
    startedAt: Date.UTC(2026, 6, 16, 12, 0),
    completedAt: Date.UTC(2026, 6, 16, 12, 1),
  },
};

describe("BmadHelpResultCard", () => {
  it("renders only the canonical recommendation and safe receipt summary", () => {
    const { container } = render(
      <BmadHelpResultCard developmentOnly result={completed} />,
    );

    expect(screen.getByRole("heading", { name: "Architecture" })).toBeTruthy();
    expect(screen.getByText("bmm / bmad-architecture / create")).toBeTruthy();
    expect(screen.getByText("User asserted")).toBeTruthy();
    expect(screen.getByText(completed.recommendation.recommendationKind === "recommended_capability"
      ? completed.recommendation.rationaleSummary
      : "")).toBeTruthy();
    expect(screen.getByText("receipt_safe_123")).toBeTruthy();
    expect(screen.getByText("Deterministic local model — development only")).toBeTruthy();
    expect(container.querySelectorAll("time[datetime]")).toHaveLength(3);
    expect(container.textContent).not.toMatch(/raw proposal|receipt proof|provider error|manifest hash/i);
    expect(container.textContent).not.toContain(completed.runId);
  });

  it("renders a closed no-recommendation reason", () => {
    render(
      <BmadHelpResultCard
        developmentOnly={false}
        result={{
          ...completed,
          recommendation: {
            recommendationKind: "no_recommendation",
            reasonCode: "catalog_evidence_absent",
            createdAt: Date.UTC(2026, 6, 16, 12, 1),
          },
        }}
      />,
    );

    expect(screen.getByRole("heading", { name: "No recommendation" })).toBeTruthy();
    expect(screen.getByText("No catalog evidence matched the reviewed intent.")).toBeTruthy();
  });

  it("describes unavailable guidance as a BMAD skill dependency", () => {
    render(
      <BmadHelpResultCard
        developmentOnly={false}
        result={{
          ...completed,
          recommendation: {
            recommendationKind: "no_recommendation",
            reasonCode: "dependency_unavailable",
            createdAt: Date.UTC(2026, 6, 16, 12, 1),
          },
        }}
      />,
    );

    expect(screen.getByText("A required BMAD skill dependency was unavailable.")).toBeTruthy();
  });

  it("has no automatically detectable accessibility violations", async () => {
    const { container } = render(<BmadHelpResultCard developmentOnly result={completed} />);
    expect((await axe.run(container)).violations).toEqual([]);
  });
});
